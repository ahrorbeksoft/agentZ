//! The settings page, laid out like t3code's: a list of sections on the left (General,
//! Appearance, Notifications, Agents, Usage, Skills, MCP Servers, Machines, Storage, then one
//! entry per project) and the chosen section's rows on the right.

mod accounts_menu;
mod mcp_servers;
mod skills;
mod storage;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::SystemTime;

use crate::agent_icons::agent_icon;
use crate::machines::{GroupKey, MachineId, Machines, ProjectGroupingMode, ProjectKey, ThreadKey};
use crate::project_store::ProjectStore;
use agentz_protocol::accounts::{
    AccountChange, AccountChoice, AccountChoices, AccountId, AccountStatus, AccountSupport,
    AgentAccounts, AtLimit, LimitResets, LimitWindow, Overage, OveragePreference, SettingsSource,
};
use agentz_protocol::agents::{
    AgentCommand, AgentId, AgentListing, AgentSession, AgentSessions, CustomAgentChange,
    InstallState,
};
use agentz_protocol::title_generation::{TitleGeneration, TitleProvider, effort_label};
use agentz_protocol::workspace::WorkspaceRemoval;
use agentz_protocol::{CAPABILITY_IMPORT_SESSIONS, Request, Response};
use gpui::{
    AnyElement, App, ClickEvent, Context, DismissEvent, Entity, EventEmitter, FocusHandle,
    Focusable, KeyBinding, ListAlignment, ListOffset, ListState, PathPromptOptions, PromptLevel,
    ScrollHandle, Subscription, Task, UniformListScrollHandle, WeakEntity, Window, actions, list,
    uniform_list,
};
use projects::{Project, ProjectIcon, ProjectId, ThreadId, ThreadOrder, Workspace};
use text_input::{TextInput, TextInputEvent};
use theme::{Appearance, ThemeRegistry};
use ui::{
    ContextMenu, ContextMenuEntry, DropdownMenu, DropdownStyle, IconButtonShape, IconPosition,
    PopoverMenu, ScrollableHandle as _, Switch, TintColor, ToggleButtonGroup,
    ToggleButtonGroupSize, ToggleButtonGroupStyle, ToggleButtonSimple, Tooltip, WithScrollbar as _,
    prelude::*,
};
use util::ResultExt as _;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::thread::{AuthStatus, ConnectionStatus};

use std::collections::BTreeMap;

use crate::agent_login::{AgentLogin, LoginLayout, LoginStep};
use crate::agent_view::TOOLBAR_HEIGHT;
use crate::app_settings::{AppSettingsStore, MachineProfile, PlaySound, ThemeMode};
use crate::confirm_dialog::ConfirmRequest;
use crate::controls::{
    ACCOUNT_COLORS, ActionButton, ActionStyle, account_badge, account_color, avatar, color_hex,
    dialog_frame, dialog_title, icon_tile, spinner, status_badge, status_dot, text_field,
};
use crate::elicitation_card::{ElicitationCard, sync_elicitation_cards};
use crate::machine_icon_picker::MachineIconPicker;
use crate::project_icon_picker::{ProjectIconPicker, ProjectImagePicker};
use crate::project_info::{render_project_icon, workspace_icon};
use crate::project_switcher::compact_path;
use crate::registry_store::AgentRegistryStore;
use crate::server_client::{MachineStatus, ServerClient, ServerUpdate};
use crate::sidebar::{SIDEBAR_WIDTH, format_relative_time, render_footer_item};
use crate::slider::Slider;
use crate::sound::{self, Sound};
use crate::thread_entity::AgentThread;
use crate::usage_limits::{
    is_resetting, render_balance, render_extra_usage, render_limit_cell, render_limit_resets,
    render_limit_windows,
};
use crate::usage_timeline::{TimelineAgent, TimelineZoom, UsageTimeline};

const KEY_CONTEXT: &str = "SettingsPage";
const ACCOUNT_RENAME_KEY_CONTEXT: &str = "AccountRename";
const ACCOUNT_DIALOG_KEY_CONTEXT: &str = "AccountDialog";
/// An account card's avatar, which its limits line up after.
const AVATAR_SIZE: Pixels = px(32.);
/// A window's column in a Usage page table, and the gap between columns. Narrower than the
/// design's 128 and 18, whose page was 64 px wider, so three windows leave room for an email.
const USAGE_COLUMN_WIDTH: Pixels = px(112.);
const USAGE_COLUMN_GAP: Pixels = px(16.);
/// An account's avatar in a menu and on its trigger.
const MENU_AVATAR_SIZE: Pixels = px(16.);
const CONTENT_WIDTH: Pixels = px(720.);
/// The space above each of a page's sections.
const SECTION_SPACING: Rems = rems(1.5);
/// The space between an agent's account cards.
const ACCOUNT_SPACING: Rems = rems(0.625);
/// An account's avatar on its line, with several accounts.
const LINE_AVATAR_SIZE: Pixels = px(26.);
/// An account's card in its dialog, and Add Account's dialog.
const ACCOUNT_DIALOG_WIDTH: Pixels = px(600.);
pub(crate) const ADD_ACCOUNT_DIALOG_WIDTH: Pixels = px(480.);
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
    cx.bind_keys([
        KeyBinding::new("escape", CloseSettings, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(ACCOUNT_RENAME_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(ACCOUNT_RENAME_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(ACCOUNT_DIALOG_KEY_CONTEXT)),
        KeyBinding::new("enter", text_input::Newline, Some("SkillBody > TextInput")),
    ]);
}

pub enum SettingsPageEvent {
    Close,
    /// Open the Add Machine dialog, or Edit… for the machine given.
    EditMachine(Option<MachineProfile>),
    /// Ask before a destructive action, in the shell's modal layer.
    Confirm(ConfirmRequest),
    /// Leave settings for the thread, as an agent's Threads tab opens one.
    OpenThread(ThreadKey),
    /// Show an account's dialog, in the shell's modal layer.
    OpenDialog(Entity<AccountDialog>),
    /// Show a project's Choose icon dialog, in the shell's modal layer.
    ChooseIcon(Entity<ProjectIconPicker>),
    /// Show a project's Choose file picker, in the shell's modal layer.
    ChooseIconFile(Entity<ProjectImagePicker>),
}

/// An agent's account dialog: an account's whole card, opened from its line, or Add Account
/// from picking how to log in to the account added. The settings page draws it and keeps what
/// it shows; closing it, however it closes, tells the page.
pub struct AccountDialog {
    page: WeakEntity<SettingsPage>,
    focus_handle: FocusHandle,
}

impl EventEmitter<DismissEvent> for AccountDialog {}

impl Focusable for AccountDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AccountDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = self
            .page
            .upgrade()
            .and_then(|page| page.update(cx, |page, cx| page.render_account_dialog(window, cx)));
        if content.is_none() {
            // The agent's page closed under it.
            let dialog = cx.weak_entity();
            cx.defer(move |cx| {
                dialog.update(cx, |_, cx| cx.emit(DismissEvent)).log_err();
            });
        }
        div()
            .key_context(ACCOUNT_DIALOG_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|_, _: &menu::Cancel, _, cx| cx.emit(DismissEvent)))
            .children(content)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Section {
    General,
    Appearance,
    Notifications,
    Agents,
    Usage,
    Skills,
    McpServers,
    Machines,
    Storage,
    /// A project's page, showing the copy given: the project combines its checkouts on every
    /// machine, and its page's machine picker moves between them.
    Project(ProjectKey),
}

/// What Settings › Agents shows, as Zed's settings window opens sub-pages.
enum AgentsPage {
    Installed,
    /// Zed's ACP Registry page, to install more agents.
    Registry,
    /// One agent's settings, with the connection made to log in or out.
    Agent(AgentPanel),
    /// Zed's Add Custom Agent form, which also changes one.
    CustomAgent(CustomAgentForm),
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
    /// The open project's copies as last seen, so removing the one shown shows the next.
    project_copies: Vec<ProjectKey>,
    name_input: Entity<TextInput>,
    /// The machine whose agents Settings › Agents shows.
    agents_machine: MachineId,
    usage_timeline_zoom: TimelineZoom,
    /// The Usage timeline's tracks' width when last drawn, which says whose bars fit their
    /// words.
    usage_timeline_width: Pixels,
    /// The machine whose thread titles Settings › General shows.
    titles_machine: MachineId,
    agent_search: Entity<TextInput>,
    /// Machines whose server is being updated, so a second click doesn't restart it midway.
    updating: HashSet<MachineId>,
    /// Whether this Mac's server has a launch agent, so it starts at login.
    starts_at_login: bool,
    agents_page: AgentsPage,
    registry_filter: RegistryFilter,
    skills_page: skills::SkillsPage,
    /// Why adding or deleting a skill failed.
    skill_error: Option<SharedString>,
    /// Add from Folder…, from picking the folder until the server has the skill.
    adding_skill: Option<Task<()>>,
    mcp_servers_page: mcp_servers::McpServersPage,
    /// Why deleting or switching an MCP server failed.
    mcp_server_error: Option<SharedString>,
    storage_page: storage::StoragePage,
    nav_scroll: ScrollHandle,
    /// The open section's rows, a list as Zed's settings pages are: laying out a page whole
    /// made each frame of a scroll slow (taffy measures every row again at each level of
    /// nesting), so a frame lays out the rows in view, each on its own.
    content_list: ListState,
    /// How far the content was scrolled when it was last drawn, which tells a scroll's frames
    /// from changes to its rows.
    content_scroll_top: ListOffset,
    registry_scroll: UniformListScrollHandle,
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
        let mut subscriptions = vec![
            cx.observe(&machines, |this, _, cx| {
                if let Section::Project(key) = this.section {
                    if this.project(key, cx).is_some() {
                        this.project_copies = this.copy_keys(key, cx);
                    } else {
                        // A removed copy's page shows the project's next copy, and a removed
                        // project's page has nothing left to show.
                        let next = next_copy(&this.project_copies, key, |copy| {
                            this.project(copy, cx).is_some()
                        });
                        match next {
                            Some(copy) => this.show_copy(copy, cx),
                            None => this.section = Section::General,
                        }
                    }
                }
                if this
                    .machines
                    .read(cx)
                    .client(this.agents_machine, cx)
                    .is_none()
                {
                    this.agents_machine = MachineId::Local;
                }
                if this
                    .machines
                    .read(cx)
                    .client(this.titles_machine, cx)
                    .is_none()
                {
                    this.titles_machine = MachineId::Local;
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
        ];
        let agent_search = cx.new(|cx| TextInput::new("Search agents…", cx));
        subscriptions.push(
            cx.subscribe(&agent_search, |this, _, _: &TextInputEvent, cx| {
                this.registry_scroll.set_offset(gpui::point(px(0.), px(0.)));
                cx.notify();
            }),
        );
        Self {
            focus_handle: cx.focus_handle(),
            machines,
            app_settings,
            section: Section::General,
            project_copies: Vec::new(),
            name_input,
            agents_machine: MachineId::Local,
            usage_timeline_zoom: TimelineZoom::default(),
            usage_timeline_width: px(488.),
            titles_machine: MachineId::Local,
            agent_search,
            updating: Default::default(),
            starts_at_login: crate::login_item::is_enabled(),
            agents_page: AgentsPage::Installed,
            registry_filter: RegistryFilter::All,
            skills_page: skills::SkillsPage::List,
            skill_error: None,
            adding_skill: None,
            mcp_servers_page: mcp_servers::McpServersPage::List,
            mcp_server_error: None,
            storage_page: storage::StoragePage::default(),
            nav_scroll: ScrollHandle::new(),
            content_list: ListState::new(0, ListAlignment::Top, px(0.)).measure_all(),
            content_scroll_top: ListOffset::default(),
            registry_scroll: UniformListScrollHandle::new(),
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

    /// The agent's Account tab on `machine`, adding an account there with `add_account`, as
    /// its Add Account does. An agent page already open stays, with its login sessions.
    pub fn show_agent_accounts(
        &mut self,
        machine: MachineId,
        agent_id: &AgentId,
        add_account: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let is_open = self.section == Section::Agents
            && self.agents_machine == machine
            && self
                .agent_panel()
                .is_some_and(|panel| panel.agent_id == *agent_id);
        if !is_open {
            self.select(Section::Agents, window, cx);
            self.set_agents_machine(machine, cx);
            let name = self
                .registry(cx)
                .read(cx)
                .agent(agent_id)
                .map(|agent| agent.name().clone())
                .unwrap_or_else(|| agent_id.0.to_string().into());
            self.open_agent(agent_id, &name, window, cx);
        }
        self.select_agent_tab(AgentTab::Account, cx);
        if add_account {
            self.add_account(window, cx);
        }
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

    /// The name the projects list shows for the project the copy is in.
    fn project_name(&self, key: ProjectKey, cx: &App) -> SharedString {
        self.machines
            .read(cx)
            .group_of(key.machine, key.project, cx)
            .map(|group| group.name())
            .or_else(|| self.project(key, cx).map(|project| project.name()))
            .unwrap_or_default()
    }

    fn copy_keys(&self, key: ProjectKey, cx: &App) -> Vec<ProjectKey> {
        self.group_members(key, cx)
            .into_iter()
            .map(|(copy, _)| copy)
            .collect()
    }

    /// The copy whose name and icon stand for the whole project, as in the sidebar: This
    /// Mac's first. The shared sections show it whichever copy is chosen.
    fn shared_copy(&self, key: ProjectKey, cx: &App) -> Option<(ProjectKey, Project)> {
        self.group_members(key, cx).into_iter().next()
    }

    /// Shows another copy of the open project. Its name and icon are the project's, so the
    /// shared sections' inputs stay as they are.
    fn show_copy(&mut self, key: ProjectKey, cx: &mut Context<Self>) {
        if self.section != Section::Project(key) {
            self.scroll_content_to_top();
        }
        self.section = Section::Project(key);
        self.project_copies = self.copy_keys(key, cx);
        cx.notify();
    }

    /// Gives the project's group the icon. An image inside the project is in every copy; one
    /// outside it was picked on this Mac, so only this Mac's checkouts take it.
    fn set_group_icon(&self, key: ProjectKey, icon: Option<ProjectIcon>, cx: &mut App) {
        let is_outside = matches!(&icon, Some(ProjectIcon::Image { path }) if path.is_absolute());
        for (member, project) in self.group_members(key, cx) {
            if project.icon == icon || (is_outside && member.machine != MachineId::Local) {
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

    /// Shows a new page's rows from the top, measured afresh.
    fn scroll_content_to_top(&self) {
        self.content_list.reset(0);
    }

    fn select(&mut self, section: Section, window: &mut Window, cx: &mut Context<Self>) {
        if self.section != section {
            self.scroll_content_to_top();
        }
        self.section = section;
        // Agents always opens on the installed agents; leaving an agent's page stops the agent.
        if !matches!(self.agents_page, AgentsPage::Installed) {
            self.show_agents_page(AgentsPage::Installed, window, cx);
        }
        self.skills_page = skills::SkillsPage::List;
        self.skill_error = None;
        self.mcp_servers_page = mcp_servers::McpServersPage::List;
        self.mcp_server_error = None;
        self.storage_page.error = None;
        if section == Section::Agents {
            self.registry(cx)
                .update(cx, |registry, cx| registry.refresh_if_stale(cx));
        }
        if let Section::Project(key) = section {
            self.project_copies = self.copy_keys(key, cx);
            if let Some((_, project)) = self.shared_copy(key, cx) {
                // Set before the section's inputs fire their change events, which then write
                // the same values back.
                self.name_input.update(cx, |input, cx| {
                    input.set_placeholder(project.folder_name(), cx);
                    input.set_text(project.custom_name.clone().unwrap_or_default(), cx);
                });
            }
        }
        cx.notify();
    }

    /// t3code's Choose icon: an icon, an emoji or a monogram for the whole project.
    fn open_icon_picker(&mut self, key: ProjectKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, project)) = self.shared_copy(key, cx) else {
            return;
        };
        let page = cx.weak_entity();
        let name = self.project_name(key, cx);
        let picker = cx.new(|cx| {
            ProjectIconPicker::new(
                project.icon.as_ref(),
                &name,
                move |icon, _, cx| {
                    page.update(cx, |page, cx| page.set_group_icon(key, Some(icon), cx))
                        .log_err();
                },
                window,
                cx,
            )
        });
        cx.emit(SettingsPageEvent::ChooseIcon(picker));
    }

    /// t3code's Choose file: an image file in the project, as its machine lists them, or, for
    /// this Mac's copies, any file.
    fn open_image_picker(&mut self, key: ProjectKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some((shared_key, _)) = self.shared_copy(key, cx) else {
            return;
        };
        let Some(client) = self.machines.read(cx).client(shared_key.machine, cx) else {
            return;
        };
        let has_local_copy = self
            .group_members(key, cx)
            .iter()
            .any(|(copy, _)| copy.machine == MachineId::Local);
        let page = cx.weak_entity();
        let on_pick_external = has_local_copy.then(|| {
            let page = page.clone();
            Rc::new(move |_: &mut Window, cx: &mut App| {
                page.update(cx, |page, cx| page.choose_icon_file(key, cx))
                    .log_err();
            }) as Rc<dyn Fn(&mut Window, &mut App)>
        });
        let name = self.project_name(key, cx);
        let picker = cx.new(|cx| {
            ProjectImagePicker::new(
                &client,
                shared_key.project,
                name,
                move |path, _, cx| {
                    page.update(cx, |page, cx| {
                        page.set_group_icon(key, Some(ProjectIcon::Image { path }), cx)
                    })
                    .log_err();
                },
                on_pick_external,
                window,
                cx,
            )
        });
        cx.emit(SettingsPageEvent::ChooseIconFile(picker));
    }

    /// Picks any file on this Mac for the project's icon.
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

    /// Removes the copy, the project's only one or one of several: the other machines keep
    /// theirs.
    fn confirm_remove_project(
        &mut self,
        key: ProjectKey,
        name: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.machines.read(cx).projects(key.machine, cx) else {
            return;
        };
        let (title, detail) = if self.group_members(key, cx).len() > 1 {
            (
                format!(
                    "Remove “{name}” from {}?",
                    self.machines.read(cx).label(key.machine, cx)
                ),
                "Its threads there are removed too. Nothing on disk is touched.",
            )
        } else {
            (
                format!("Remove “{name}” from agentZ?"),
                "Its threads are removed too. Nothing on disk is touched.",
            )
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            &title,
            Some(detail),
            &["Remove", "Cancel"],
            cx,
        );
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                store.update(cx, |store, cx| store.remove_project(key.project, cx));
            }
        })
        .detach();
    }

    /// Asks first, then asks again when the server finds work that removing would lose.
    fn confirm_remove_workspace(
        &mut self,
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
            let removal = remove_workspace(&store, path.clone(), false, cx).await;
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
                    remove_workspace(&store, path, true, cx).await.err()
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
        let Some(store) = self.machines.read(cx).projects(machine, cx) else {
            return div().into_any_element();
        };
        let mut rows: Vec<AnyElement> = project
            .workspaces
            .iter()
            .enumerate()
            .map(|(index, workspace)| {
                let branch = store
                    .read(cx)
                    .git_head(&workspace.path)
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
                                this.confirm_remove_workspace(&workspace, window, cx)
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
        let projects = self.machines.read(cx).project_groups(cx);
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
            self.render_nav_item(
                "Notifications",
                Some(IconName::Bell),
                None,
                Section::Notifications,
                cx,
            ),
            self.render_nav_item("Agents", Some(IconName::Sparkle), None, Section::Agents, cx),
            self.render_nav_item("Usage", Some(IconName::Gauge), None, Section::Usage, cx),
            self.render_nav_item("Skills", Some(IconName::Book), None, Section::Skills, cx),
            self.render_nav_item(
                "MCP Servers",
                Some(IconName::ToolHammer),
                None,
                Section::McpServers,
                cx,
            ),
            self.render_nav_item(
                "Machines",
                Some(IconName::Server),
                None,
                Section::Machines,
                cx,
            ),
            self.render_nav_item(
                "Storage",
                Some(IconName::DatabaseZap),
                None,
                Section::Storage,
                cx,
            ),
        ];
        let fixed_count = items.len();
        let mut project_items = Vec::with_capacity(projects.len());
        // One row per project, however many copies it combines: it opens on the copy shown,
        // or else on the sidebar's first (This Mac's).
        for group in &projects {
            let Some((machine, project)) = group.primary() else {
                continue;
            };
            let copy = match self.section {
                Section::Project(open) if group.contains(open.machine, open.project) => open,
                _ => ProjectKey {
                    machine,
                    project: project.id,
                },
            };
            let icon = render_project_icon(machine, project, px(14.), cx);
            let label: SharedString = match group.machines().as_slice() {
                [MachineId::Remote(_)] => format!(
                    "{} · {}",
                    group.name(),
                    self.machines.read(cx).label(machine, cx)
                )
                .into(),
                _ => group.name(),
            };
            project_items.push(self.render_nav_item(
                label,
                None,
                Some(icon),
                Section::Project(copy),
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
            Section::Notifications => "settings-nav-notifications".into(),
            Section::Agents => "settings-nav-agents".into(),
            Section::Usage => "settings-nav-usage".into(),
            Section::Skills => "settings-nav-skills".into(),
            Section::McpServers => "settings-nav-mcp-servers".into(),
            Section::Machines => "settings-nav-machines".into(),
            Section::Storage => "settings-nav-storage".into(),
            Section::Project(key) => format!(
                "settings-nav-project-{}-{}",
                key.machine.slug(),
                key.project.0
            )
            .into(),
        };
        h_flex()
            .id(id.clone())
            .debug_selector(move || id.to_string())
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
                vec![
                    render_row(
                        "Thread order",
                        "How threads are sorted in the sidebar.",
                        DropdownMenu::new("thread-order", label, menu).into_any_element(),
                        cx,
                    ),
                    self.render_modifier_to_send_row(cx),
                    self.render_show_thinking_row(cx),
                ],
                cx,
            ),
            render_section("Chats", vec![self.render_chats_row(cx)], cx),
            render_section(
                "Thread titles",
                self.render_title_generation_rows(window, cx),
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

    /// Zed's "Use Modifier To Send", in its words.
    fn render_modifier_to_send_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.app_settings.read(cx).settings().use_modifier_to_send;
        let app_settings = self.app_settings.clone();
        render_row(
            "Use modifier to send",
            "Whether to always use cmd-enter (or ctrl-enter on Linux or Windows) to send \
             messages.",
            Switch::new("use-modifier-to-send", enabled.into())
                .on_click(move |state, _, cx| {
                    let enabled = *state == ToggleState::Selected;
                    app_settings.update(cx, |store, cx| {
                        store.update(|settings| settings.use_modifier_to_send = enabled, cx)
                    })
                })
                .into_any_element(),
            cx,
        )
    }

    /// Zed's "Thinking Display", as a switch between its expanded and collapsed modes.
    fn render_show_thinking_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.app_settings.read(cx).settings().show_thinking;
        let app_settings = self.app_settings.clone();
        render_row(
            "Show thinking",
            "Whether the agent's thinking shows open in threads. Otherwise it's a Thinking row \
             that opens on click.",
            Switch::new("show-thinking", enabled.into())
                .on_click(move |state, _, cx| {
                    let enabled = *state == ToggleState::Selected;
                    app_settings.update(cx, |store, cx| {
                        store.update(|settings| settings.show_thinking = enabled, cx)
                    })
                })
                .into_any_element(),
            cx,
        )
    }

    fn render_chats_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self.app_settings.read(cx).settings().chats;
        let app_settings = self.app_settings.clone();
        render_row(
            "Chats",
            "Threads for general conversation, outside every project, in their own group in \
             the sidebar.",
            Switch::new("chats", enabled.into())
                .on_click(move |state, _, cx| {
                    let enabled = *state == ToggleState::Selected;
                    app_settings.update(cx, |store, cx| {
                        store.update(|settings| settings.chats = enabled, cx)
                    })
                })
                .into_any_element(),
            cx,
        )
    }

    /// t3code's text generation model, for thread titles: whether the machine titles threads
    /// whose agent doesn't, and with which CLI, model and reasoning effort. Each machine keeps
    /// its own, since the CLIs are installed on it.
    fn render_title_generation_rows(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let mut rows = Vec::new();
        if self.machines.read(cx).has_remotes() {
            rows.push(render_row(
                "Machine",
                "Each machine titles its own threads, with the CLIs installed on it.",
                self.render_titles_machine_picker(window, cx),
                cx,
            ));
        }
        let Some(client) = self.machines.read(cx).client(self.titles_machine, cx) else {
            return rows;
        };
        let state = client.read(cx).title_generation().clone();
        let settings = state.settings.clone();
        let choose = move |settings: TitleGeneration, cx: &mut App| {
            client.update(cx, |client, cx| {
                client.choose_title_generation(settings, cx)
            })
        };
        rows.push(render_row(
            "Generate thread titles",
            "Titles a thread from its first message when its agent doesn't name it, with a \
             coding agent's CLI installed on the machine.",
            div()
                .debug_selector(|| "generate-titles".into())
                .child(
                    Switch::new("generate-titles", settings.enabled.into()).on_click({
                        let settings = settings.clone();
                        let choose = choose.clone();
                        move |state, _, cx| {
                            choose(
                                TitleGeneration {
                                    enabled: *state == ToggleState::Selected,
                                    ..settings.clone()
                                },
                                cx,
                            )
                        }
                    }),
                )
                .into_any_element(),
            cx,
        ));
        if !settings.enabled {
            return rows;
        }

        let info = state
            .providers
            .iter()
            .find(|info| info.provider == settings.provider);
        let provider_description: SharedString = match info {
            None if state.providers.is_empty() => "Looking for the CLIs installed…".into(),
            Some(info) if !info.installed => format!(
                "{} isn't installed on this machine, so threads keep their first message as \
                 their title.",
                settings.provider.program().unwrap_or("Its CLI")
            )
            .into(),
            _ => "The CLI that writes the titles.".into(),
        };
        let menu = ContextMenu::build(window, cx, {
            let settings = settings.clone();
            let providers = state.providers.clone();
            let choose = choose.clone();
            move |mut menu, _, _| {
                for provider in TitleProvider::ALL {
                    let installed = providers
                        .iter()
                        .any(|info| info.provider == provider && info.installed);
                    let label = if installed || providers.is_empty() {
                        provider.label().to_string()
                    } else {
                        format!("{} (not installed)", provider.label())
                    };
                    let chosen = TitleGeneration {
                        enabled: true,
                        provider: provider.clone(),
                        model: None,
                        effort: None,
                    };
                    let choose = choose.clone();
                    menu = menu.item(
                        ContextMenuEntry::new(label)
                            .toggleable(IconPosition::End, settings.provider == provider)
                            .handler(move |_, cx| choose(chosen.clone(), cx)),
                    );
                }
                menu
            }
        });
        rows.push(render_row(
            "Provider",
            provider_description,
            div()
                .debug_selector(|| "title-provider".into())
                .child(DropdownMenu::new(
                    "title-provider",
                    settings.provider.label(),
                    menu,
                ))
                .into_any_element(),
            cx,
        ));

        let models = info.map(|info| info.models.clone()).unwrap_or_default();
        let model = models.iter().find(|model| model.id == settings.model());
        let model_label: SharedString = model
            .map(|model| model.name.clone())
            .unwrap_or_else(|| settings.model().to_string())
            .into();
        let menu = ContextMenu::build(window, cx, {
            let settings = settings.clone();
            let models = models.clone();
            let choose = choose.clone();
            move |mut menu, _, _| {
                for model in &models {
                    let effort = settings
                        .effort()
                        .filter(|effort| model.efforts.iter().any(|offered| offered == effort))
                        .filter(|effort| Some(*effort) != settings.provider.default_effort())
                        .map(str::to_string);
                    let chosen = TitleGeneration {
                        model: (model.id != settings.provider.default_model())
                            .then(|| model.id.clone()),
                        effort,
                        ..settings.clone()
                    };
                    let choose = choose.clone();
                    menu = menu.toggleable_entry(
                        model.name.clone(),
                        model.id == settings.model(),
                        IconPosition::End,
                        None,
                        move |_, cx| choose(chosen.clone(), cx),
                    );
                }
                menu
            }
        });
        rows.push(render_row(
            "Model",
            "The model the CLI writes titles with.",
            div()
                .debug_selector(|| "title-model".into())
                .child(
                    DropdownMenu::new("title-model", model_label, menu).disabled(models.is_empty()),
                )
                .into_any_element(),
            cx,
        ));

        let efforts = model.map(|model| model.efforts.clone()).unwrap_or_default();
        if !efforts.is_empty() {
            let current = settings
                .effort()
                .filter(|effort| efforts.iter().any(|offered| offered == effort))
                .unwrap_or(&efforts[0])
                .to_string();
            let menu = ContextMenu::build(window, cx, {
                let settings = settings.clone();
                let current = current.clone();
                move |mut menu, _, _| {
                    for effort in &efforts {
                        let chosen = TitleGeneration {
                            effort: (Some(effort.as_str()) != settings.provider.default_effort())
                                .then(|| effort.clone()),
                            ..settings.clone()
                        };
                        let choose = choose.clone();
                        menu = menu.toggleable_entry(
                            effort_label(effort),
                            *effort == current,
                            IconPosition::End,
                            None,
                            move |_, cx| choose(chosen.clone(), cx),
                        );
                    }
                    menu
                }
            });
            rows.push(render_row(
                "Reasoning effort",
                "How much the model thinks before it writes a title.",
                div()
                    .debug_selector(|| "title-effort".into())
                    .child(DropdownMenu::new(
                        "title-effort",
                        effort_label(&current),
                        menu,
                    ))
                    .into_any_element(),
                cx,
            ));
        }
        rows
    }

    fn render_titles_machine_picker(
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
        let current = self.titles_machine;
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
                        this.update(cx, |this, cx| {
                            this.titles_machine = machine;
                            cx.notify();
                        })
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
        DropdownMenu::new_with_element("titles-machine", trigger, menu).into_any_element()
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
            "Checkouts of one repository, on this machine or others, share one entry in the \
             projects list.",
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

    fn render_notifications(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let settings = self.app_settings.read(cx).settings();
        let (when_finished, when_input_needed, volume, notify) = (
            settings.play_sound_when_finished,
            settings.play_sound_when_input_needed,
            settings.sound_volume,
            settings.notify_when_unfocused,
        );
        let app_settings = self.app_settings.clone();
        vec![
            render_section(
                "Sounds",
                vec![
                    self.render_volume_row(volume, cx),
                    self.render_sound_row(
                        "Sound when finished",
                        "When to play a sound as an agent finishes its turn.",
                        Sound::Finished,
                        when_finished,
                        window,
                        cx,
                    ),
                    self.render_sound_row(
                        "Sound when input is needed",
                        "When to play a sound as an agent asks for a permission or an answer.",
                        Sound::NeedsInput,
                        when_input_needed,
                        window,
                        cx,
                    ),
                ],
                cx,
            ),
            render_section(
                "System notifications",
                vec![render_row(
                    "Notify when agentZ isn't focused",
                    "Shows a system notification when an agent finishes or needs input while \
                     another app is in front.",
                    Switch::new("notify-when-unfocused", notify.into())
                        .on_click(move |state, _, cx| {
                            let enabled = *state == ToggleState::Selected;
                            app_settings.update(cx, |store, cx| {
                                store
                                    .update(|settings| settings.notify_when_unfocused = enabled, cx)
                            })
                        })
                        .into_any_element(),
                    cx,
                )],
                cx,
            ),
        ]
    }

    /// Both sounds' volume, as macOS's Alert volume: letting go of the slider plays the
    /// finished sound at the new level.
    fn render_volume_row(&self, volume: f32, cx: &mut Context<Self>) -> AnyElement {
        let app_settings = self.app_settings.clone();
        render_row(
            "Volume",
            "How loud agentZ's sounds play. Letting go plays the finished sound.",
            h_flex()
                .gap_2()
                .child(
                    Icon::new(IconName::AudioOff)
                        .size(IconSize::Small)
                        .color(Color::Muted),
                )
                .child(div().debug_selector(|| "sound-volume".to_string()).child(
                    Slider::new("sound-volume", volume).on_release(move |volume, _, cx| {
                        app_settings.update(cx, |store, cx| {
                            store.update(|settings| settings.sound_volume = volume, cx)
                        });
                        sound::play(Sound::Finished, cx);
                    }),
                ))
                .child(
                    Icon::new(IconName::AudioOn)
                        .size(IconSize::Small)
                        .color(Color::Muted),
                )
                .into_any_element(),
            cx,
        )
    }

    /// Zed's "Play sound when agent done" dropdown, for one sound. Picking a value that plays
    /// it plays it once, as picking an alert sound in macOS's Sound settings does.
    fn render_sound_row(
        &self,
        title: &'static str,
        description: &'static str,
        sound: Sound,
        current: PlaySound,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let app_settings = self.app_settings.clone();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for value in PlaySound::ALL {
                let app_settings = app_settings.clone();
                menu = menu.toggleable_entry(
                    value.label(),
                    current == value,
                    IconPosition::End,
                    None,
                    move |_, cx| {
                        app_settings.update(cx, |store, cx| {
                            store.update(
                                |settings| match sound {
                                    Sound::Finished => settings.play_sound_when_finished = value,
                                    Sound::NeedsInput => {
                                        settings.play_sound_when_input_needed = value
                                    }
                                },
                                cx,
                            )
                        });
                        if value != PlaySound::Never {
                            sound::play(sound, cx);
                        }
                    },
                );
            }
            menu
        });
        let id = match sound {
            Sound::Finished => "sound-when-finished",
            Sound::NeedsInput => "sound-when-input-needed",
        };
        render_row(
            title,
            description,
            div()
                .debug_selector(|| id.to_string())
                .child(DropdownMenu::new(id, current.label(), menu))
                .into_any_element(),
            cx,
        )
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
    fn render_agents(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<ContentRow> {
        let sections = match &self.agents_page {
            AgentsPage::Installed => self.render_installed_agents(cx),
            // Laid out by `render_registry` instead, as its list scrolls on its own.
            AgentsPage::Registry => Vec::new(),
            AgentsPage::Agent(_) => return self.render_agent_page(window, cx),
            AgentsPage::CustomAgent(_) => self.render_custom_agent_form(window, cx),
        };
        sections.into_iter().map(ContentRow::section).collect()
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
            AgentsPage::Registry => self.render_breadcrumb("ACP Registry", cx),
            AgentsPage::CustomAgent(form) => self.render_breadcrumb(
                if form.agent_id.is_some() {
                    "Configure Custom Agent"
                } else {
                    "Add Custom Agent"
                },
                cx,
            ),
        };
        let machine = if !self.machines.read(cx).has_remotes() {
            None
        } else if let AgentsPage::Agent(_) | AgentsPage::CustomAgent(_) = self.agents_page {
            // An agent's page, or its form, belongs to the machine it was opened on.
            Some(self.render_machine_label(cx))
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

    /// The machine a page belongs to, where a picker would let it change.
    fn render_machine_label(&self, cx: &mut Context<Self>) -> AnyElement {
        let machines = self.machines.read(cx);
        h_flex()
            .gap_1p5()
            .child(
                Icon::new(machines.machine_icon(self.agents_machine, cx))
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(Label::new(machines.label(self.agents_machine, cx)).color(Color::Muted))
            .into_any_element()
    }

    fn render_breadcrumb(&self, title: &'static str, cx: &mut Context<Self>) -> AnyElement {
        self.render_sub_page_heading(
            "agents-back",
            "Agents",
            title,
            |this, window, cx| {
                if let AgentsPage::CustomAgent(_) = this.agents_page {
                    this.close_custom_agent_form(window, cx)
                } else {
                    this.show_agents_page(AgentsPage::Installed, window, cx)
                }
            },
            cx,
        )
    }

    /// Zed's sub-page heading: a back button and "`parent` / `title`".
    fn render_sub_page_heading(
        &self,
        back_id: &'static str,
        parent: &'static str,
        title: &'static str,
        on_back: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        h_flex()
            .min_w_0()
            .ml_neg_1p5()
            .gap_1()
            .child(
                div().debug_selector(move || back_id.into()).child(
                    IconButton::new(back_id, IconName::ArrowLeft)
                        .icon_size(IconSize::Small)
                        .shape(IconButtonShape::Square)
                        .tooltip(Tooltip::text("Back"))
                        .on_click(
                            cx.listener(move |this, _, window, cx| on_back(this, window, cx)),
                        ),
                ),
            )
            .child(
                Headline::new(parent)
                    .size(HeadlineSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Headline::new("/")
                    .size(HeadlineSize::Small)
                    .color(Color::Muted),
            )
            .child(Headline::new(title).size(HeadlineSize::Small))
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
        let page = cx.weak_entity();
        let add = PopoverMenu::new("agents-add-menu")
            .trigger(
                Button::new("agents-add", "Add Agent")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::Small)
                    .color(Color::Muted)
                    .start_icon(
                        Icon::new(IconName::Plus)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    ),
            )
            .anchor(gpui::Anchor::TopRight)
            .menu(move |window, cx| {
                let page = page.clone();
                Some(ContextMenu::build(window, cx, move |menu, _, _| {
                    let registry_page = page.clone();
                    menu.entry("Install from Registry", None, move |window, cx| {
                        registry_page
                            .update(cx, |page, cx| {
                                page.show_agents_page(AgentsPage::Registry, window, cx)
                            })
                            .log_err();
                    })
                    .entry("Add Custom Agent", None, move |window, cx| {
                        page.update(cx, |page, cx| page.open_custom_agent_form(None, window, cx))
                            .log_err();
                    })
                    .separator()
                    .header("Learn More")
                    .item(
                        ContextMenuEntry::new("ACP Docs")
                            .icon(IconName::ArrowUpRight)
                            .icon_color(Color::Muted)
                            .icon_position(IconPosition::End)
                            .handler(|_, cx| cx.open_url("https://agentclientprotocol.com/")),
                    )
                }))
            });
        vec![render_section_with_actions(
            "Installed",
            rows,
            div()
                .debug_selector(|| "agents-add".into())
                .child(add)
                .into_any_element(),
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
                !agent.is_custom()
                    && agent.supports_current_platform()
                    && matches_query
                    && matches_filter
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
    fn render_agent_page(&self, window: &mut Window, cx: &mut Context<Self>) -> Vec<ContentRow> {
        let Some(panel) = self.agent_panel() else {
            return Vec::new();
        };
        let listing = self.registry(cx).read(cx).agent(&panel.agent_id).cloned();
        let support = listing
            .as_ref()
            .and_then(|listing| listing.accounts.clone());
        let heading = ContentRow::section(
            v_flex()
                .gap(px(22.))
                .child(self.render_agent_heading(listing.as_ref(), cx))
                .child(self.render_agent_tabs(panel.tab, cx))
                .into_any_element(),
        );
        let content = match panel.tab {
            AgentTab::Account => match &support {
                Some(support) => {
                    return std::iter::once(heading)
                        .chain(self.render_accounts_tab(support, window, cx))
                        .collect();
                }
                None => self.render_account_tab(cx),
            },
            AgentTab::Defaults => self.render_agent_defaults(window, cx),
            AgentTab::Environment => self.render_agent_env(cx),
            AgentTab::Threads => self.render_agent_threads(window, cx),
        };
        let picker = match panel.tab {
            AgentTab::Account => None,
            _ => self.render_account_picker(window, cx),
        };
        let content = match picker {
            Some(picker) => v_flex()
                .gap(px(18.))
                .child(picker)
                .child(content)
                .into_any_element(),
            None => content,
        };
        vec![heading, ContentRow::section(content)]
    }

    /// The agent's icon, its name beside whether it's logged in, its version, description and
    /// links, Update when there's a newer version, and a menu with Uninstall.
    fn render_agent_heading(
        &self,
        listing: Option<&AgentListing>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(panel) = self.agent_panel() else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors().clone();
        let status_colors = cx.theme().status().clone();
        let connection = panel.external.connection.read(cx);
        let id = panel.agent_id.clone();
        let name = listing
            .map(|agent| agent.name().clone())
            .unwrap_or_else(|| connection.agent_name().clone());
        let (badge_label, badge_color) = match panel.agent_state(cx) {
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
        let is_custom = listing.is_some_and(AgentListing::is_custom);
        let page = cx.weak_entity();
        let menu = PopoverMenu::new("agent-menu")
            .menu(move |window, cx| {
                let page = page.clone();
                let id = menu_id.clone();
                let name = menu_name.clone();
                Some(ContextMenu::build(window, cx, move |menu, _, _| {
                    let menu = if is_custom {
                        let page = page.clone();
                        let id = id.clone();
                        menu.entry("Configure…", None, move |window, cx| {
                            page.update(cx, |page, cx| {
                                let listing = page.registry(cx).read(cx).agent(&id).cloned();
                                page.open_custom_agent_form(listing.as_ref(), window, cx)
                            })
                            .log_err();
                        })
                    } else {
                        menu
                    };
                    let verb = if is_custom { "Remove" } else { "Uninstall" };
                    menu.entry(format!("{verb} {name}…"), None, move |window, cx| {
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
                                    this.scroll_content_to_top();
                                }))
                        })
                        .child(label)
                }),
            )
            .into_any_element()
    }

    /// "Account" and a menu of the accounts over the Defaults, Environment and Threads tabs,
    /// which picks whose settings or sessions they show. An agent with one account has none.
    fn render_account_picker(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let panel = self.agent_panel()?;
        let accounts = panel.client(cx).read(cx).accounts(&panel.agent_id);
        let entries = account_entries(&accounts);
        if entries.len() < 2 {
            return None;
        }
        let current = panel.settings_account(&accounts);
        let shown = entries.iter().find(|entry| entry.account == current)?;
        let label = h_flex()
            .gap_1p5()
            .child(render_entry_avatar(shown, MENU_AVATAR_SIZE, cx))
            .child(Label::new(shown.name.clone()).size(LabelSize::Small))
            .into_any_element();
        let page = cx.weak_entity();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for entry in entries {
                let account = entry.account;
                let selector = account_selector(account);
                menu = menu.custom_entry(
                    move |_, cx| {
                        let selector = selector.clone();
                        div()
                            .w_full()
                            .debug_selector(move || format!("account-picker-{selector}"))
                            .child(render_account_entry(&entry, entry.account == current, cx))
                            .into_any_element()
                    },
                    on_page(&page, move |page, _, cx| page.pick_account(account, cx)),
                );
            }
            menu
        });
        Some(
            h_flex()
                .gap_2()
                .child(
                    Label::new("Account")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    div().debug_selector(|| "account-picker".into()).child(
                        DropdownMenu::new_with_element("account-picker", label, menu)
                            .style(DropdownStyle::Outlined)
                            .trigger_size(ButtonSize::Compact),
                    ),
                )
                .into_any_element(),
        )
    }

    /// The agent whose page is open.
    fn agent_panel(&self) -> Option<&AgentPanel> {
        match &self.agents_page {
            AgentsPage::Agent(panel) => Some(panel),
            _ => None,
        }
    }

    fn agent_panel_mut(&mut self) -> Option<&mut AgentPanel> {
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
        self.scroll_content_to_top();
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
        // The tabs open on the account for new threads.
        let account = client.read(cx).accounts(id).new_thread_account();
        let env_rows = self.account_env_rows(&client, id, account, cx);
        let external = self.open_account_session(&client, id, None, name, cx);
        // Accounts added or removed, here or in another window.
        let accounts_changed = cx.observe(&client, |this, _, cx| {
            this.sync_account_sessions(cx);
            this.sync_settings_account(cx);
        });
        let panel = AgentPanel {
            agent_id: id.clone(),
            tab: AgentTab::Account,
            external,
            accounts: BTreeMap::new(),
            renaming: None,
            adding_account: None,
            account_error: None,
            picked_account: AccountChoice::Default,
            env_account: account,
            env_rows,
            sessions_account: account,
            sessions: None,
            sessions_project: None,
            sessions_shown: SESSIONS_INITIAL_COUNT,
            importing: HashSet::new(),
            import_error: None,
            limit_tabs: HashMap::new(),
            switching_to_core: HashSet::new(),
            using_limit_reset: HashSet::new(),
            dialog: None,
            _subscriptions: vec![accounts_changed],
        };
        self.show_agents_page(AgentsPage::Agent(panel), window, cx);
        self.sync_account_sessions(cx);
    }

    /// Starts the agent in the account's home (`None` being the External account's), to show
    /// whether it's logged in and to log it in or out.
    fn open_account_session(
        &self,
        client: &Entity<ServerClient>,
        agent_id: &AgentId,
        account: Option<AccountId>,
        agent_name: &SharedString,
        cx: &mut Context<Self>,
    ) -> AccountSession {
        let connection = cx.new(|cx| {
            AgentThread::open_login_session(
                client.clone(),
                agent_id.clone(),
                account,
                agent_name.clone(),
                cx,
            )
        });
        let login = cx.new(|cx| {
            AgentLogin::new(
                connection.clone(),
                LoginLayout::Rows,
                Some(agent_id.clone()),
                cx,
            )
        });
        // The server remembers the options and modes the agent offers, and logins made in
        // the panel.
        let subscription = cx.observe(&connection, move |this, connection, cx| {
            if let Some(session) = this
                .agent_panel_mut()
                .and_then(|panel| panel.session_mut(account))
            {
                sync_elicitation_cards(&mut session.elicitation_cards, &connection, cx);
                let thread = connection.read(cx);
                let was_authenticating =
                    std::mem::replace(&mut session.was_authenticating, thread.is_authenticating());
                // A finished login settles the account change.
                if was_authenticating
                    && !thread.is_authenticating()
                    && thread.auth_error().is_none()
                {
                    session.changing_account = false;
                }
            }
            cx.notify()
        });
        AccountSession {
            connection,
            login,
            elicitation_cards: Vec::new(),
            changing_account: false,
            was_authenticating: false,
            _subscription: subscription,
        }
    }

    /// Gives each of the agent's agentZ accounts a login session, and stops those of the
    /// accounts that are gone.
    fn sync_account_sessions(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let agent_id = panel.agent_id.clone();
        let connection = panel.external.connection.read(cx);
        let client = connection.client().clone();
        let agent_name = connection.agent_name().clone();
        let listed: Vec<AccountId> = client
            .read(cx)
            .accounts(&agent_id)
            .accounts
            .iter()
            .map(|account| account.id)
            .collect();
        let missing: Vec<AccountId> = listed
            .iter()
            .copied()
            .filter(|id| !panel.accounts.contains_key(id))
            .collect();
        let is_unchanged = missing.is_empty() && panel.accounts.len() == listed.len();
        // With one account left, the page shows its card, and its dialog closes.
        let shows_card = client.read(cx).accounts(&agent_id).listed().len() <= 1;
        let has_account_dialog = panel
            .dialog
            .as_ref()
            .is_some_and(|dialog| matches!(dialog.content, DialogContent::Account(_)));
        if shows_card && has_account_dialog {
            self.close_account_dialog(cx);
        }
        if is_unchanged {
            return;
        }
        let opened: Vec<(AccountId, AccountSession)> = missing
            .into_iter()
            .map(|id| {
                let session =
                    self.open_account_session(&client, &agent_id, Some(id), &agent_name, cx);
                (id, session)
            })
            .collect();
        if let Some(panel) = self.agent_panel_mut() {
            panel.accounts.retain(|id, _| listed.contains(id));
            panel.accounts.extend(opened);
            if panel
                .renaming
                .as_ref()
                .and_then(|rename| rename.account)
                .is_some_and(|id| !listed.contains(&id))
            {
                panel.renaming = None;
            }
        }
        // A removed account's dialog closes with it.
        let is_removed = self
            .agent_panel()
            .and_then(|panel| panel.dialog.as_ref())
            .is_some_and(|dialog| {
                matches!(dialog.content, DialogContent::Account(Some(id)) if !listed.contains(&id))
            });
        if is_removed {
            self.close_account_dialog(cx);
        }
        self.sync_adding_login(cx);
        cx.notify();
    }

    /// A variable on the Environment tab, saved as it's typed.
    fn new_env_row(&self, key: &str, value: &str, cx: &mut Context<Self>) -> EnvRow {
        let mut row = new_variable_row(key, value, cx);
        row._subscriptions = vec![
            cx.subscribe(&row.key, |this, _, _: &TextInputEvent, cx| {
                this.save_env(cx)
            }),
            cx.subscribe(&row.value, |this, _, _: &TextInputEvent, cx| {
                this.save_env(cx)
            }),
        ];
        row
    }

    /// The Environment tab's rows for the account's variables.
    fn account_env_rows(
        &self,
        client: &Entity<ServerClient>,
        agent_id: &AgentId,
        account: Option<AccountId>,
        cx: &mut Context<Self>,
    ) -> Vec<EnvRow> {
        client
            .read(cx)
            .account_settings(agent_id, account)
            .env
            .iter()
            .map(|(key, value)| self.new_env_row(key, value, cx))
            .collect()
    }

    /// Shows the account's variables on the Environment tab, read again from its settings.
    fn show_account_env(&mut self, account: Option<AccountId>, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let client = panel.client(cx);
        let agent_id = panel.agent_id.clone();
        let rows = self.account_env_rows(&client, &agent_id, account, cx);
        if let Some(panel) = self.agent_panel_mut() {
            panel.env_account = account;
            panel.env_rows = rows;
        }
        cx.notify();
    }

    /// Follows the account the tabs show when it changes: picked, made the default, or gone.
    fn sync_settings_account(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let accounts = panel.client(cx).read(cx).accounts(&panel.agent_id);
        let account = panel.settings_account(&accounts);
        if panel.env_account != account {
            self.show_account_env(account, cx);
        }
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        if panel.sessions_account != account && panel.sessions.is_some() {
            panel.sessions = None;
            panel.importing.clear();
            panel.import_error = None;
            if panel.tab == AgentTab::Threads {
                self.list_agent_sessions(cx);
            }
            cx.notify();
        }
    }

    fn pick_account(&mut self, account: Option<AccountId>, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        panel.picked_account = AccountChoice::of(account);
        self.sync_settings_account(cx);
        cx.notify();
    }

    /// Writes the panel's variables to their account's settings; rows without a name are
    /// skipped.
    fn save_env(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel() else {
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
        let agent_id = panel.agent_id.clone();
        let account = panel.env_account;
        panel.client(cx).update(cx, |client, cx| {
            client.update_account_settings(&agent_id, account, |settings| settings.env = env, cx)
        });
    }

    /// Opens Add Custom Agent, or the form for the custom agent given, filled in from it.
    fn open_custom_agent_form(
        &mut self,
        agent: Option<&AgentListing>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let command = agent
            .and_then(|agent| agent.custom_command.clone())
            .unwrap_or_default();
        // Its own variables, and those its Environment tab added, which win when it starts.
        let mut env: BTreeMap<String, String> = command.env.into_iter().collect();
        if let Some(agent) = agent {
            env.extend(
                self.agents_client(cx)
                    .read(cx)
                    .agent_settings(&agent.id().0)
                    .env,
            );
        }
        let path = command.path.to_string_lossy().into_owned();
        let name_text = agent
            .map(|agent| agent.name().to_string())
            .unwrap_or_default();
        let name = new_text_input("My Agent", &name_text, cx);
        let form = CustomAgentForm {
            agent_id: agent.map(|agent| agent.id().clone()),
            name: name.clone(),
            command: new_text_input("/path/to/agent", &path, cx),
            args: new_text_input("--flag value", &command.args.join(" "), cx),
            env: env
                .iter()
                .map(|(key, value)| new_variable_row(key, value, cx))
                .collect(),
            error: None,
            saving: None,
        };
        self.show_agents_page(AgentsPage::CustomAgent(form), window, cx);
        window.focus(&name.focus_handle(cx), cx);
    }

    /// Back to where the form was opened from: the agent's page, or the installed agents.
    fn close_custom_agent_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let AgentsPage::CustomAgent(form) = &self.agents_page else {
            return;
        };
        let listing = form
            .agent_id
            .as_ref()
            .and_then(|id| self.registry(cx).read(cx).agent(id).cloned());
        match listing {
            Some(agent) => self.open_agent(agent.id(), agent.name(), window, cx),
            None => self.show_agents_page(AgentsPage::Installed, window, cx),
        }
    }

    /// Sends the form to the server, which starts the agent to check it before keeping it.
    fn save_custom_agent_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let registry = self.registry(cx);
        let AgentsPage::CustomAgent(form) = &mut self.agents_page else {
            return;
        };
        if form.saving.is_some() {
            return;
        }
        let path = form.command.read(cx).text().trim().to_string();
        if path.is_empty() {
            form.error = Some("Command is required.".into());
            cx.notify();
            return;
        }
        let mut env = collections::HashMap::default();
        for row in &form.env {
            let key = row.key.read(cx).text().trim().to_string();
            if key.is_empty() {
                continue;
            }
            if env
                .insert(key.clone(), row.value.read(cx).text().to_string())
                .is_some()
            {
                form.error = Some(format!("Duplicate environment variable \"{key}\".").into());
                cx.notify();
                return;
            }
        }
        let change = CustomAgentChange {
            agent_id: form.agent_id.clone(),
            name: form.name.read(cx).text().trim().to_string(),
            command: AgentCommand {
                path: path.into(),
                args: form
                    .args
                    .read(cx)
                    .text()
                    .split_whitespace()
                    .map(str::to_string)
                    .collect(),
                env,
                env_remove: Vec::new(),
            },
        };
        let save = registry.read(cx).save_custom_agent(change, cx);
        form.error = None;
        form.saving = Some(cx.spawn_in(window, async move |this, cx| {
            let saved = save.await;
            this.update_in(cx, |this, window, cx| {
                let AgentsPage::CustomAgent(form) = &mut this.agents_page else {
                    return;
                };
                form.saving = None;
                match saved {
                    Ok(_) => this.show_agents_page(AgentsPage::Installed, window, cx),
                    Err(error) => {
                        form.error = Some(format!("{error:#}").into());
                        cx.notify();
                    }
                }
            })
            .log_err();
        }));
        cx.notify();
    }

    /// Zed's custom agent form: its name, the command that starts it with its arguments, and
    /// its environment, then Cancel and Save.
    fn render_custom_agent_form(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let AgentsPage::CustomAgent(form) = &self.agents_page else {
            return Vec::new();
        };
        let field = |input: &Entity<TextInput>, cx: &App| {
            text_field(input, false, window, cx)
                .font_buffer(cx)
                .text_size(rems_from_px(12_f32))
        };
        let agent = render_section(
            "Agent",
            vec![
                render_row(
                    "Name",
                    "Optional. Left blank, it's the name the agent gives itself.",
                    div()
                        .w(px(320.))
                        .child(text_field(&form.name, false, window, cx))
                        .into_any_element(),
                    cx,
                ),
                render_row(
                    "Command",
                    "Required. Path to the executable that launches the agent.",
                    div()
                        .w(px(320.))
                        .child(field(&form.command, cx))
                        .into_any_element(),
                    cx,
                ),
                render_row(
                    "Arguments",
                    "Space-separated arguments passed to the command.",
                    div()
                        .w(px(320.))
                        .child(field(&form.args, cx))
                        .into_any_element(),
                    cx,
                ),
            ],
            cx,
        );

        let mut variables: Vec<AnyElement> = form
            .env
            .iter()
            .enumerate()
            .map(|(index, row)| {
                h_flex()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .child(div().w(px(180.)).child(field(&row.key, cx)))
                    .child(Label::new("=").color(Color::Muted))
                    .child(div().flex_1().min_w_0().child(field(&row.value, cx)))
                    .child(
                        IconButton::new(("custom-agent-remove-env", index), IconName::Close)
                            .icon_size(IconSize::Small)
                            .tooltip(Tooltip::text("Remove Variable"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let AgentsPage::CustomAgent(form) = &mut this.agents_page
                                    && index < form.env.len()
                                {
                                    form.env.remove(index);
                                }
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        if variables.is_empty() {
            variables.push(
                div()
                    .px_4()
                    .py_3()
                    .child(Label::new("No variables.").color(Color::Muted))
                    .into_any_element(),
            );
        }
        let add_variable = Button::new("custom-agent-add-env", "Add Variable")
            .style(ButtonStyle::Subtle)
            .label_size(LabelSize::Small)
            .color(Color::Muted)
            .start_icon(
                Icon::new(IconName::Plus)
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            )
            .on_click(cx.listener(|this, _, window, cx| {
                let row = new_variable_row("", "", cx);
                // So the new name can be typed straight away.
                window.focus(&row.key.focus_handle(cx), cx);
                if let AgentsPage::CustomAgent(form) = &mut this.agents_page {
                    form.env.push(row);
                }
                cx.notify();
            }));
        let environment = v_flex()
            .gap_2()
            .child(render_section_with_actions(
                "Environment Variables",
                variables,
                add_variable.into_any_element(),
                cx,
            ))
            .child(
                Label::new("Environment variables provided to the agent process.")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .into_any_element();

        let error = form.error.clone().map(|error| {
            h_flex()
                .debug_selector(|| "custom-agent-error".into())
                .gap_2()
                .items_start()
                .child(
                    Icon::new(IconName::XCircle)
                        .size(IconSize::Small)
                        .color(Color::Error),
                )
                .child(Label::new(error).size(LabelSize::Small).color(Color::Error))
                .into_any_element()
        });
        let is_saving = form.saving.is_some();
        let actions = h_flex()
            .justify_end()
            .gap_2()
            .child(
                div().debug_selector(|| "custom-agent-cancel".into()).child(
                    ActionButton::new("custom-agent-cancel", "Cancel")
                        .style(ActionStyle::Ghost)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.close_custom_agent_form(window, cx)
                        })),
                ),
            )
            .child(
                div().debug_selector(|| "custom-agent-save".into()).child(
                    // The server starts the agent to check it, which takes a moment.
                    ActionButton::new(
                        "custom-agent-save",
                        if is_saving { "Starting…" } else { "Save" },
                    )
                    .style(ActionStyle::Primary)
                    .disabled(is_saving)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.save_custom_agent_form(window, cx)),
                    ),
                ),
            )
            .into_any_element();
        [agent, environment]
            .into_iter()
            .chain(error)
            .chain([actions])
            .collect()
    }

    /// Zed's per-agent defaults, here the account's: what a new session starts with. Choosing a
    /// setting in a thread changes these too.
    fn render_agent_defaults(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        const TITLE: &str = "Defaults for New Threads";
        let Some(panel) = self.agent_panel() else {
            return div().into_any_element();
        };
        let agent_id = panel.agent_id.clone();
        let agent_name = panel.external.connection.read(cx).agent_name().clone();
        let client = panel.client(cx);
        let accounts = client.read(cx).accounts(&agent_id);
        let account = panel.settings_account(&accounts);
        let agent = client.read(cx).account_settings(&agent_id, account);
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
                                client.update_account_settings(
                                    &agent_id,
                                    account,
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
            let selector = format!("agent-default-{}", option.id.0);
            rows.push(render_row(
                option.name.clone(),
                option.description.clone().unwrap_or_default(),
                div()
                    .debug_selector({
                        let selector = selector.clone();
                        move || selector
                    })
                    .child(DropdownMenu::new(SharedString::from(selector), label, menu))
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
                                client.update_account_settings(
                                    &agent_id,
                                    account,
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
            let session = panel.session(account).unwrap_or(&panel.external);
            let connection = session.connection.read(cx);
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
        let note = if accounts.listed().len() > 1 {
            "Choosing one in a thread on this account also makes it the default."
        } else {
            "Choosing one in a thread also makes it the default."
        };
        render_section_with_note(TITLE, rows, note, cx)
    }

    /// The variables the agent starts with, one row each.
    fn render_agent_env(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(panel) = self.agent_panel() else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors().clone();
        let agent_name = panel.external.connection.read(cx).agent_name().clone();
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
                                if let Some(panel) = this.agent_panel_mut()
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
                if let Some(panel) = this.agent_panel_mut() {
                    panel.env_rows.push(row);
                }
                cx.notify();
            }));
        let has_accounts = panel
            .client(cx)
            .read(cx)
            .accounts(&panel.agent_id)
            .listed()
            .len()
            > 1;
        let starts = if has_accounts {
            "when it starts on this account"
        } else {
            "when it starts"
        };
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
                    "Passed to {agent_name} {starts}. Running threads pick them up after Reload \
                     Agent."
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
        let Some(panel) = self.agent_panel() else {
            return div().into_any_element();
        };
        let account = &panel.external;
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
            .agent_settings(&panel.agent_id.0)
            .login_method;
        let machine = self.machines.read(cx).label(self.agents_machine, cx);
        let check_again = IconButton::new("account-check", IconName::RotateCw)
            .icon_size(IconSize::Small)
            .icon_color(Color::Muted)
            .tooltip(Tooltip::text("Check Again"))
            .on_click(cx.listener(|this, _, _, cx| {
                if let Some(panel) = this.agent_panel_mut() {
                    panel
                        .external
                        .connection
                        .update(cx, |connection, cx| connection.check_login(cx));
                }
            }))
            .into_any_element();

        let mut rows: Vec<AnyElement> = Vec::new();
        let mut shows_login = false;
        // Agents that report no account show the method as the card's title.
        let mut title_shows_method = false;
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
                title_shows_method = email.is_none()
                    && auth_status
                        .as_ref()
                        .is_none_or(|status| status.label.is_none());
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
                                cx.listener(|this, _, _, cx| this.confirm_logout(None, None, cx)),
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
                            Some(email) => avatar(email, None, AVATAR_SIZE, cx),
                            None => account_badge(AVATAR_SIZE, cx),
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
                tab.child(render_login_source(
                    &agent_name,
                    &machine,
                    login_method.as_deref(),
                    title_shows_method,
                    can_log_out,
                    cx,
                ))
            })
            .children(cards)
            .into_any_element()
    }

    /// The Account tab of an agent that can have more accounts: a card per account under
    /// "Accounts" and Add Account, the External account first while it's listed. Each card is a
    /// row of the page's list, so a scroll lays out only the cards in view.
    fn render_accounts_tab(
        &self,
        support: &AccountSupport,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<ContentRow> {
        let Some(panel) = self.agent_panel() else {
            return Vec::new();
        };
        let colors = cx.theme().colors().clone();
        let status_colors = cx.theme().status().clone();
        let external = panel.external.connection.read(cx);
        let agent_name = external.agent_name().clone();
        let external_state = AccountState::of(external);
        let external_failure = match external.status() {
            ConnectionStatus::Failed(error) => Some(error.clone()),
            _ => None,
        };
        let accounts = external.client().read(cx).accounts(&panel.agent_id);
        // The account being added is in Add Account's dialog until it's done.
        let adding = panel.adding().and_then(|adding| adding.account);
        let listed: Vec<Option<AccountId>> = accounts
            .listed()
            .into_iter()
            .filter(|&account| adding.is_none_or(|id| account != Some(id)))
            .collect();
        let in_dialog = panel
            .dialog
            .as_ref()
            .and_then(|dialog| match dialog.content {
                DialogContent::Account(account) => Some(account),
                DialogContent::Adding(_) => None,
            });

        let mut cards: Vec<AnyElement> = Vec::new();
        // Lines sit against each other, as one table.
        let mut lines: Vec<AnyElement> = Vec::new();
        if listed.len() > 1 {
            lines = self.render_account_lines(&listed, &accounts, cx);
        } else {
            cards.extend(listed.iter().map(|&account| {
                self.render_account_card(
                    account,
                    &accounts,
                    support,
                    listed.len(),
                    CardPlace::Page,
                    window,
                    cx,
                )
            }));
        }
        for &account in listed.iter().filter(|&&account| in_dialog != Some(account)) {
            if let Some(session) = panel.session(account) {
                cards.extend(
                    session
                        .elicitation_cards
                        .iter()
                        .map(|card| card.clone().into_any_element()),
                );
            }
        }
        if listed.is_empty() {
            // The agent isn't logged in outside agentZ, and has no account of agentZ's yet.
            let row = match external_failure.filter(|_| external_state == AccountState::Failed) {
                Some(error) => render_status_row(
                    status_dot(status_colors.error).into_any_element(),
                    format!("Couldn't start {agent_name}").into(),
                    Some((error, Color::Error)),
                    vec![
                        ActionButton::new("account-retry", "Try Again")
                            .on_click(
                                cx.listener(|this, _, window, cx| this.reopen_agent(window, cx)),
                            )
                            .into_any_element(),
                    ],
                ),
                None => render_status_row(
                    status_dot(status_colors.warning).into_any_element(),
                    "Not logged in".into(),
                    Some((
                        format!("Add an account to log {agent_name} in.").into(),
                        Color::Muted,
                    )),
                    Vec::new(),
                ),
            };
            cards.push(
                div()
                    .debug_selector(|| "account-card-none".into())
                    .rounded_lg()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.panel_background)
                    .child(row)
                    .into_any_element(),
            );
        }

        let add = Button::new("account-add", "Add Account")
            .style(ButtonStyle::Subtle)
            .label_size(LabelSize::Small)
            .color(Color::Muted)
            .start_icon(
                Icon::new(IconName::Plus)
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            )
            .disabled(panel.adding_account.is_some())
            .on_click(cx.listener(|this, _, window, cx| this.add_account(window, cx)));
        let header = h_flex()
            .justify_between()
            .child(
                Label::new("Accounts")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .child(div().debug_selector(|| "account-add".into()).child(add))
            .into_any_element();
        let error = panel.account_error.clone().map(|error| {
            Label::new(error)
                .size(LabelSize::Small)
                .color(Color::Error)
                .into_any_element()
        });
        let line_count = lines.len();
        std::iter::once(ContentRow::section(header))
            .chain(
                error
                    .into_iter()
                    .map(|row| ContentRow::new(row, ACCOUNT_SPACING)),
            )
            .chain(lines.into_iter().enumerate().map(|(index, line)| {
                let space_above = if index == 0 {
                    ACCOUNT_SPACING
                } else {
                    rems(0.)
                };
                ContentRow::new(line, space_above)
            }))
            .chain(cards.into_iter().enumerate().map(|(index, card)| {
                let space_above = if index == 0 && line_count > 0 {
                    SECTION_SPACING
                } else {
                    ACCOUNT_SPACING
                };
                ContentRow::new(card, space_above)
            }))
            .collect()
    }

    /// Several accounts as lines, the Usage page's: a head naming the windows, then a line per
    /// account with its avatar, name, tags, email and plan, and a cell per window. A line opens
    /// the account's whole card in a dialog. Each line is a row of the page's list, drawn as
    /// part of one table.
    fn render_account_lines(
        &self,
        listed: &[Option<AccountId>],
        accounts: &AgentAccounts,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(panel) = self.agent_panel() else {
            return Vec::new();
        };
        let colors = cx.theme().colors().clone();
        let agent_name = panel.external.connection.read(cx).agent_name().clone();
        let entries: Vec<AccountEntry> = listed
            .iter()
            .map(|&account| account_entry(accounts, account))
            .collect();
        let mut columns: Vec<String> = Vec::new();
        for window in entries
            .iter()
            .filter(|entry| !entry.is_logged_out)
            .flat_map(|entry| &entry.windows)
        {
            if !columns.contains(&window.label) {
                columns.push(window.label.clone());
            }
        }
        let column = |child: Option<AnyElement>| {
            div()
                .w(USAGE_COLUMN_WIDTH)
                .flex_none()
                .min_w_0()
                .children(child)
        };
        let head = h_flex()
            .px_4()
            .py_2()
            .gap(USAGE_COLUMN_GAP)
            .child(
                div().flex_1().min_w_0().child(
                    Label::new("Account")
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                ),
            )
            .children(columns.iter().map(|label| {
                column(Some(
                    Label::new(label.clone())
                        .size(LabelSize::XSmall)
                        .color(Color::Muted)
                        .truncate()
                        .into_any_element(),
                ))
            }))
            .child(div().w(px(16.)).flex_none());
        let now = SystemTime::now();
        let default = accounts.new_thread_account();
        let count = entries.len();
        let lines = entries.into_iter().enumerate().map(|(index, entry)| {
            let account = entry.account;
            let selector = account_selector(account);
            let key = format!("line-{selector}");
            let state = panel
                .session(account)
                .map(|session| AccountState::of(session.connection.read(cx)));
            let detail: (SharedString, Color) = match state {
                Some(AccountState::Failed) => {
                    (format!("Couldn't start {agent_name}").into(), Color::Error)
                }
                Some(AccountState::LoggedOut) => ("Not logged in".into(), Color::Warning),
                _ if entry.is_logged_out => ("Not logged in".into(), Color::Warning),
                _ => (
                    entry
                        .email
                        .iter()
                        .filter(|&email| *email != entry.name.as_ref())
                        .chain(&entry.plan)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(" · ")
                        .into(),
                    Color::Muted,
                ),
            };
            let windows = if entry.is_logged_out || state == Some(AccountState::LoggedOut) {
                &[][..]
            } else {
                &entry.windows[..]
            };
            let line_selector = format!("account-line-{selector}");
            let line = h_flex()
                .id(SharedString::from(line_selector.clone()))
                .debug_selector(move || line_selector)
                .px_4()
                .py(px(10.))
                .gap(USAGE_COLUMN_GAP)
                .cursor_pointer()
                .hover(|line| line.bg(colors.element_hover))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_3()
                        .child(render_entry_avatar(&entry, LINE_AVATAR_SIZE, cx))
                        .child(
                            v_flex()
                                .min_w_0()
                                .gap_px()
                                .child(
                                    h_flex()
                                        .min_w_0()
                                        .gap_2()
                                        .child(
                                            div()
                                                .min_w_0()
                                                .child(Label::new(entry.name.clone()).truncate()),
                                        )
                                        .when(account.is_none(), |name| {
                                            name.child(account_tag(
                                                "Outside agentZ",
                                                Color::Muted,
                                                cx,
                                            ))
                                        })
                                        .when(default == account, |name| {
                                            name.child(account_tag("Default", Color::Accent, cx))
                                        }),
                                )
                                .when(!detail.0.is_empty(), |column| {
                                    column.child(
                                        Label::new(detail.0)
                                            .size(LabelSize::XSmall)
                                            .color(detail.1)
                                            .truncate(),
                                    )
                                }),
                        ),
                )
                .children(columns.iter().enumerate().map(|(index, label)| {
                    column(
                        windows
                            .iter()
                            .find(|window| window.label == *label)
                            .map(|window| render_limit_cell(&key, index, window, now, cx)),
                    )
                }))
                .child(
                    div().w(px(16.)).flex_none().child(
                        Icon::new(IconName::ChevronRight)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    ),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_account_dialog(DialogContent::Account(account), window, cx)
                }));
            let is_last = index + 1 == count;
            div()
                .border_x_1()
                .border_color(colors.border)
                .bg(colors.panel_background)
                .when(is_last, |line| {
                    line.border_b_1().rounded_b_lg().overflow_hidden()
                })
                .child(
                    div()
                        .border_t_1()
                        .border_color(colors.border_variant)
                        .child(line),
                )
                .into_any_element()
        });
        std::iter::once(
            div()
                .debug_selector(|| "account-lines".into())
                .border_1()
                .border_b_0()
                .border_color(colors.border)
                .rounded_t_lg()
                .bg(colors.panel_background)
                .child(head)
                .into_any_element(),
        )
        .chain(lines)
        .collect()
    }

    /// An account as today's Account card shows the agent's login: its avatar in its color,
    /// its name with its tags, its plan, its limits, and its ⋯ menu. Logged out, its login
    /// rows follow; before its first login, it's the "New account" card. In its dialog, it's
    /// the dialog's whole content, with × to close it.
    #[allow(clippy::too_many_arguments)]
    fn render_account_card(
        &self,
        account: Option<AccountId>,
        accounts: &AgentAccounts,
        support: &AccountSupport,
        listed_count: usize,
        place: CardPlace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(panel) = self.agent_panel() else {
            return div().into_any_element();
        };
        // Its session opens as soon as the page learns of the account.
        let Some(session) = panel.session(account) else {
            return div().into_any_element();
        };
        let colors = cx.theme().colors().clone();
        let selector = account.map_or_else(|| "external".to_string(), |id| id.to_string());
        let connection = session.connection.read(cx);
        let agent_name = connection.agent_name().clone();
        let state = AccountState::of(connection);
        let has_auth_methods = !connection.auth_methods().is_empty();
        let auth_status = connection
            .auth_status()
            .filter(|status| status.is_logged_in())
            .cloned();
        let auth_error = connection.auth_error().cloned();
        let failure = match connection.status() {
            ConnectionStatus::Failed(error) => Some(error.clone()),
            _ => None,
        };
        let can_log_out = connection.supports_logout() || accounts.logs_in_with_key(account);
        let login_method = match account {
            None => {
                connection
                    .client()
                    .read(cx)
                    .agent_settings(&panel.agent_id.0)
                    .login_method
            }
            Some(id) => accounts
                .account(id)
                .and_then(|account| account.settings.login_method.clone()),
        };
        let read = accounts.status(account);
        let choices = accounts.choices(account).cloned().unwrap_or_default();
        let color = choices
            .color
            .as_deref()
            .and_then(|hex| account_color(hex, cx));
        let reported_email = auth_status
            .as_ref()
            .and_then(|status| status.account.as_ref())
            .and_then(|account| account.email.clone());
        let email = read
            .and_then(|read| read.status.email.clone())
            .or(reported_email);
        let name = accounts.name(account).or_else(|| email.clone());
        let card = v_flex()
            .id(SharedString::from(format!("account-card-{selector}")))
            .debug_selector({
                let selector = selector.clone();
                move || format!("account-card-{selector}")
            })
            .when(place == CardPlace::Page, |card| {
                card.rounded_lg()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.panel_background)
                    .overflow_hidden()
            });
        let close = (place == CardPlace::Dialog).then(|| {
            div()
                .debug_selector(|| "account-dialog-close".into())
                .child(
                    IconButton::new("account-dialog-close", IconName::Close)
                        .icon_size(IconSize::Small)
                        .icon_color(Color::Muted)
                        .tooltip(Tooltip::text("Close"))
                        .on_click(cx.listener(|this, _, _, cx| this.close_account_dialog(cx))),
                )
                .into_any_element()
        });

        let is_new = state != AccountState::LoggedIn
            && name.is_none()
            && read.is_none()
            && login_method.is_none();
        if let Some(id) = account.filter(|_| is_new) {
            let subtitle = match (failure.clone(), auth_error) {
                (Some(error), _) | (None, Some(error)) => (error, Color::Error),
                (None, None) if state == AccountState::Connecting => {
                    (format!("Starting {agent_name}…").into(), Color::Muted)
                }
                (None, None) => (
                    format!(
                        "Choose how {agent_name} logs in. The account takes its email once \
                         it's logged in."
                    )
                    .into(),
                    Color::Muted,
                ),
            };
            let new_avatar = div()
                .size(AVATAR_SIZE)
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_dashed()
                .border_color(colors.border)
                .child(
                    Icon::new(IconName::Plus)
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                );
            let mut actions = Vec::new();
            if failure.is_some() {
                actions.push(
                    ActionButton::new(
                        SharedString::from(format!("account-retry-{id}")),
                        "Try Again",
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.reopen_agent(window, cx)))
                    .into_any_element(),
                );
            }
            actions.push(
                div()
                    .debug_selector(move || format!("account-cancel-{id}"))
                    .child(
                        ActionButton::new(
                            SharedString::from(format!("account-cancel-{id}")),
                            "Cancel",
                        )
                        .style(ActionStyle::Ghost)
                        // Nothing is in its folder yet, so it goes without asking.
                        .on_click(cx.listener(move |this, _, _, cx| this.remove_account(id, cx))),
                    )
                    .into_any_element(),
            );
            actions.extend(close);
            return card
                .child(
                    h_flex()
                        .px_4()
                        .py_3()
                        .gap_3()
                        .child(new_avatar)
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap_0p5()
                                .child(Label::new("New account"))
                                .child(
                                    Label::new(subtitle.0)
                                        .size(LabelSize::Small)
                                        .color(subtitle.1),
                                ),
                        )
                        .children(actions),
                )
                .when(listed_count > 1, |card| {
                    card.child(self.render_copy_settings(
                        id,
                        accounts,
                        support,
                        CardPlace::Page,
                        window,
                        cx,
                    ))
                })
                .when(has_auth_methods && failure.is_none(), |card| {
                    card.child(session.login.clone())
                })
                .into_any_element();
        }

        let title: SharedString = match &name {
            Some(name) => name.clone().into(),
            None if state == AccountState::LoggedOut => "Not logged in".into(),
            None => logged_in_title(auth_status.as_ref(), login_method.as_deref()),
        };
        let detail: Option<(SharedString, Color)> = match state {
            AccountState::Failed => Some((
                failure.unwrap_or_else(|| format!("Couldn't start {agent_name}").into()),
                Color::Error,
            )),
            AccountState::LoggedOut => match auth_error {
                Some(error) => Some((error, Color::Error)),
                None => name
                    .is_some()
                    .then(|| ("Not logged in".into(), Color::Warning)),
            },
            AccountState::Connecting if read.is_none() && auth_status.is_none() => Some((
                format!("Checking whether {agent_name} is logged in…").into(),
                Color::Muted,
            )),
            _ => {
                let details = account_card_details(
                    &choices,
                    email.as_deref(),
                    read.map(|read| &read.status),
                    auth_status.as_ref(),
                );
                (!details.is_empty()).then(|| (details.join(" · ").into(), Color::Muted))
            }
        };

        let mut tags: Vec<AnyElement> = Vec::new();
        if account.is_none() {
            let machine = self.machines.read(cx).label(self.agents_machine, cx);
            let explanation = format!(
                "{agent_name}'s own login on {machine}, from its CLI. Threads on it go into the \
                 CLI's history{}",
                if can_log_out {
                    ", and logging out here logs out the CLI too."
                } else {
                    "."
                }
            );
            tags.push(
                account_tag("Outside agentZ", Color::Muted, cx)
                    .id("account-tag-outside")
                    .debug_selector(|| "account-tag-outside".into())
                    .tooltip(Tooltip::element(move |_, _| {
                        div()
                            .w(px(320.))
                            .child(Label::new(explanation.clone()).size(LabelSize::Small))
                            .into_any_element()
                    }))
                    .into_any_element(),
            );
        }
        let is_default = listed_count > 1 && accounts.new_thread_account() == account;
        if is_default {
            let selector = selector.clone();
            tags.push(
                account_tag("Default", Color::Accent, cx)
                    .debug_selector(move || format!("account-tag-default-{selector}"))
                    .into_any_element(),
            );
        }

        let is_renaming = panel
            .renaming
            .as_ref()
            .filter(|rename| rename.account == account);
        let title_element = match is_renaming {
            Some(rename) => div()
                .w(px(240.))
                .key_context(ACCOUNT_RENAME_KEY_CONTEXT)
                .on_action(cx.listener(|this, _: &menu::Confirm, window, cx| {
                    this.finish_account_rename(true, cx);
                    this.focus_after_rename(window, cx);
                }))
                .on_action(cx.listener(|this, _: &menu::Cancel, window, cx| {
                    this.finish_account_rename(false, cx);
                    this.focus_after_rename(window, cx);
                }))
                .px_1()
                .rounded_sm()
                .border_1()
                .border_color(colors.border_focused)
                .child(rename.input.clone())
                .into_any_element(),
            None => div()
                .min_w_0()
                .child(Label::new(title.clone()).truncate())
                .into_any_element(),
        };
        let avatar = match &name {
            Some(name) => avatar(name, color, AVATAR_SIZE, cx),
            None => account_badge(AVATAR_SIZE, cx),
        };

        let mut actions: Vec<AnyElement> = Vec::new();
        if state == AccountState::Failed {
            actions.push(
                ActionButton::new(
                    SharedString::from(format!("account-retry-{selector}")),
                    "Try Again",
                )
                .on_click(cx.listener(|this, _, window, cx| this.reopen_agent(window, cx)))
                .into_any_element(),
            );
        }
        let shows_read = matches!(state, AccountState::LoggedIn | AccountState::Connecting);
        if let Some(read) = read.filter(|_| shows_read) {
            let read_selector = format!("account-read-{selector}");
            actions.push(
                div()
                    .debug_selector(move || read_selector)
                    .flex_none()
                    .child(
                        Label::new(read_ago(read.read_at, SystemTime::now()))
                            .size(LabelSize::XSmall)
                            .color(Color::Placeholder),
                    )
                    .into_any_element(),
            );
        }
        let is_local = self.agents_machine == MachineId::Local;
        let menu = AccountMenu {
            account,
            name: name.map_or_else(|| title.clone(), Into::into),
            can_make_default: listed_count > 1 && accounts.new_thread_account() != account,
            color: choices.color,
            can_refresh: support.reads_usage && state == AccountState::LoggedIn,
            read_at: read.map(|read| read.read_at),
            usage_page: support.usage_page.clone(),
            folder: account
                .filter(|_| is_local)
                .map(|account| support.home(account)),
            can_log_out: can_log_out && state == AccountState::LoggedIn,
        };
        actions.push(render_account_menu(&selector, menu, cx));
        actions.extend(close);

        let status = read
            .map(|read| read.status.clone())
            .filter(|_| matches!(state, AccountState::LoggedIn | AccountState::Connecting));
        let limits = status
            .as_ref()
            .filter(|status| !status.windows.is_empty())
            .map(|status| {
                let tab = panel.limit_tabs.get(&account).copied().unwrap_or_default();
                self.render_limits(&selector, account, status, tab, cx)
            });
        let limit_resets = status.as_ref().and_then(|status| {
            let resets = status.limit_resets?;
            let (name, agent_name, status) = (title.clone(), agent_name.clone(), status.clone());
            Some(render_limit_resets(
                &selector,
                resets,
                panel.using_limit_reset.contains(&account),
                SystemTime::now(),
                cx.listener(move |this, _, _, cx| {
                    this.confirm_limit_reset(account, &name, &agent_name, &status, cx)
                }),
            ))
        });
        let extra_usage = status.as_ref().and_then(|status| {
            Some(render_extra_usage(
                &selector,
                status.extra_usage.as_ref()?,
                support.usage_page.clone(),
            ))
        });
        let overage = status.as_ref().and_then(|status| {
            let overage = status.overage?;
            Some(self.render_overage(
                account,
                overage,
                status.credits.as_deref(),
                support.extra_usage_page.clone(),
                panel.switching_to_core.contains(&account),
                window,
                cx,
            ))
        });
        let shows_login = account.is_some()
            && has_auth_methods
            && matches!(state, AccountState::LoggedOut | AccountState::LoggingIn);
        // Continuing at the reset needs the reset, from the account's limits. Beside the
        // agent's own choice, it's what agentZ does once the agent stops.
        let at_limit_title = if overage.is_some() {
            format!("When {agent_name} stops at a limit")
        } else {
            "When a limit is reached".to_string()
        };
        let at_limit = support
            .reads_usage
            .then(|| self.render_at_limit(account, choices.at_limit, at_limit_title, window, cx));
        let settings: Vec<AnyElement> = overage.into_iter().chain(at_limit).collect();

        card.child(
            h_flex()
                .px_4()
                .py_3()
                .gap_3()
                .child(avatar)
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_0p5()
                        .child(
                            h_flex()
                                .min_w_0()
                                .gap_2()
                                .child(title_element)
                                .children(tags),
                        )
                        .children(detail.map(|(detail, color)| {
                            Label::new(detail)
                                .size(LabelSize::Small)
                                .color(color)
                                .truncate()
                        })),
                )
                .children(actions),
        )
        .when(
            limits.is_some() || limit_resets.is_some() || extra_usage.is_some(),
            |card| {
                // Under the name, past the avatar.
                card.child(
                    v_flex()
                        .pl(px(16.) + AVATAR_SIZE + px(12.))
                        .pr_4()
                        .pb(px(14.))
                        .gap_3()
                        .children(limits)
                        .children(limit_resets)
                        .children(extra_usage),
                )
            },
        )
        .when(!settings.is_empty(), |card| {
            card.child(
                v_flex()
                    .py(px(6.))
                    .border_t_1()
                    .border_color(colors.border_variant)
                    .children(settings),
            )
        })
        .when(shows_login, |card| card.child(session.login.clone()))
        .into_any_element()
    }

    /// After a rename, focus goes back where the card is: its dialog, or the page.
    fn focus_after_rename(&self, window: &mut Window, cx: &mut Context<Self>) {
        let dialog = self
            .agent_panel()
            .and_then(|panel| panel.dialog.as_ref())
            .and_then(|dialog| dialog.view.upgrade());
        match dialog {
            Some(dialog) => window.focus(&dialog.focus_handle(cx), cx),
            None => window.focus(&self.focus_handle, cx),
        }
    }

    /// The account's windows. An account with pools of limits has a tab for each over them, as
    /// Droid's `/limits` does, and one for its extra usage balance when it can turn it on.
    fn render_limits(
        &self,
        selector: &str,
        account: Option<AccountId>,
        status: &AccountStatus,
        tab: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let now = SystemTime::now();
        let Some(pool) = &status.pool else {
            return render_limit_windows(selector, &status.windows, now, cx);
        };
        let mut tabs: Vec<(SharedString, Option<&[LimitWindow]>)> =
            vec![(pool.clone().into(), Some(&status.windows))];
        tabs.extend(
            status
                .other_pools
                .iter()
                .map(|pool| (pool.label.clone().into(), Some(&pool.windows[..]))),
        );
        if status.overage.is_some_and(|overage| overage.can_change) {
            tabs.push(("Extra usage".into(), None));
        }
        let tab = tab.min(tabs.len() - 1);
        let shown = match tabs[tab].1 {
            Some(windows) if tab == 0 => render_limit_windows(selector, windows, now, cx),
            Some(windows) => render_limit_windows(&format!("{selector}-{tab}"), windows, now, cx),
            None => render_balance(selector, status.credits.as_deref()),
        };
        let buttons: Vec<ToggleButtonSimple> = tabs
            .iter()
            .enumerate()
            .map(|(index, (label, _))| {
                ToggleButtonSimple::new(
                    label.clone(),
                    cx.listener(move |this, _, _, cx| {
                        if let Some(panel) = this.agent_panel_mut() {
                            panel.limit_tabs.insert(account, index);
                            cx.notify();
                        }
                    }),
                )
            })
            .collect();
        let id = format!("limit-tabs-{selector}");
        v_flex()
            .gap_3()
            .children(render_limit_tabs(id, buttons, tab))
            .child(shown)
            .into_any_element()
    }

    /// Droid's "When limit is reached" (decisions.md §8), which it keeps on Factory's server.
    /// "Use extra usage" opens Factory's page, as Droid's own does: Droid never saves it.
    #[allow(clippy::too_many_arguments)]
    fn render_overage(
        &self,
        account: Option<AccountId>,
        overage: Overage,
        credits: Option<&str>,
        extra_usage_page: Option<String>,
        switching: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = account_selector(account);
        let current = match overage.preference {
            Some(OveragePreference::DroidCore) => "Switch to Droid Core",
            Some(OveragePreference::ExtraUsage) => "Use extra usage",
            None => "Not chosen",
        };
        let id = format!("overage-{selector}");
        let control = if !overage.can_change {
            Label::new(format!("{current}, set by your organization"))
                .size(LabelSize::Small)
                .color(Color::Muted)
                .into_any_element()
        } else {
            let page = cx.weak_entity();
            let extra_usage_page = extra_usage_page.filter(|_| overage.extra_usage_allowed);
            let balance = credits.map_or_else(String::new, |credits| format!(" ({credits})"));
            let menu = ContextMenu::build(window, cx, {
                move |menu, _, _| {
                    let chosen = overage.preference;
                    let core_selector = format!("overage-{selector}-droid-core");
                    let menu = menu.custom_entry(
                        move |_, _| {
                            let selector = core_selector.clone();
                            v_flex()
                                .w(px(280.))
                                .debug_selector(move || selector)
                                .child(render_check_entry(
                                    "Switch to Droid Core",
                                    chosen == Some(OveragePreference::DroidCore),
                                ))
                                .child(
                                    Label::new(
                                        "Keep working on Droid Core models, at no extra cost.",
                                    )
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                                )
                                .into_any_element()
                        },
                        on_page(&page, move |page, _, cx| {
                            page.switch_to_droid_core(account, cx)
                        }),
                    );
                    let Some(url) = extra_usage_page else {
                        return menu;
                    };
                    let selector = format!("overage-{selector}-extra-usage");
                    menu.custom_entry(
                        move |_, _| {
                            let selector = selector.clone();
                            v_flex()
                                .w(px(280.))
                                .debug_selector(move || selector)
                                .child(
                                    h_flex()
                                        .gap_1()
                                        .child(render_check_entry(
                                            "Use extra usage",
                                            chosen == Some(OveragePreference::ExtraUsage),
                                        ))
                                        .child(
                                            Icon::new(IconName::ArrowUpRight)
                                                .size(IconSize::XSmall)
                                                .color(Color::Muted),
                                        ),
                                )
                                .child(
                                    Label::new(format!(
                                        "Keep working on the same models, billed from your \
                                         extra usage balance{balance}. Turned on in Factory's \
                                         settings."
                                    ))
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                                )
                                .into_any_element()
                        },
                        move |_, cx| cx.open_url(&url),
                    )
                }
            });
            let label = if switching {
                "Switching to Droid Core…"
            } else {
                current
            };
            div()
                .flex_none()
                .debug_selector({
                    let id = id.clone();
                    move || id
                })
                .child(DropdownMenu::new(SharedString::from(id), label, menu).disabled(switching))
                .into_any_element()
        };
        render_card_setting("When a limit is reached", control)
    }

    /// The card's Switch to Droid Core, which takes a moment: Droid's terminal UI saves it.
    fn switch_to_droid_core(&mut self, account: Option<AccountId>, cx: &mut Context<Self>) {
        self.send_pending_account_request(
            account,
            |agent_id| Request::SwitchToDroidCore { agent_id, account },
            |panel| &mut panel.switching_to_core,
            "Couldn't switch to Droid Core",
            cx,
        );
    }

    /// The card's Use Reset, which asks first: a reset can't be given back.
    fn confirm_limit_reset(
        &mut self,
        account: Option<AccountId>,
        name: &str,
        agent_name: &str,
        status: &AccountStatus,
        cx: &mut Context<Self>,
    ) {
        let page = cx.weak_entity();
        let request = ConfirmRequest::use_limit_reset(name, agent_name, status, move |_, cx| {
            page.update(cx, |page, cx| page.use_limit_reset(account, cx))
                .log_err();
        });
        cx.emit(SettingsPageEvent::Confirm(request));
    }

    fn use_limit_reset(&mut self, account: Option<AccountId>, cx: &mut Context<Self>) {
        self.send_pending_account_request(
            account,
            |agent_id| Request::UseLimitReset { agent_id, account },
            |panel| &mut panel.using_limit_reset,
            "Couldn't use the limit reset",
            cx,
        );
    }

    /// An account's request that its card shows on its way (in `pending`), once at a time.
    fn send_pending_account_request(
        &mut self,
        account: Option<AccountId>,
        request: impl FnOnce(AgentId) -> Request,
        pending: fn(&mut AgentPanel) -> &mut HashSet<Option<AccountId>>,
        failure: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        if !pending(panel).insert(account) {
            return;
        }
        panel.account_error = None;
        let agent_id = panel.agent_id.clone();
        let response = panel.client(cx).read(cx).request(request(agent_id.clone()));
        cx.spawn(async move |this, cx| {
            let result = response.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this
                    .agent_panel_mut()
                    .filter(|panel| panel.agent_id == agent_id)
                else {
                    return;
                };
                pending(panel).remove(&account);
                if let Err(error) = result {
                    panel.account_error = Some(format!("{failure}: {error:#}").into());
                }
                cx.notify();
            })
            .log_err();
        })
        .detach();
        cx.notify();
    }

    /// "When a limit is reached": what every thread on the account does, Stop or Continue at
    /// reset, with what each means in the menu.
    fn render_at_limit(
        &self,
        account: Option<AccountId>,
        current: AtLimit,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        const CHOICES: [(AtLimit, &str, &str); 2] = [
            (AtLimit::Stop, "Stop", "The thread waits for you."),
            (
                AtLimit::ContinueAtReset,
                "Continue at reset",
                "agentZ sends “Continue.” when the limit resets.",
            ),
        ];
        let label = CHOICES
            .iter()
            .find(|(choice, ..)| *choice == current)
            .map_or("Stop", |(_, name, _)| *name);
        let selector = account_selector(account);
        let page = cx.weak_entity();
        let menu = ContextMenu::build(window, cx, {
            let selector = selector.clone();
            move |mut menu, _, _| {
                for (choice, name, description) in CHOICES {
                    let selector = format!("at-limit-{selector}-{choice:?}");
                    menu = menu.custom_entry(
                        move |_, _| {
                            let selector = selector.clone();
                            v_flex()
                                .w(px(260.))
                                .debug_selector(move || selector)
                                .child(render_check_entry(name, choice == current))
                                .child(
                                    Label::new(description)
                                        .size(LabelSize::Small)
                                        .color(Color::Muted),
                                )
                                .into_any_element()
                        },
                        on_page(&page, move |page, _, cx| {
                            page.update_account(account, AccountChange::SetAtLimit(choice), cx)
                        }),
                    );
                }
                menu
            }
        });
        let id = format!("at-limit-{selector}");
        render_card_setting(
            title,
            div()
                .debug_selector({
                    let id = id.clone();
                    move || id
                })
                .child(DropdownMenu::new(SharedString::from(id), label, menu))
                .into_any_element(),
        )
    }

    /// A new account's "Copy settings from": the other accounts, the default one first, then
    /// Nothing. In Add Account's dialog, it's under the login methods, in the dialog's margins.
    fn render_copy_settings(
        &self,
        account: AccountId,
        accounts: &AgentAccounts,
        support: &AccountSupport,
        place: CardPlace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let agent_name = self
            .agent_panel()
            .map(|panel| panel.external.connection.read(cx).agent_name().clone())
            .unwrap_or_default();
        let current = accounts
            .account(account)
            .map(|account| account.settings_from)
            .unwrap_or_default();
        let default = accounts.new_thread_account();
        let mut entries: Vec<AccountEntry> = account_entries(accounts)
            .into_iter()
            .filter(|entry| entry.account != Some(account))
            .collect();
        entries.sort_by_key(|entry| entry.account != default);
        let source = |entry: &AccountEntry| match entry.account {
            None => SettingsSource::External,
            Some(id) => SettingsSource::Account(id),
        };
        let label = match entries.iter().find(|entry| source(entry) == current) {
            Some(entry) => h_flex()
                .gap_1p5()
                .child(render_entry_avatar(entry, MENU_AVATAR_SIZE, cx))
                .child(Label::new(entry.name.clone()))
                .into_any_element(),
            None => Label::new("Nothing").into_any_element(),
        };
        let page = cx.weak_entity();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for entry in entries {
                let from = source(&entry);
                let selector = account_selector(entry.account);
                menu = menu.custom_entry(
                    move |_, cx| {
                        let selector = selector.clone();
                        div()
                            .w_full()
                            .debug_selector(move || format!("copy-settings-from-{selector}"))
                            .child(render_account_entry(&entry, from == current, cx))
                            .into_any_element()
                    },
                    on_page(&page, move |page, _, cx| {
                        page.copy_account_settings(account, from, cx)
                    }),
                );
            }
            menu.separator().custom_entry(
                move |_, _| {
                    div()
                        .w_full()
                        .debug_selector(|| "copy-settings-from-nothing".into())
                        .child(render_check_entry(
                            "Nothing",
                            current == SettingsSource::Nothing,
                        ))
                        .into_any_element()
                },
                on_page(&page, move |page, _, cx| {
                    page.copy_account_settings(account, SettingsSource::Nothing, cx)
                }),
            )
        });
        let what = if support.copies_settings_files {
            format!(
                "Its defaults, environment variables and {agent_name}'s own settings, but not \
                 its login."
            )
        } else {
            "Its defaults and environment variables.".to_string()
        };
        let description = format!("{what} Defaults this account doesn't offer are dropped.");
        let control = div()
            .debug_selector(move || format!("copy-settings-{account}"))
            .child(DropdownMenu::new_with_element(
                SharedString::from(format!("copy-settings-{account}")),
                label,
                menu,
            ))
            .into_any_element();
        let border = cx.theme().colors().border_variant;
        match place {
            CardPlace::Page => div()
                .border_t_1()
                .border_color(border)
                .child(render_row("Copy settings from", description, control, cx))
                .into_any_element(),
            CardPlace::Dialog => h_flex()
                .mt_2()
                .pt_2p5()
                .gap_4()
                .border_t_1()
                .border_color(border)
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_px()
                        .child(Label::new("Copy settings from").size(LabelSize::Small))
                        .child(
                            Label::new(description)
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                        ),
                )
                .child(div().flex_none().child(control))
                .into_any_element(),
        }
    }

    /// Copy settings from, then the account's agent starts again with them, to log in with its
    /// variables.
    fn copy_account_settings(
        &mut self,
        account: AccountId,
        from: SettingsSource,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        panel.account_error = None;
        let agent_id = panel.agent_id.clone();
        let response = panel
            .client(cx)
            .read(cx)
            .request(Request::CopyAccountSettings {
                agent_id: agent_id.clone(),
                account,
                from,
            });
        cx.spawn(async move |this, cx| {
            let copied = response.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this
                    .agent_panel_mut()
                    .filter(|panel| panel.agent_id == agent_id)
                else {
                    return;
                };
                match copied {
                    Ok(_) => this.restart_account_session(account, cx),
                    Err(error) => {
                        panel.account_error =
                            Some(format!("Couldn't copy the settings: {error:#}").into());
                    }
                }
                cx.notify();
            })
            .log_err();
        })
        .detach();
        cx.notify();
    }

    /// Add Account opens its dialog, and the server makes the account's folder. The account
    /// arrives with the agent's accounts, which opens its login session, and the dialog logs
    /// it in.
    fn add_account(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        if panel.adding_account.is_some() || panel.dialog.is_some() {
            return;
        }
        let agent_id = panel.agent_id.clone();
        let client = panel.external.connection.read(cx).client().clone();
        let request = client
            .read(cx)
            .request(Request::AddAccount(agent_id.clone()));
        let task = cx.spawn(async move |this, cx| {
            let added = request.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this
                    .agent_panel_mut()
                    .filter(|panel| panel.agent_id == agent_id)
                else {
                    return;
                };
                panel.adding_account = None;
                let id = match added {
                    Ok(Response::AccountAdded(id)) => id,
                    Ok(response) => {
                        log::error!("unexpected answer to Add Account: {response:?}");
                        return;
                    }
                    Err(error) => {
                        panel.account_error =
                            Some(format!("Couldn't add an account: {error:#}").into());
                        this.close_account_dialog(cx);
                        cx.notify();
                        return;
                    }
                };
                match panel.adding_mut() {
                    Some(adding) => adding.account = Some(id),
                    // The dialog closed before the account was made.
                    None => {
                        this.remove_account(id, cx);
                        return;
                    }
                }
                this.sync_adding_login(cx);
                cx.notify();
            })
            .log_err();
        });
        if let Some(panel) = self.agent_panel_mut() {
            panel.adding_account = Some(task);
            panel.account_error = None;
        }
        self.open_account_dialog(
            DialogContent::Adding(AddingAccount {
                account: None,
                login: None,
            }),
            window,
            cx,
        );
    }

    /// Opens the account dialog over the page, unless one is open.
    fn open_account_dialog(
        &mut self,
        content: DialogContent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .agent_panel()
            .is_none_or(|panel| panel.dialog.is_some())
        {
            return;
        }
        let page = cx.weak_entity();
        let dialog = cx.new(|cx| AccountDialog {
            page,
            focus_handle: cx.focus_handle(),
        });
        window.focus(&dialog.focus_handle(cx), cx);
        let released = cx.observe_release(&dialog, |this, _, cx| this.account_dialog_closed(cx));
        if let Some(panel) = self.agent_panel_mut() {
            panel.dialog = Some(AccountDialogState {
                view: dialog.downgrade(),
                content,
                _released: released,
            });
        }
        cx.emit(SettingsPageEvent::OpenDialog(dialog));
        cx.notify();
    }

    /// Done, Cancel and the dialog's ×: the shell closes it, which tells the page.
    fn close_account_dialog(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self
            .agent_panel()
            .and_then(|panel| panel.dialog.as_ref())
            .and_then(|dialog| dialog.view.upgrade())
        {
            view.update(cx, |_, cx| cx.emit(DismissEvent));
        }
    }

    /// An account added for a login that didn't finish is empty, so it goes with the dialog.
    fn account_dialog_closed(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        let Some(dialog) = panel.dialog.take() else {
            return;
        };
        cx.notify();
        let DialogContent::Adding(AddingAccount {
            account: Some(id), ..
        }) = dialog.content
        else {
            return;
        };
        let accounts = panel.client(cx).read(cx).accounts(&panel.agent_id);
        let is_logged_in = panel.session(Some(id)).is_some_and(|session| {
            AccountState::of(session.connection.read(cx)) == AccountState::LoggedIn
        });
        if accounts.account(id).is_some() && !is_logged_in {
            self.remove_account(id, cx);
        }
    }

    /// Gives Add Account's dialog the new account's login, once both the account and its
    /// session are there.
    fn sync_adding_login(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        let agent_id = panel.agent_id.clone();
        let Some(id) = panel
            .adding_mut()
            .filter(|adding| adding.login.is_none())
            .and_then(|adding| adding.account)
        else {
            return;
        };
        let Some(connection) = panel
            .accounts
            .get(&id)
            .map(|session| session.connection.clone())
        else {
            return;
        };
        let login =
            cx.new(|cx| AgentLogin::new(connection, LoginLayout::Dialog, Some(agent_id), cx));
        if let Some(adding) = self.agent_panel_mut().and_then(AgentPanel::adding_mut) {
            adding.login = Some(login);
        }
        cx.notify();
    }

    /// Starts the account's agent again, in a new login session: after Copy settings from, or
    /// to try again once it failed to start.
    fn restart_account_session(&mut self, account: AccountId, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        panel.accounts.remove(&account);
        if let Some(adding) = panel
            .adding_mut()
            .filter(|adding| adding.account == Some(account))
        {
            adding.login = None;
        }
        self.sync_account_sessions(cx);
    }

    /// The account dialog's content, `None` once there's nothing for it to show.
    fn render_account_dialog(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let panel = self.agent_panel()?;
        let dialog = panel.dialog.as_ref()?;
        let support = self
            .registry(cx)
            .read(cx)
            .agent(&panel.agent_id)?
            .accounts
            .clone()?;
        let accounts = panel.client(cx).read(cx).accounts(&panel.agent_id);
        let frame = dialog_frame(cx);
        let elicitation_cards = |account: Option<AccountId>| -> Vec<AnyElement> {
            panel
                .session(account)
                .map(|session| {
                    session
                        .elicitation_cards
                        .iter()
                        .map(|card| card.clone().into_any_element())
                        .collect()
                })
                .unwrap_or_default()
        };
        match &dialog.content {
            DialogContent::Account(account) => {
                let account = *account;
                let listed = accounts.listed();
                if !listed.contains(&account) {
                    return None;
                }
                let card = self.render_account_card(
                    account,
                    &accounts,
                    &support,
                    listed.len(),
                    CardPlace::Dialog,
                    window,
                    cx,
                );
                Some(
                    frame
                        .debug_selector(|| "account-dialog".into())
                        .w(ACCOUNT_DIALOG_WIDTH)
                        .py_1()
                        .child(card)
                        .children(
                            elicitation_cards(account)
                                .into_iter()
                                .map(|card| div().px_4().pb_3().child(card)),
                        )
                        .into_any_element(),
                )
            }
            DialogContent::Adding(adding) => {
                let (title, body, buttons) =
                    self.render_adding_account(adding, &accounts, &support, window, cx);
                let elicitations = adding
                    .account
                    .map(|id| elicitation_cards(Some(id)))
                    .unwrap_or_default();
                Some(
                    frame
                        .debug_selector(|| "add-account-dialog".into())
                        .w(ADD_ACCOUNT_DIALOG_WIDTH)
                        .px_4()
                        .pt_4()
                        .pb_3p5()
                        .child(dialog_title(title))
                        .child(body)
                        .children(elicitations)
                        .child(h_flex().mt_3p5().gap_2().justify_end().children(buttons))
                        .into_any_element(),
                )
            }
        }
    }

    /// Add Account's dialog by its step: the agent starting, how to log in with Copy settings
    /// from, the login in progress or why it failed, then the account added, or the account it
    /// turned out to be already.
    fn render_adding_account(
        &self,
        adding: &AddingAccount,
        accounts: &AgentAccounts,
        support: &AccountSupport,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (SharedString, AnyElement, Vec<AnyElement>) {
        let Some(panel) = self.agent_panel() else {
            return ("".into(), div().into_any_element(), Vec::new());
        };
        let agent_name = panel.external.connection.read(cx).agent_name().clone();
        let adding_title: SharedString = format!("Add a {agent_name} account").into();
        let button = |id: &'static str, label: &'static str, style: ActionStyle| {
            ActionButton::new(id, label).style(style)
        };
        let wrap = |id: &'static str, button: ActionButton| {
            div()
                .debug_selector(move || id.into())
                .child(button)
                .into_any_element()
        };
        let done = |cx: &Context<Self>| {
            wrap(
                "add-account-done",
                button("add-account-done", "Done", ActionStyle::Primary)
                    .on_click(cx.listener(|this, _, _, cx| this.close_account_dialog(cx))),
            )
        };
        let cancel = |style: ActionStyle, cx: &Context<Self>| {
            wrap(
                "add-account-cancel",
                button("add-account-cancel", "Cancel", style)
                    .on_click(cx.listener(|this, _, _, cx| this.close_account_dialog(cx))),
            )
        };
        let centered = |children: Vec<AnyElement>| {
            v_flex()
                .py_4()
                .items_center()
                .gap_2p5()
                .text_center()
                .children(children)
                .into_any_element()
        };

        let Some(id) = adding.account else {
            let body = centered(vec![
                spinner(Color::Muted),
                Label::new(format!("Starting {agent_name}…"))
                    .color(Color::Muted)
                    .into_any_element(),
            ]);
            return (adding_title, body, vec![cancel(ActionStyle::Ghost, cx)]);
        };
        if let Some(duplicate) = accounts
            .duplicate
            .as_ref()
            .filter(|duplicate| duplicate.account == id)
        {
            let body = div()
                .debug_selector(|| "add-account-duplicate".into())
                .child(
                    Label::new(format!(
                        "{} is already one of {agent_name}'s accounts here, so nothing was added.",
                        duplicate.email
                    ))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
                )
                .into_any_element();
            return ("Account already added".into(), body, vec![done(cx)]);
        }
        let session = panel.session(Some(id));
        let state = session.map(|session| AccountState::of(session.connection.read(cx)));
        match state {
            Some(AccountState::LoggedIn) => {
                let body = self.render_account_added(id, accounts, cx);
                return ("Account added".into(), body, vec![done(cx)]);
            }
            Some(AccountState::Failed) => {
                let error = session
                    .and_then(|session| match session.connection.read(cx).status() {
                        ConnectionStatus::Failed(error) => Some(error.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| format!("Couldn't start {agent_name}").into());
                let body = centered(vec![
                    Icon::new(IconName::XCircle)
                        .size(IconSize::Medium)
                        .color(Color::Error)
                        .into_any_element(),
                    Label::new(format!("Couldn't start {agent_name}")).into_any_element(),
                    Label::new(error)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                        .into_any_element(),
                    wrap(
                        "add-account-retry",
                        button("add-account-retry", "Try Again", ActionStyle::Outline).on_click(
                            cx.listener(move |this, _, _, cx| this.restart_account_session(id, cx)),
                        ),
                    ),
                ]);
                return (adding_title, body, vec![cancel(ActionStyle::Ghost, cx)]);
            }
            _ => {}
        }
        let login = adding
            .login
            .as_ref()
            .filter(|_| state.is_some_and(|state| state != AccountState::Connecting));
        let Some(login) = login else {
            let body = centered(vec![
                spinner(Color::Muted),
                Label::new(format!("Starting {agent_name}…"))
                    .color(Color::Muted)
                    .into_any_element(),
            ]);
            return (adding_title, body, vec![cancel(ActionStyle::Ghost, cx)]);
        };
        let step = login.read(cx).step(cx);
        let mut body = v_flex();
        if step == LoginStep::Choosing {
            body = body.child(
                div().mb_2p5().child(
                    Label::new(
                        "Each account has its own login, sessions and history. Choose how to \
                         log in.",
                    )
                    .size(LabelSize::Small)
                    .color(Color::Muted),
                ),
            );
        }
        body = body.child(login.clone());
        let has_others = accounts.listed().iter().any(|&account| account != Some(id));
        if step == LoginStep::Choosing && has_others {
            body = body.child(self.render_copy_settings(
                id,
                accounts,
                support,
                CardPlace::Dialog,
                window,
                cx,
            ));
        }
        let back = || {
            let login = login.clone();
            wrap(
                "add-account-back",
                button("add-account-back", "Back", ActionStyle::Ghost)
                    .on_click(move |_, _, cx| login.update(cx, |login, cx| login.back(cx))),
            )
        };
        let buttons = match step {
            LoginStep::Choosing => vec![cancel(ActionStyle::Ghost, cx)],
            LoginStep::InProgress | LoginStep::Failed => {
                vec![back(), cancel(ActionStyle::Outline, cx)]
            }
            LoginStep::Entering { can_submit } => {
                let login = login.clone();
                vec![
                    back(),
                    cancel(ActionStyle::Outline, cx),
                    wrap(
                        "add-account-log-in",
                        button("add-account-log-in", "Log In", ActionStyle::Primary)
                            .disabled(!can_submit)
                            .on_click(move |_, _, cx| {
                                login.update(cx, |login, cx| login.submit(cx))
                            }),
                    ),
                ]
            }
        };
        (adding_title, body.into_any_element(), buttons)
    }

    /// Add Account's result: the account named by its email, its plan and limits, and where
    /// its settings came from.
    fn render_account_added(
        &self,
        id: AccountId,
        accounts: &AgentAccounts,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let entry = account_entry(accounts, Some(id));
        let detail = entry
            .email
            .iter()
            .filter(|&email| *email != entry.name.as_ref())
            .chain(&entry.plan)
            .cloned()
            .collect::<Vec<_>>()
            .join(" · ");
        let key = format!("added-{id}");
        let windows = (!entry.windows.is_empty())
            .then(|| render_limit_windows(&key, &entry.windows, SystemTime::now(), cx));
        let copied_from = accounts
            .account(id)
            .and_then(|account| match account.settings_from {
                SettingsSource::Nothing => None,
                SettingsSource::External => Some(None),
                SettingsSource::Account(from) => Some(Some(from)),
            })
            .map(|from| account_entry(accounts, from).name);
        let note = match copied_from {
            Some(from) => {
                format!("Its settings were copied from {from}. You can rename it from its ⋯ menu.")
            }
            None => "You can rename it from its ⋯ menu.".to_string(),
        };
        v_flex()
            .debug_selector(|| "add-account-added".into())
            .pt_1p5()
            .gap_3()
            .child(
                v_flex()
                    .rounded_lg()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.panel_background)
                    .child(
                        h_flex()
                            .px_3()
                            .py_2p5()
                            .gap_3()
                            .child(render_entry_avatar(&entry, px(24.), cx))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_px()
                                    .child(Label::new(entry.name.clone()).truncate())
                                    .when(!detail.is_empty(), |column| {
                                        column.child(
                                            Label::new(detail)
                                                .size(LabelSize::XSmall)
                                                .color(Color::Muted)
                                                .truncate(),
                                        )
                                    }),
                            ),
                    )
                    .children(windows.map(|windows| {
                        div()
                            .px_3()
                            .py_2p5()
                            .border_t_1()
                            .border_color(colors.border_variant)
                            .child(windows)
                    })),
            )
            .child(Label::new(note).size(LabelSize::Small).color(Color::Muted))
            .into_any_element()
    }

    /// Sends a request about the agent's accounts, to say why if it fails.
    fn send_account_request(
        &mut self,
        request: Request,
        failure: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        panel.account_error = None;
        let agent_id = panel.agent_id.clone();
        let response = panel
            .external
            .connection
            .read(cx)
            .client()
            .read(cx)
            .request(request);
        cx.spawn(async move |this, cx| {
            let Err(error) = response.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if let Some(panel) = this
                    .agent_panel_mut()
                    .filter(|panel| panel.agent_id == agent_id)
                {
                    panel.account_error = Some(format!("{failure}: {error:#}").into());
                    cx.notify();
                }
            })
            .log_err();
        })
        .detach();
        cx.notify();
    }

    fn update_account(
        &mut self,
        account: Option<AccountId>,
        change: AccountChange,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let request = Request::UpdateAccount {
            agent_id: panel.agent_id.clone(),
            account,
            change,
        };
        self.send_account_request(request, "Couldn't change the account", cx);
    }

    fn refresh_usage(&mut self, account: Option<AccountId>, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let request = Request::RefreshUsage {
            agent_id: panel.agent_id.clone(),
            account,
        };
        self.send_account_request(request, "Couldn't read the usage", cx);
    }

    fn remove_account(&mut self, account: AccountId, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let request = Request::RemoveAccount {
            agent_id: panel.agent_id.clone(),
            account,
        };
        self.send_account_request(request, "Couldn't remove the account", cx);
    }

    fn confirm_remove_account(
        &mut self,
        account: AccountId,
        name: SharedString,
        cx: &mut Context<Self>,
    ) {
        let page = cx.weak_entity();
        let request = ConfirmRequest::remove_account(&name, move |_, cx| {
            page.update(cx, |page, cx| page.remove_account(account, cx))
                .log_err();
        });
        cx.emit(SettingsPageEvent::Confirm(request));
    }

    /// Rename…: the card's name becomes a field, with the name it shows.
    fn start_account_rename(
        &mut self,
        account: Option<AccountId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let accounts = panel
            .external
            .connection
            .read(cx)
            .client()
            .read(cx)
            .accounts(&panel.agent_id);
        let name = accounts.name(account).unwrap_or_default();
        let input = cx.new(|cx| {
            let mut input = TextInput::new("Name", cx);
            input.set_text(name, cx);
            input.select_all_text(cx);
            input
        });
        if let Some(panel) = self.agent_panel_mut() {
            panel.renaming = Some(AccountRename {
                account,
                input,
                _blur: None,
            });
        }
        cx.notify();
        // The menu takes focus two frames after it opens, and gives it back as it closes: the
        // field takes it after both.
        let page = cx.weak_entity();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, _| {
                window.on_next_frame(move |window, cx| {
                    page.update(cx, |page, cx| page.focus_account_rename(window, cx))
                        .log_err();
                });
            });
        });
    }

    fn focus_account_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(rename) = self
            .agent_panel_mut()
            .and_then(|panel| panel.renaming.as_mut())
        else {
            return;
        };
        let focus_handle = rename.input.focus_handle(cx);
        window.focus(&focus_handle, cx);
        rename._blur = Some(cx.on_blur(&focus_handle, window, |this, _, cx| {
            this.finish_account_rename(true, cx)
        }));
    }

    /// Enter or clicking elsewhere keeps the name; Escape doesn't. An empty name shows the
    /// email again.
    fn finish_account_rename(&mut self, keep: bool, cx: &mut Context<Self>) {
        let Some(rename) = self
            .agent_panel_mut()
            .and_then(|panel| panel.renaming.take())
        else {
            return;
        };
        cx.notify();
        if !keep {
            return;
        }
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let accounts = panel
            .external
            .connection
            .read(cx)
            .client()
            .read(cx)
            .accounts(&panel.agent_id);
        let name = rename.input.read(cx).text().trim().to_string();
        if accounts.name(rename.account).unwrap_or_default() == name {
            return;
        }
        let label = Some(name).filter(|name| !name.is_empty());
        self.update_account(rename.account, AccountChange::Rename(label), cx);
    }

    fn set_changing_account(&mut self, changing_account: bool, cx: &mut Context<Self>) {
        if let Some(panel) = self.agent_panel_mut() {
            panel.external.changing_account = changing_account;
        }
        cx.notify();
    }

    /// Starts the agent again for its page, on the tab that was open.
    fn reopen_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(account) = self.agent_panel() else {
            return;
        };
        let id = account.agent_id.clone();
        let tab = account.tab;
        let name = account.external.connection.read(cx).agent_name().clone();
        self.open_agent(&id, &name, window, cx);
        self.select_agent_tab(tab, cx);
    }

    /// Shows one of the agent page's tabs. Threads lists the account's sessions the first time,
    /// and again after the agent couldn't list them. Environment reads the account's variables
    /// again, which Copy settings from may have changed.
    fn select_agent_tab(&mut self, tab: AgentTab, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        panel.tab = tab;
        let needs_listing = match &panel.sessions {
            None | Some(SessionList::Failed(_)) => true,
            Some(SessionList::Listed(sessions)) => !matches!(sessions, AgentSessions::Listed(_)),
            Some(SessionList::Listing { .. }) => false,
        };
        let env_account = panel.env_account;
        if tab == AgentTab::Threads && needs_listing {
            self.list_agent_sessions(cx);
        }
        if tab == AgentTab::Environment {
            self.show_account_env(env_account, cx);
        }
        self.sync_settings_account(cx);
        cx.notify();
    }

    fn list_agent_sessions(&mut self, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let agent_id = panel.agent_id.clone();
        let client = panel.client(cx);
        let account = panel.settings_account(&client.read(cx).accounts(&agent_id));
        // While disconnected, the request fails and says so.
        let is_outdated = client.read(cx).connection().is_some()
            && !client.read(cx).has_capability(CAPABILITY_IMPORT_SESSIONS);
        if is_outdated {
            if let Some(panel) = self.agent_panel_mut() {
                panel.sessions_account = account;
                panel.sessions = Some(SessionList::Failed(
                    "This machine's agentz-server can't list threads. Update it to import them."
                        .into(),
                ));
            }
            cx.notify();
            return;
        }
        let listing =
            client
                .read(cx)
                .projects()
                .read(cx)
                .list_agent_sessions(agent_id.clone(), account, cx);
        let task = cx.spawn(async move |this, cx| {
            let listing = listing.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this.agent_panel_mut().filter(|panel| {
                    panel.agent_id == agent_id && panel.sessions_account == account
                }) else {
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
        if let Some(panel) = self.agent_panel_mut() {
            panel.sessions_account = account;
            panel.sessions = Some(SessionList::Listing { _task: task });
            panel.import_error = None;
        }
        cx.notify();
    }

    /// Adds an archived thread for each of the sessions.
    fn import_agent_sessions(&mut self, sessions: Vec<AgentSession>, cx: &mut Context<Self>) {
        let Some(panel) = self.agent_panel_mut() else {
            return;
        };
        let agent_id = panel.agent_id.clone();
        let account = panel.sessions_account;
        let session_ids: Vec<String> = sessions
            .iter()
            .map(|session| session.session_id.clone())
            .collect();
        panel.importing.extend(session_ids.iter().cloned());
        panel.import_error = None;
        let import = panel
            .client(cx)
            .read(cx)
            .projects()
            .read(cx)
            .import_agent_sessions(agent_id.clone(), account, sessions, cx);
        cx.spawn(async move |this, cx| {
            let imported = import.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this
                    .agent_panel_mut()
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
        let Some(panel) = self.agent_panel() else {
            return div().into_any_element();
        };
        let status_colors = cx.theme().status().clone();
        let connection = panel.external.connection.read(cx);
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
                        let thread = store.thread_for_session(
                            &panel.agent_id.0,
                            panel.sessions_account,
                            &session.session_id,
                        )?;
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
                            if let Some(panel) = page.agent_panel_mut() {
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
            .agent_panel()
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

    /// Asks first, as t3code does: logging out affects every thread on the account. An agent
    /// that can have more than one names the account (`account_name`).
    fn confirm_logout(
        &mut self,
        account: Option<AccountId>,
        account_name: Option<SharedString>,
        cx: &mut Context<Self>,
    ) {
        let Some(panel) = self.agent_panel() else {
            return;
        };
        let agent_name = panel.external.connection.read(cx).agent_name().clone();
        let page = cx.weak_entity();
        let log_out = move |_: &mut Window, cx: &mut App| {
            page.update(cx, |page, cx| {
                if let Some(session) = page
                    .agent_panel_mut()
                    .and_then(|panel| panel.session_mut(account))
                {
                    session.changing_account = false;
                    session
                        .connection
                        .update(cx, |connection, cx| connection.logout(cx));
                }
            })
            .log_err();
        };
        let request = match account_name {
            Some(name) => {
                ConfirmRequest::account_logout(&name, &agent_name, account.is_none(), log_out)
            }
            None => ConfirmRequest::logout(&agent_name, log_out),
        };
        cx.emit(SettingsPageEvent::Confirm(request));
    }

    fn confirm_uninstall(
        &mut self,
        id: &AgentId,
        name: &SharedString,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let registry = self.registry(cx);
        let is_custom = registry
            .read(cx)
            .agent(id)
            .is_some_and(AgentListing::is_custom);
        let (verb, again) = if is_custom {
            ("Remove", "added")
        } else {
            ("Uninstall", "installed")
        };
        let id = id.clone();
        let page = cx.weak_entity();
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest {
            icon: IconName::Trash,
            title: format!("{verb} {name}?").into(),
            message: format!("Threads that use it can't continue until it's {again} again.").into(),
            confirm_label: verb.into(),
            on_confirm: Rc::new(move |window, cx| {
                registry.update(cx, |registry, cx| {
                    if is_custom {
                        registry.remove_custom_agent(&id, cx)
                    } else {
                        registry.uninstall(&id, cx)
                    }
                });
                page.update(cx, |page, cx| {
                    if page.agent_panel().is_some_and(|panel| panel.agent_id == id) {
                        page.show_agents_page(AgentsPage::Installed, window, cx);
                    }
                })
                .log_err();
            }),
        }));
    }

    /// The Usage page's title, and the machine whose agents it shows, as on the Agents page.
    fn render_usage_header(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(px(28.))
            .gap_4()
            .justify_between()
            .child(Headline::new("Usage").size(HeadlineSize::Small))
            .when(self.machines.read(cx).has_remotes(), |header| {
                header.child(self.render_agents_machine_picker(window, cx))
            })
            .into_any_element()
    }

    /// The Usage page: each agent whose accounts' limits are read, as a table of its accounts
    /// and their windows.
    fn render_usage(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let client = self.agents_client(cx);
        let mut agents: Vec<AgentListing> = self
            .registry(cx)
            .read(cx)
            .agents()
            .iter()
            .filter(|agent| {
                counts_as_installed(&agent.install_state)
                    && agent
                        .accounts
                        .as_ref()
                        .is_some_and(|support| support.reads_usage)
            })
            .cloned()
            .collect();
        agents.sort_by_key(|agent| agent.name().to_lowercase());
        let now = SystemTime::now();
        let mut timeline_agents = Vec::new();
        let mut sections: Vec<AnyElement> = agents
            .iter()
            .filter_map(|agent| {
                let table = UsageTable::new(account_entries(&client.read(cx).accounts(agent.id())));
                if table.rows.is_empty() {
                    return None;
                }
                let section = self.render_usage_agent(agent, &table, now, cx);
                timeline_agents.push(TimelineAgent {
                    id: agent.id().clone(),
                    name: agent.name().clone(),
                    icon: agent_icon(agent.id(), cx),
                    accounts: table.rows,
                });
                Some(section)
            })
            .collect();
        if sections.is_empty() {
            return vec![
                Label::new("No agent on this machine reports its accounts' limits.")
                    .color(Color::Muted)
                    .into_any_element(),
            ];
        }
        sections.push(self.render_usage_timeline(&timeline_agents, now, cx));
        sections
    }

    /// Every account's windows side by side, after the tables (subscription timeline round).
    fn render_usage_timeline(
        &self,
        agents: &[TimelineAgent],
        now: SystemTime,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let page = cx.entity().downgrade();
        let on_zoom = Rc::new(move |zoom, _: &mut Window, cx: &mut App| {
            page.update(cx, |page, cx| {
                page.usage_timeline_zoom = zoom;
                cx.notify();
            })
            .log_err();
        });
        let page = cx.entity().downgrade();
        let on_track_width = Rc::new(move |width, _: &mut Window, cx: &mut App| {
            page.update(cx, |page, cx| {
                page.usage_timeline_width = width;
                cx.notify();
            })
            .log_err();
        });
        let page = cx.entity().downgrade();
        let machine = self.agents_machine;
        let on_open = Rc::new(
            move |agent_id: &AgentId, window: &mut Window, cx: &mut App| {
                page.update(cx, |page, cx| {
                    page.show_agent_accounts(machine, agent_id, false, window, cx)
                })
                .log_err();
            },
        );
        UsageTimeline {
            agents,
            zoom: self.usage_timeline_zoom,
            track_width: self.usage_timeline_width,
            now,
            on_zoom,
            on_open,
            on_track_width,
        }
        .render(cx)
    }

    /// An agent's card: a column per window, an "All N accounts" row with what's left across
    /// them (t3code's pooled number) when there's more than one, then a row per account.
    fn render_usage_agent(
        &self,
        agent: &AgentListing,
        table: &UsageTable,
        now: SystemTime,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let icon = match agent_icon(agent.id(), cx) {
            Some(markup) => Icon::from_svg_markup(markup),
            None => Icon::new(IconName::Sparkle),
        };
        let count = table.rows.len();
        let agent_key = agent.id().0.to_string();
        let column = |child: Option<AnyElement>| {
            div()
                .w(USAGE_COLUMN_WIDTH)
                .flex_none()
                .min_w_0()
                .children(child)
        };
        let head = h_flex()
            .px_4()
            .py_2()
            .gap(USAGE_COLUMN_GAP)
            .child(
                div().flex_1().min_w_0().child(
                    Label::new("Account")
                        .size(LabelSize::XSmall)
                        .color(Color::Muted),
                ),
            )
            .children(table.columns.iter().map(|label| {
                column(Some(
                    Label::new(label.clone())
                        .size(LabelSize::XSmall)
                        .color(Color::Muted)
                        .truncate()
                        .into_any_element(),
                ))
            }))
            .child(div().w(px(16.)).flex_none());
        let all = (count > 1).then(|| {
            let selector = format!("usage-{agent_key}-all");
            h_flex()
                .debug_selector(move || selector)
                .px_4()
                .py(px(9.))
                .gap(USAGE_COLUMN_GAP)
                .bg(colors.text.opacity(0.02))
                .child(
                    div().flex_1().min_w_0().child(
                        Label::new(format!("All {count} accounts"))
                            .size(LabelSize::Small)
                            .weight(gpui::FontWeight::MEDIUM),
                    ),
                )
                .children((0..table.columns.len()).map(|index| {
                    column(table.pooled(index, now).map(|window| {
                        render_limit_cell(
                            &format!("usage-{agent_key}-all"),
                            index,
                            &window,
                            now,
                            cx,
                        )
                    }))
                }))
                .child(div().w(px(16.)).flex_none())
        });
        let rows: Vec<AnyElement> = table
            .rows
            .iter()
            .map(|entry| self.render_usage_row(agent.id(), table, entry, now, cx))
            .collect();
        let card_selector = format!("usage-{agent_key}");
        v_flex()
            .gap_2p5()
            .child(
                h_flex()
                    .gap_2()
                    .child(icon.size(IconSize::Small).color(Color::Muted))
                    .child(Label::new(agent.name().clone()).weight(gpui::FontWeight::MEDIUM))
                    .when(count > 1, |title| {
                        title.child(
                            Label::new(format!("{count} accounts"))
                                .size(LabelSize::Small)
                                .color(Color::Placeholder),
                        )
                    }),
            )
            .child(
                v_flex()
                    .debug_selector(move || card_selector)
                    .rounded_lg()
                    .border_1()
                    .border_color(colors.border)
                    .overflow_hidden()
                    .child(head)
                    .children(
                        all.into_iter()
                            .map(IntoElement::into_any_element)
                            .chain(rows)
                            .map(|row| {
                                div()
                                    .border_t_1()
                                    .border_color(colors.border_variant)
                                    .child(row)
                            }),
                    ),
            )
            .into_any_element()
    }

    /// An account's row: its avatar, name and plan, then a cell per window. Clicking it opens
    /// the account on its agent's page.
    fn render_usage_row(
        &self,
        agent_id: &AgentId,
        table: &UsageTable,
        entry: &AccountEntry,
        now: SystemTime,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let key = format!("usage-{}-{}", agent_id.0, account_selector(entry.account));
        let detail = entry
            .plan
            .iter()
            .cloned()
            .chain(
                entry
                    .email
                    .clone()
                    .filter(|email| entry.name.as_ref() != email),
            )
            .collect::<Vec<_>>()
            .join(" · ");
        let machine = self.agents_machine;
        let agent_id = agent_id.clone();
        let selector = key.clone();
        h_flex()
            .id(SharedString::from(key.clone()))
            .debug_selector(move || selector)
            .px_4()
            .py_2()
            .gap(USAGE_COLUMN_GAP)
            .cursor_pointer()
            .hover(|row| row.bg(colors.element_hover))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .child(render_entry_avatar(entry, px(22.), cx))
                    .child(
                        v_flex()
                            .min_w_0()
                            .gap_px()
                            .child(
                                h_flex()
                                    .min_w_0()
                                    .gap_2()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .child(Label::new(entry.name.clone()).truncate()),
                                    )
                                    .when(entry.account.is_none(), |name| {
                                        name.child(account_tag("Outside", Color::Muted, cx))
                                    }),
                            )
                            .when(!detail.is_empty(), |column| {
                                column.child(
                                    Label::new(detail)
                                        .size(LabelSize::XSmall)
                                        .color(Color::Muted)
                                        .truncate(),
                                )
                            }),
                    ),
            )
            .children(table.columns.iter().enumerate().map(|(index, label)| {
                div().w(USAGE_COLUMN_WIDTH).flex_none().min_w_0().children(
                    entry
                        .windows
                        .iter()
                        .find(|window| window.label == *label)
                        .map(|window| render_limit_cell(&key, index, window, now, cx)),
                )
            }))
            .child(
                div().w(px(16.)).flex_none().child(
                    Icon::new(IconName::ChevronRight)
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                ),
            )
            .on_click(cx.listener(move |this, _, window, cx| {
                this.show_agent_accounts(machine, &agent_id, false, window, cx)
            }))
            .into_any_element()
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
        self.skill_error = None;
        self.mcp_server_error = None;
        self.storage_page.error = None;
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
            None => crate::machines::LOCAL_MACHINE_NAME.into(),
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

    /// The project's name, and the machine picker while it has more than one copy (the
    /// Agents and Usage pages' dropdown, with New Thread's labels).
    fn render_project_header(
        &self,
        key: ProjectKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let copies = self.group_members(key, cx);
        let name = self.project_name(key, cx);
        let picker = (copies.len() > 1).then(|| {
            let machines = self.machines.read(cx);
            let rows: Vec<(ProjectKey, IconName, SharedString)> = copies
                .iter()
                .map(|(copy, _)| {
                    (
                        *copy,
                        machines.machine_icon(copy.machine, cx),
                        copy_label(*copy, &copies, cx),
                    )
                })
                .collect();
            let icon = machines.machine_icon(key.machine, cx);
            let page = cx.weak_entity();
            // Custom entries, since a toggleable entry's icon takes the check's place.
            let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
                for (copy, icon, label) in rows {
                    menu = menu.custom_entry(
                        move |_, _| {
                            h_flex()
                                .w_full()
                                .gap_1p5()
                                .debug_selector(move || {
                                    format!(
                                        "project-copy-option-{}-{}",
                                        copy.machine.slug(),
                                        copy.project.0
                                    )
                                })
                                .child(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
                                .child(Label::new(label.clone()))
                                .child(div().flex_1().min_w(px(16.)))
                                .when(copy == key, |row| {
                                    row.child(
                                        Icon::new(IconName::Check)
                                            .size(IconSize::Small)
                                            .color(Color::Accent),
                                    )
                                })
                                .into_any_element()
                        },
                        on_page(&page, move |page, _, cx| page.show_copy(copy, cx)),
                    );
                }
                menu
            });
            let trigger = h_flex()
                .gap_1p5()
                .child(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
                .child(Label::new(copy_label(key, &copies, cx)))
                .into_any_element();
            div().debug_selector(|| "project-copy-picker".into()).child(
                DropdownMenu::new_with_element("project-copy", trigger, menu),
            )
        });
        h_flex()
            .h(px(28.))
            .gap_4()
            .justify_between()
            .child(Headline::new(name).size(HeadlineSize::Small))
            .children(picker)
            .into_any_element()
    }

    /// The project's own sections, from the copy that stands for it, then the chosen copy's:
    /// its folder and grouping under its machine's name, its checkouts, and removing it.
    fn render_project(
        &self,
        key: ProjectKey,
        project: Project,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let colors = cx.theme().colors().clone();
        let copies = self.group_members(key, cx);
        let (shared_key, shared) = copies
            .first()
            .cloned()
            .unwrap_or_else(|| (key, project.clone()));
        // t3code's descriptions.
        let icon_description: SharedString = match &shared.icon {
            None => "Automatic: the project's favicon, or a monogram.".into(),
            Some(ProjectIcon::Icon { name, color }) => format!("{name} · {color}").into(),
            Some(ProjectIcon::Emoji { emoji }) => emoji.clone().into(),
            Some(ProjectIcon::Monogram { text, color }) => format!("{text} · {color}").into(),
            Some(ProjectIcon::Image { path }) => path.display().to_string().into(),
        };
        let has_icon = copies.iter().any(|(_, copy)| copy.icon.is_some());
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
        let icon_controls = h_flex()
            .gap_2()
            .child(
                div()
                    .debug_selector(|| "project-icon".into())
                    .child(render_project_icon(
                        shared_key.machine,
                        &shared,
                        px(24.),
                        cx,
                    )),
            )
            .child(
                div().debug_selector(|| "choose-icon".into()).child(
                    Button::new("choose-icon", "Choose icon")
                        .style(ButtonStyle::Outlined)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_icon_picker(shared_key, window, cx)
                        })),
                ),
            )
            .child(
                div().debug_selector(|| "choose-icon-file".into()).child(
                    Button::new("choose-icon-file", "Choose file")
                        .style(ButtonStyle::Outlined)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.open_image_picker(shared_key, window, cx)
                        })),
                ),
            )
            .when(has_icon, |this| {
                this.child(
                    div().debug_selector(|| "reset-icon".into()).child(
                        Button::new("reset-icon", "Reset")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.set_group_icon(shared_key, None, cx)
                            })),
                    ),
                )
            });
        let machine_label = self.machines.read(cx).label(key.machine, cx);
        let mut copy_rows = vec![render_row(
            "Folder",
            project.path.display().to_string(),
            div().into_any_element(),
            cx,
        )];
        copy_rows.extend(self.render_grouping(key, &project, window, cx));
        let is_combined = copies.len() > 1;
        let name = self.project_name(key, cx);
        let remove_button = div()
            .debug_selector(|| "remove-project".into())
            .child(
                Button::new(
                    "remove-project",
                    if is_combined {
                        "Remove…"
                    } else {
                        "Remove Project"
                    },
                )
                .style(ButtonStyle::Outlined)
                .color(Color::Error)
                .start_icon(
                    Icon::new(IconName::Trash)
                        .size(IconSize::Small)
                        .color(Color::Error),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.confirm_remove_project(key, name.clone(), window, cx)
                })),
            )
            .into_any_element();
        let remove_row = if is_combined {
            render_row(
                format!("Remove from {machine_label}"),
                format!(
                    "Removes this copy and its threads from agentZ. {} Files on disk are not \
                     touched.",
                    kept_copies(key, &copies, cx)
                ),
                remove_button,
                cx,
            )
        } else {
            render_row(
                "Remove project",
                "Removes the project and its threads from agentZ. Files on disk are not touched.",
                remove_button,
                cx,
            )
        };
        vec![render_section(
            "Project",
            vec![
                render_row(
                    "Name",
                    "Shown in the sidebar and thread lists. Leave it empty for the folder name.",
                    input_box(self.name_input.clone(), px(256.)).into_any_element(),
                    cx,
                ),
                render_row(
                    "Project icon",
                    icon_description,
                    icon_controls.into_any_element(),
                    cx,
                ),
            ],
            cx,
        )]
        .into_iter()
        .chain(render_repository(&project, cx))
        .chain([
            div()
                .debug_selector(move || {
                    format!("project-copy-{}-{}", key.machine.slug(), key.project.0)
                })
                .child(render_section(machine_label, copy_rows, cx))
                .into_any_element(),
            self.render_checkouts(key.machine, &project, cx),
            render_section("Danger", vec![remove_row], cx),
        ])
        .collect()
    }

    /// Whether the copy combines with the repository's other checkouts, which is saved for
    /// this copy only.
    fn render_grouping(
        &self,
        key: ProjectKey,
        project: &Project,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        project.repository.as_ref()?;
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
        Some(render_row(
            "Grouping",
            mode.description(),
            DropdownMenu::new("project-grouping-override", dropdown_label, menu).into_any_element(),
            cx,
        ))
    }
}

/// The repository the project's checkouts belong to.
fn render_repository(project: &Project, cx: &App) -> Option<AnyElement> {
    let repository = project.repository.as_ref()?;
    let name = repository
        .display_name
        .clone()
        .unwrap_or_else(|| repository.canonical_key.clone());
    Some(render_section(
        "Repository",
        vec![render_row(
            "Repository",
            format!(
                "{name} · {} {}",
                repository.remote_name, repository.remote_url
            ),
            div().into_any_element(),
            cx,
        )],
        cx,
    ))
}

/// The copy's machine in the project's machine picker, and its folder too while the project
/// has another copy there, as New Thread's machine menu tells them apart.
fn copy_label(copy: ProjectKey, copies: &[(ProjectKey, Project)], cx: &App) -> SharedString {
    let label = Machines::global(cx).read(cx).label(copy.machine, cx);
    let shares_machine = copies
        .iter()
        .any(|(other, _)| *other != copy && other.machine == copy.machine);
    let path = copies
        .iter()
        .find(|(other, _)| *other == copy)
        .map(|(_, project)| &project.path);
    match path {
        Some(path) if shares_machine => {
            let folder = match copy.machine {
                MachineId::Local => compact_path(path),
                MachineId::Remote(_) => path.display().to_string(),
            };
            format!("{label} · {folder}").into()
        }
        _ => label,
    }
}

/// Who keeps the project once the copy is removed, for Remove's description.
fn kept_copies(copy: ProjectKey, copies: &[(ProjectKey, Project)], cx: &App) -> String {
    let others: Vec<MachineId> = copies
        .iter()
        .map(|(other, _)| *other)
        .filter(|other| *other != copy)
        .map(|other| other.machine)
        .collect();
    if others == [copy.machine] {
        return "The other copy stays.".to_string();
    }
    if others.contains(&copy.machine) {
        return "The other copies stay.".to_string();
    }
    match others.as_slice() {
        [machine] => format!(
            "{} keeps its copy.",
            Machines::global(cx).read(cx).label(*machine, cx)
        ),
        [machine, rest @ ..] if rest.iter().all(|other| other == machine) => format!(
            "{} keeps its copies.",
            Machines::global(cx).read(cx).label(*machine, cx)
        ),
        _ => "Other machines keep theirs.".to_string(),
    }
}

/// The copy to show once `removed` is gone: the next of the project's copies still there, else
/// the one before it.
fn next_copy(
    copies: &[ProjectKey],
    removed: ProjectKey,
    exists: impl Fn(ProjectKey) -> bool,
) -> Option<ProjectKey> {
    let index = copies.iter().position(|copy| *copy == removed)?;
    copies[index + 1..]
        .iter()
        .chain(copies[..index].iter().rev())
        .copied()
        .find(|copy| exists(*copy))
}

pub(crate) async fn remove_workspace(
    store: &Entity<ProjectStore>,
    path: PathBuf,
    force: bool,
    cx: &mut gpui::AsyncWindowContext,
) -> anyhow::Result<WorkspaceRemoval> {
    store
        .read_with(cx, |store, cx| store.remove_workspace(path, force, cx))
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

struct AgentPanel {
    agent_id: AgentId,
    tab: AgentTab,
    /// The External account's: the agent in its own home. The other tabs use it.
    external: AccountSession,
    /// agentZ's accounts', opened as they're listed.
    accounts: BTreeMap<AccountId, AccountSession>,
    /// The account whose name is being edited on its card.
    renaming: Option<AccountRename>,
    /// Add Account's request, while it's on its way.
    adding_account: Option<Task<()>>,
    /// Why the last change to the accounts failed.
    account_error: Option<SharedString>,
    /// The account picked over the Defaults, Environment and Threads tabs.
    picked_account: AccountChoice,
    /// Whose variables `env_rows` are.
    env_account: Option<AccountId>,
    env_rows: Vec<EnvRow>,
    /// Whose sessions `sessions` are.
    sessions_account: Option<AccountId>,
    /// Listed when the Threads tab first opens; `None` before.
    sessions: Option<SessionList>,
    /// The project whose sessions the Threads tab shows, once the user picks one.
    sessions_project: Option<ProjectId>,
    sessions_shown: usize,
    /// Sessions whose import hasn't been answered yet.
    importing: HashSet<String>,
    import_error: Option<SharedString>,
    /// The tab each card shows over its limits, for an account with pools: its own first, as
    /// Droid's `/limits` opens on Standard.
    limit_tabs: HashMap<Option<AccountId>, usize>,
    /// Accounts whose Switch to Droid Core is on its way.
    switching_to_core: HashSet<Option<AccountId>>,
    /// Accounts whose Use Reset is on its way.
    using_limit_reset: HashSet<Option<AccountId>>,
    /// The account dialog over the page, while it's open.
    dialog: Option<AccountDialogState>,
    _subscriptions: Vec<Subscription>,
}

struct AccountDialogState {
    view: WeakEntity<AccountDialog>,
    content: DialogContent,
    _released: Subscription,
}

/// What an agent's account dialog shows.
enum DialogContent {
    /// An account's whole card, `None` being the External account.
    Account(Option<AccountId>),
    Adding(AddingAccount),
}

/// Add Account's dialog, from the request for the account to its result.
struct AddingAccount {
    /// The account the server made for the login, once it answers.
    account: Option<AccountId>,
    /// The account's login in the dialog, once its session opens.
    login: Option<Entity<AgentLogin>>,
}

/// Where an account's card is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CardPlace {
    Page,
    Dialog,
}

impl AgentPanel {
    fn client(&self, cx: &App) -> Entity<ServerClient> {
        self.external.connection.read(cx).client().clone()
    }

    /// The account the Defaults, Environment and Threads tabs show: the one picked while it's
    /// listed, else the account for new threads.
    fn settings_account(&self, accounts: &AgentAccounts) -> Option<AccountId> {
        let listed = accounts.listed();
        match self.picked_account {
            AccountChoice::External if listed.contains(&None) => None,
            AccountChoice::Account(id) if listed.contains(&Some(id)) => Some(id),
            _ => accounts.new_thread_account(),
        }
    }

    /// Add Account's dialog, while it's open.
    fn adding(&self) -> Option<&AddingAccount> {
        match &self.dialog.as_ref()?.content {
            DialogContent::Adding(adding) => Some(adding),
            DialogContent::Account(_) => None,
        }
    }

    fn adding_mut(&mut self) -> Option<&mut AddingAccount> {
        match &mut self.dialog.as_mut()?.content {
            DialogContent::Adding(adding) => Some(adding),
            DialogContent::Account(_) => None,
        }
    }

    /// The account's login session, `None` being the External account.
    fn session(&self, account: Option<AccountId>) -> Option<&AccountSession> {
        match account {
            None => Some(&self.external),
            Some(id) => self.accounts.get(&id),
        }
    }

    fn session_mut(&mut self, account: Option<AccountId>) -> Option<&mut AccountSession> {
        match account {
            None => Some(&mut self.external),
            Some(id) => self.accounts.get_mut(&id),
        }
    }

    /// For the badge beside the agent's name: logged in while any account it lists is, else
    /// the furthest along of them. Without agentZ accounts, the External account's.
    fn agent_state(&self, cx: &App) -> AccountState {
        let external = AccountState::of(self.external.connection.read(cx));
        if self.accounts.is_empty() {
            return external;
        }
        let accounts = self
            .external
            .connection
            .read(cx)
            .client()
            .read(cx)
            .accounts(&self.agent_id);
        let states: Vec<AccountState> = accounts
            .listed()
            .into_iter()
            .filter_map(|account| self.session(account))
            .map(|session| AccountState::of(session.connection.read(cx)))
            .collect();
        [
            AccountState::LoggedIn,
            AccountState::LoggingIn,
            AccountState::Connecting,
            AccountState::Failed,
        ]
        .into_iter()
        .find(|state| states.contains(state))
        .unwrap_or(AccountState::LoggedOut)
    }
}

/// An account's connection to the agent, without a session, in the account's home: whether
/// it's logged in, and its login rows. Alive only while the agent's page is open.
struct AccountSession {
    connection: Entity<AgentThread>,
    login: Entity<AgentLogin>,
    elicitation_cards: Vec<Entity<ElicitationCard>>,
    /// The user asked to log in to another account while logged in (Change Account, for an
    /// agent that can't have more accounts).
    changing_account: bool,
    /// Whether the connection was logging in when last seen, to notice when it's done.
    was_authenticating: bool,
    _subscription: Subscription,
}

/// Rename, edited in place on the account's card.
struct AccountRename {
    account: Option<AccountId>,
    input: Entity<TextInput>,
    /// Clicking elsewhere keeps the name, once the field has focus.
    _blur: Option<Subscription>,
}

/// What an account's ⋯ menu offers.
struct AccountMenu {
    account: Option<AccountId>,
    /// For the confirms.
    name: SharedString,
    can_make_default: bool,
    color: Option<String>,
    can_refresh: bool,
    read_at: Option<SystemTime>,
    usage_page: Option<String>,
    /// Show in Finder's (or the file manager's), on this machine.
    folder: Option<PathBuf>,
    can_log_out: bool,
}

struct EnvRow {
    key: Entity<TextInput>,
    value: Entity<TextInput>,
    _subscriptions: Vec<Subscription>,
}

struct CustomAgentForm {
    /// The agent being changed; `None` adds one.
    agent_id: Option<AgentId>,
    name: Entity<TextInput>,
    command: Entity<TextInput>,
    /// Split on spaces, as Zed's form does.
    args: Entity<TextInput>,
    env: Vec<EnvRow>,
    error: Option<SharedString>,
    /// The server is starting the agent to check it.
    saving: Option<Task<()>>,
}

fn new_text_input(placeholder: &str, text: &str, cx: &mut App) -> Entity<TextInput> {
    cx.new(|cx| {
        let mut input = TextInput::new(placeholder.to_string(), cx);
        input.set_text(text.to_string(), cx);
        input
    })
}

fn new_variable_row(key: &str, value: &str, cx: &mut App) -> EnvRow {
    EnvRow {
        key: new_text_input("NAME", key, cx),
        value: new_text_input("value", value, cx),
        _subscriptions: Vec::new(),
    }
}

/// What a login method logs in with, from its name: "Log in with Google" gives "Google", "Use an
/// API key" "an API key", "API Key" stays, and a bare "Log In" gives nothing.
fn login_method_subject(method: &str) -> Option<String> {
    let trimmed = method.trim();
    let lower = trimmed.to_lowercase();
    for prefix in [
        "log in with ",
        "login with ",
        "sign in with ",
        "signin with ",
        "use ",
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
pub(crate) fn logged_in_title(
    status: Option<&AuthStatus>,
    login_method: Option<&str>,
) -> SharedString {
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

/// Under the account card: that agentZ logged the agent in, with which method, or that it was
/// logged in outside agentZ (as by its own CLI), which a callout explains since logging out here
/// logs that login out too. Either way, every thread with the agent on the machine shares it.
fn render_login_source(
    agent_name: &SharedString,
    machine: &SharedString,
    login_method: Option<&str>,
    title_shows_method: bool,
    can_log_out: bool,
    cx: &App,
) -> AnyElement {
    let Some(method) = login_method else {
        let colors = cx.theme().colors();
        let heading = "Logged in outside agentZ.";
        let text = format!(
            "{heading} {agent_name} found the login it already had on {machine} (from its CLI or \
             its own settings). Every thread with it uses this login{}",
            if can_log_out {
                ", and logging out here logs out the CLI too."
            } else {
                "."
            }
        );
        let heading_style = gpui::HighlightStyle {
            color: Some(colors.text),
            font_weight: Some(gpui::FontWeight::MEDIUM),
            ..Default::default()
        };
        return h_flex()
            .debug_selector(|| "login-source-outside".into())
            .items_start()
            .gap_2p5()
            .px_3()
            .py_2p5()
            .rounded_lg()
            .border_1()
            .border_color(colors.border_variant)
            .child(
                div().pt(px(1.)).child(
                    Icon::new(IconName::Info)
                        .size(IconSize::Small)
                        .color(Color::Accent),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_ui_sm(cx)
                    .text_color(colors.text_muted)
                    .child(
                        gpui::StyledText::new(text)
                            .with_highlights([(0..heading.len(), heading_style)]),
                    ),
            )
            .into_any_element();
    };
    let subject = login_method_subject(method).filter(|_| !title_shows_method);
    let source = match subject {
        Some(subject) => format!("Logged in from agentZ with {subject}."),
        None => "Logged in from agentZ.".to_string(),
    };
    Label::new(format!(
        "{source} Every thread with {agent_name} on {machine} uses this login."
    ))
    .size(LabelSize::Small)
    .color(Color::Muted)
    .into_any_element()
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

/// An account card's line under its name: the email when Rename gave it another name, and the
/// plan, from the last read or else from what the agent reported.
fn account_card_details(
    choices: &AccountChoices,
    email: Option<&str>,
    status: Option<&AccountStatus>,
    auth_status: Option<&AuthStatus>,
) -> Vec<String> {
    let mut details = Vec::new();
    if choices.label.is_some() {
        details.extend(email.map(str::to_string));
    }
    match status {
        Some(status) => details.extend(status.plan.clone()),
        None => details.extend(auth_status.map(account_details).unwrap_or_default()),
    }
    details.retain(|detail| !detail.trim().is_empty());
    details
}

/// An account as the account menus show it.
#[derive(Clone)]
pub(crate) struct AccountEntry {
    pub(crate) account: Option<AccountId>,
    pub(crate) name: SharedString,
    /// Whether `name` is the account's own (an email, or Rename's), for its avatar's initial.
    pub(crate) is_named: bool,
    pub(crate) color: Option<String>,
    /// From its last read.
    pub(crate) plan: Option<String>,
    pub(crate) email: Option<String>,
    pub(crate) windows: Vec<LimitWindow>,
    pub(crate) limit_resets: Option<LimitResets>,
    /// Found logged out when last checked.
    pub(crate) is_logged_out: bool,
}

/// The listed accounts, for the menus that pick one.
pub(crate) fn account_entries(accounts: &AgentAccounts) -> Vec<AccountEntry> {
    accounts
        .listed()
        .into_iter()
        .map(|account| account_entry(accounts, account))
        .collect()
}

/// Any of the agent's accounts, listed or not, `None` being the External one.
pub(crate) fn account_entry(accounts: &AgentAccounts, account: Option<AccountId>) -> AccountEntry {
    let name = accounts.name(account);
    let fallback = match account {
        None => "Outside agentZ".to_string(),
        // An API key's account has no email, and goes by its login method.
        Some(id) => accounts
            .account(id)
            .and_then(|account| account.settings.login_method.as_deref())
            .and_then(login_method_subject)
            .unwrap_or_else(|| "New account".to_string()),
    };
    let status = accounts.status(account).map(|read| &read.status);
    let logged_in = match account {
        None => accounts.external_logged_in,
        Some(id) => accounts.account(id).and_then(|account| account.logged_in),
    };
    AccountEntry {
        account,
        is_named: name.is_some(),
        name: name.unwrap_or(fallback).into(),
        color: accounts
            .choices(account)
            .and_then(|choices| choices.color.clone()),
        plan: status.and_then(|status| status.plan.clone()),
        email: status.and_then(|status| status.email.clone()),
        windows: status
            .map(|status| status.windows.clone())
            .unwrap_or_default(),
        limit_resets: status.and_then(|status| status.limit_resets),
        is_logged_out: logged_in == Some(false),
    }
}

/// An agent's accounts on the Usage page: a column per window, in the order they first
/// appear, and a row per account with a read, the External one first. Accounts found logged out
/// are left out, their last reads being out of date.
struct UsageTable {
    columns: Vec<String>,
    rows: Vec<AccountEntry>,
}

impl UsageTable {
    fn new(entries: Vec<AccountEntry>) -> Self {
        let rows: Vec<AccountEntry> = entries
            .into_iter()
            .filter(|entry| !entry.is_logged_out && !entry.windows.is_empty())
            .collect();
        let mut columns: Vec<String> = Vec::new();
        for window in rows.iter().flat_map(|entry| &entry.windows) {
            if !columns.contains(&window.label) {
                columns.push(window.label.clone());
            }
        }
        Self { columns, rows }
    }

    /// What's left of the column's window across the accounts that have it, each counting the
    /// same whatever its plan, as in t3code. It has no reset or pace of its own. A window past
    /// its reset is left out until it's read again, its percentage being out of date.
    fn pooled(&self, column: usize, now: SystemTime) -> Option<LimitWindow> {
        let label = self.columns.get(column)?;
        let used: Vec<f64> = self
            .rows
            .iter()
            .filter_map(|entry| entry.windows.iter().find(|window| window.label == *label))
            .filter(|window| !is_resetting(window, now))
            .map(|window| window.used_percent.clamp(0., 100.))
            .collect();
        (!used.is_empty()).then(|| LimitWindow {
            label: label.clone(),
            used_percent: used.iter().sum::<f64>() / used.len() as f64,
            resets_at: None,
            length: None,
        })
    }
}

/// How an account's elements are named in tests: by its id, or "external".
pub(crate) fn account_selector(account: Option<AccountId>) -> String {
    account.map_or_else(|| "external".to_string(), |id| id.to_string())
}

pub(crate) fn render_entry_avatar(entry: &AccountEntry, size: Pixels, cx: &App) -> AnyElement {
    if entry.is_named {
        let color = entry
            .color
            .as_deref()
            .and_then(|hex| account_color(hex, cx));
        avatar(&entry.name, color, size, cx)
    } else {
        account_badge(size, cx)
    }
}

/// An account in a menu: its avatar and name, and a check when it's the one chosen.
pub(crate) fn render_account_entry(entry: &AccountEntry, is_current: bool, cx: &App) -> AnyElement {
    h_flex()
        .w_full()
        .gap_1p5()
        .child(render_entry_avatar(entry, MENU_AVATAR_SIZE, cx))
        .child(Label::new(entry.name.clone()))
        .child(div().flex_1().min_w(px(16.)))
        .when(is_current, |row| {
            row.child(
                Icon::new(IconName::Check)
                    .size(IconSize::Small)
                    .color(Color::Accent),
            )
        })
        .into_any_element()
}

/// A small tag beside an account's name: "Outside agentZ", "Default".
fn account_tag(label: &'static str, color: Color, cx: &App) -> gpui::Div {
    div()
        .flex_none()
        .px_1p5()
        .rounded(px(4.))
        .bg(cx.theme().colors().element_hover)
        .child(Label::new(label).size(LabelSize::XSmall).color(color))
}

/// A menu item's handler that acts on the settings page.
fn on_page(
    page: &WeakEntity<SettingsPage>,
    action: impl Fn(&mut SettingsPage, &mut Window, &mut Context<SettingsPage>) + 'static,
) -> impl Fn(&mut Window, &mut App) + 'static {
    let page = page.clone();
    move |window, cx| {
        page.update(cx, |page, cx| action(page, window, cx))
            .log_err();
    }
}

/// An account's ⋯ menu, with the sidebar's muted icons.
fn render_account_menu(
    selector: &str,
    menu: AccountMenu,
    cx: &mut Context<SettingsPage>,
) -> AnyElement {
    let page = cx.weak_entity();
    let menu = Rc::new(menu);
    let debug_selector = format!("account-menu-{selector}");
    div()
        .debug_selector(move || debug_selector)
        .child(
            PopoverMenu::new(SharedString::from(format!("account-menu-{selector}")))
                .menu(move |window, cx| {
                    let page = page.clone();
                    let menu = menu.clone();
                    Some(ContextMenu::build(
                        window,
                        cx,
                        move |context_menu, _, cx| {
                            build_account_menu(context_menu, &menu, &page, cx)
                        },
                    ))
                })
                .trigger_with_tooltip(
                    IconButton::new(
                        SharedString::from(format!("account-menu-trigger-{selector}")),
                        IconName::Ellipsis,
                    )
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted),
                    Tooltip::text("More"),
                )
                .anchor(gpui::Anchor::TopRight)
                .offset(gpui::point(px(0.), px(4.))),
        )
        .into_any_element()
}

fn build_account_menu(
    mut context_menu: ContextMenu,
    menu: &AccountMenu,
    page: &WeakEntity<SettingsPage>,
    cx: &App,
) -> ContextMenu {
    let account = menu.account;
    context_menu = context_menu.item(
        ContextMenuEntry::new("Rename…")
            .icon(IconName::Pencil)
            .icon_color(Color::Muted)
            .handler(on_page(page, move |page, window, cx| {
                page.start_account_rename(account, window, cx)
            })),
    );
    if menu.can_make_default {
        context_menu = context_menu.item(
            ContextMenuEntry::new("Use for New Threads")
                .icon(IconName::Star)
                .icon_color(Color::Muted)
                .handler(on_page(page, move |page, _, cx| {
                    page.update_account(account, AccountChange::MakeDefault, cx)
                })),
        );
    }
    let swatch = menu
        .color
        .as_deref()
        .and_then(|hex| account_color(hex, cx))
        .map_or(Color::Muted, Color::Custom);
    let color = menu.color.clone();
    let color_page = page.clone();
    context_menu = context_menu.submenu_with_colored_icon(
        "Color",
        IconName::Circle,
        swatch,
        move |submenu, _, _| {
            build_account_color_menu(submenu, account, color.as_deref(), &color_page)
        },
    );
    if menu.can_refresh {
        let read = menu
            .read_at
            .map(|read_at| read_ago(read_at, SystemTime::now()));
        context_menu =
            context_menu.custom_entry(
                move |_, _| {
                    h_flex()
                        .debug_selector(|| "account-menu-refresh".into())
                        .w_full()
                        .gap_1p5()
                        .child(
                            Icon::new(IconName::RotateCw)
                                .size(IconSize::Small)
                                .color(Color::Muted),
                        )
                        .child(Label::new("Refresh Usage"))
                        .child(div().flex_1().min_w(px(16.)))
                        .children(read.clone().map(|read| {
                            Label::new(read).size(LabelSize::Small).color(Color::Muted)
                        }))
                        .into_any_element()
                },
                on_page(page, move |page, _, cx| page.refresh_usage(account, cx)),
            );
    }
    if let Some(url) = menu.usage_page.clone() {
        context_menu = context_menu.item(
            ContextMenuEntry::new("Open Usage Page")
                .icon(IconName::ArrowUpRight)
                .icon_color(Color::Muted)
                .handler(move |_, cx| cx.open_url(&url)),
        );
    }
    if let Some(folder) = menu.folder.clone() {
        context_menu = context_menu.item(
            ContextMenuEntry::new(if cfg!(target_os = "macos") {
                "Show in Finder"
            } else {
                "Show in File Manager"
            })
            .icon(IconName::Folder)
            .icon_color(Color::Muted)
            .handler(move |_, cx| cx.reveal_path(&folder)),
        );
    }
    if menu.can_log_out || account.is_some() {
        context_menu = context_menu.separator();
    }
    if menu.can_log_out {
        let name = menu.name.clone();
        context_menu = context_menu.item(
            ContextMenuEntry::new("Log Out")
                .icon(IconName::Exit)
                .icon_color(Color::Muted)
                .handler(on_page(page, move |page, _, cx| {
                    page.confirm_logout(account, Some(name.clone()), cx)
                })),
        );
    }
    // The External account is the agent's own login, which agentZ never removes.
    if let Some(id) = account {
        let name = menu.name.clone();
        context_menu = context_menu.item(
            ContextMenuEntry::new("Remove Account…")
                .icon(IconName::Trash)
                .icon_color(Color::Muted)
                .handler(on_page(page, move |page, _, cx| {
                    page.confirm_remove_account(id, name.clone(), cx)
                })),
        );
    }
    context_menu
}

/// The account's color (§13), as t3code's accent colors: a swatch each, then No Color.
fn build_account_color_menu(
    mut submenu: ContextMenu,
    account: Option<AccountId>,
    current: Option<&str>,
    page: &WeakEntity<SettingsPage>,
) -> ContextMenu {
    for (name, shade, _) in ACCOUNT_COLORS {
        let hex = color_hex(shade);
        let is_current = current.is_some_and(|current| current.eq_ignore_ascii_case(&hex));
        let swatch = hex.clone();
        submenu = submenu.custom_entry(
            move |_, cx| render_color_entry(name, account_color(&swatch, cx), is_current, cx),
            on_page(page, move |page, _, cx| {
                page.update_account(account, AccountChange::SetColor(Some(hex.clone())), cx)
            }),
        );
    }
    let has_none = current.is_none();
    submenu.separator().custom_entry(
        move |_, cx| render_color_entry("No Color", None, has_none, cx),
        on_page(page, move |page, _, cx| {
            page.update_account(account, AccountChange::SetColor(None), cx)
        }),
    )
}

fn render_color_entry(
    name: &'static str,
    swatch: Option<gpui::Hsla>,
    is_current: bool,
    cx: &App,
) -> AnyElement {
    let dot = div().size(px(10.)).m(px(2.)).flex_none().rounded_full();
    let dot = match swatch {
        Some(color) => dot.bg(color),
        None => dot.border_1().border_color(cx.theme().colors().border),
    };
    h_flex()
        .w_full()
        .gap_1p5()
        .child(dot)
        .child(Label::new(name))
        .child(div().flex_1().min_w(px(16.)))
        .when(is_current, |row| {
            row.child(
                Icon::new(IconName::Check)
                    .size(IconSize::Small)
                    .color(Color::Accent),
            )
        })
        .into_any_element()
}

/// A card's tabs over its limits: two or three, as `ToggleButtonGroup` takes a fixed number.
fn render_limit_tabs(
    id: String,
    buttons: Vec<ToggleButtonSimple>,
    selected: usize,
) -> Option<AnyElement> {
    fn group<const COUNT: usize>(
        id: String,
        buttons: [ToggleButtonSimple; COUNT],
        selected: usize,
    ) -> AnyElement {
        let selector = id.clone();
        // As wide as its tabs, rather than the card.
        h_flex()
            .child(
                div().debug_selector(move || selector).child(
                    ToggleButtonGroup::single_row(id, buttons)
                        .style(ToggleButtonGroupStyle::Outlined)
                        .label_size(LabelSize::Small)
                        .auto_width()
                        .selected_index(selected),
                ),
            )
            .into_any_element()
    }
    match buttons.len() {
        2 => Some(group::<2>(id, buttons.try_into().ok()?, selected)),
        3 => Some(group::<3>(id, buttons.try_into().ok()?, selected)),
        4 => Some(group::<4>(id, buttons.try_into().ok()?, selected)),
        _ => None,
    }
}

fn render_check_entry(name: &'static str, is_current: bool) -> AnyElement {
    h_flex()
        .w_full()
        .gap_1p5()
        .child(Label::new(name))
        .child(div().flex_1().min_w(px(16.)))
        .when(is_current, |row| {
            row.child(
                Icon::new(IconName::Check)
                    .size(IconSize::Small)
                    .color(Color::Accent),
            )
        })
        .into_any_element()
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
fn render_section(title: impl Into<SharedString>, rows: Vec<AnyElement>, cx: &App) -> AnyElement {
    render_section_with_actions(title, rows, gpui::Empty.into_any_element(), cx)
}

/// A section with buttons beside its title, as t3code's `headerAction`.
fn render_section_with_actions(
    title: impl Into<SharedString>,
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
            if let Some(panel) = this.agent_panel_mut() {
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

/// A setting under an account card's hairline, in the body's size so it doesn't read as a
/// heading: its name, and its menu at the right. It lines up with the limits, past the avatar.
fn render_card_setting(title: impl Into<SharedString>, control: AnyElement) -> AnyElement {
    h_flex()
        .pl(px(16.) + AVATAR_SIZE + px(12.))
        .pr_4()
        .py(px(4.))
        .gap_6()
        .justify_between()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(Label::new(title.into()).size(LabelSize::Small)),
        )
        .child(div().flex_none().child(control))
        .into_any_element()
}

/// When an account's limits were read: "read just now", "read 5m ago".
fn read_ago(read_at: SystemTime, now: SystemTime) -> String {
    match format_relative_time(read_at, now).as_str() {
        "now" => "read just now".to_string(),
        ago => format!("read {ago} ago"),
    }
}

/// A row of a page's list, which is laid out on its own.
struct ContentRow {
    element: AnyElement,
    /// The space between it and the row above it.
    space_above: Rems,
}

impl ContentRow {
    fn new(element: AnyElement, space_above: Rems) -> Self {
        Self {
            element,
            space_above,
        }
    }

    /// One of a page's sections, spaced from the one above as sections are.
    fn section(element: AnyElement) -> Self {
        Self::new(element, SECTION_SPACING)
    }
}

/// The page's header and rows as its list draws them: each in the page's centered column, with
/// the page's margin under the last one.
fn content_list_items(header: AnyElement, rows: Vec<ContentRow>) -> Vec<Option<AnyElement>> {
    let count = rows.len() + 1;
    std::iter::once(ContentRow::section(header))
        .chain(rows)
        .enumerate()
        .map(|(index, row)| {
            // Blocks rather than flex columns: taffy measures a flex column's children twice,
            // and once more for each column around it.
            div()
                .w_full()
                .child(
                    div()
                        .max_w(CONTENT_WIDTH)
                        .mx_auto()
                        .px_8()
                        .pt(row.space_above)
                        .when(index + 1 == count, |column| column.pb_6())
                        .child(row.element),
                )
                .into_any_element()
        })
        .map(Some)
        .collect()
}

impl SettingsPage {
    /// The open page's header, and its rows under it.
    fn render_content(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (AnyElement, Vec<ContentRow>) {
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
            Section::Notifications => (
                headline("Notifications".into()),
                self.render_notifications(window, cx),
            ),
            Section::Agents => {
                return (
                    self.render_agents_header(window, cx),
                    self.render_agents(window, cx),
                );
            }
            Section::Usage => (self.render_usage_header(window, cx), self.render_usage(cx)),
            Section::Skills => (
                self.render_skills_header(window, cx),
                self.render_skills(window, cx),
            ),
            Section::McpServers => (
                self.render_mcp_servers_header(window, cx),
                self.render_mcp_servers(window, cx),
            ),
            Section::Machines => (headline("Machines".into()), self.render_machines(cx)),
            Section::Storage => (
                self.render_storage_header(window, cx),
                self.render_storage(cx),
            ),
            Section::Project(key) => match self.project(key, cx) {
                Some(project) => (
                    self.render_project_header(key, window, cx),
                    self.render_project(key, project, window, cx),
                ),
                None => (headline("General".into()), self.render_general(window, cx)),
            },
        };
        (
            header,
            sections.into_iter().map(ContentRow::section).collect(),
        )
    }

    /// The open page as a list, as Zed's settings pages are, so a frame lays out only the rows
    /// in view, each on its own.
    fn render_content_list(
        &mut self,
        header: AnyElement,
        rows: Vec<ContentRow>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let items = content_list_items(header, rows);
        let count = items.len();
        let listed = self.content_list.item_count();
        if count != listed {
            let kept = count.min(listed);
            self.content_list.splice(kept..listed, count - kept);
        }
        let scroll_top = self.content_list.logical_scroll_top();
        let scrolled = scroll_top.item_ix != self.content_scroll_top.item_ix
            || scroll_top.offset_in_item != self.content_scroll_top.offset_in_item;
        self.content_scroll_top = scroll_top;
        if !scrolled {
            // The list lays out the rows in view each frame, but keeps the height a row out of
            // view had when it was last laid out, and anything but a scroll may have changed it
            // (a usage read, a login).
            let first_in_view = scroll_top.item_ix.min(count);
            let viewport_bottom = self.content_list.viewport_bounds().bottom();
            let after_view = (first_in_view..count)
                .find(|&index| {
                    self.content_list
                        .bounds_for_item(index)
                        .is_none_or(|bounds| bounds.top() >= viewport_bottom)
                })
                .unwrap_or(count);
            self.content_list.remeasure_items(0..first_in_view);
            self.content_list.remeasure_items(after_view..count);
        }

        let items = Rc::new(RefCell::new(items));
        let render_item = cx.processor(move |this, index: usize, window, cx| {
            let mut items = items.borrow_mut();
            // The list can lay out a row more than once in a frame, after it was taken.
            if items.get(index).is_none_or(Option::is_none) {
                let (header, rows) = this.render_content(window, cx);
                *items = content_list_items(header, rows);
            }
            items
                .get_mut(index)
                .and_then(Option::take)
                .unwrap_or_else(|| div().into_any_element())
        });
        div()
            .id("settings-content-scroll")
            .flex_1()
            .min_w_0()
            .h_full()
            .child(list(self.content_list.clone(), render_item).size_full())
            .vertical_scrollbar_for(&self.content_list, window, cx)
            .into_any_element()
    }
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let (header, rows) = self.render_content(window, cx);
        let content = if self.section == Section::Agents
            && matches!(self.agents_page, AgentsPage::Registry)
        {
            self.render_registry(header, window, cx)
        } else {
            self.render_content_list(header, rows, window, cx)
        };
        h_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|_, _: &CloseSettings, _, cx| cx.emit(SettingsPageEvent::Close)))
            .size_full()
            .bg(colors.editor_background)
            .child(self.render_nav(window, cx))
            .child(content)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::time::Duration;

    use agentz_protocol::accounts::{Account, DuplicateLogin, ExtraUsage, LimitPool, StatusRead};
    use agentz_protocol::agents::{AgentSettings, RegistryAgentMetadata, RegistrySnapshot};
    use agentz_protocol::spaces::SpacesSnapshot;
    use agentz_protocol::thread::{ThreadState, ThreadView};
    use agentz_protocol::{AgentSettingsChange, ConnectionId, Response};
    use gpui::TestAppContext;
    use projects::{ImportedSession, ProjectsSnapshot};

    use super::*;
    use crate::server_client::ServerClient;

    pub(super) fn listing(id: &str, name: &str, install_state: InstallState) -> AgentListing {
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
            custom_command: None,
            accounts: None,
        }
    }

    fn agent_page_id(
        page: &Entity<SettingsPage>,
        cx: &mut gpui::VisualTestContext,
    ) -> Option<String> {
        page.read_with(cx, |page, _| {
            page.agent_panel().map(|panel| panel.agent_id.0.to_string())
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
    fn picking_when_a_sound_plays_plays_it(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            crate::machines::init_for_test(vec![client], cx);
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Notifications, window, cx)
        });
        cx.run_until_parked();
        let pick =
            |dropdown: &'static str, item: &'static str, cx: &mut gpui::VisualTestContext| {
                let dropdown = cx.debug_bounds(dropdown).expect("the sound has a dropdown");
                cx.simulate_click(dropdown.center(), gpui::Modifiers::none());
                let item = cx.debug_bounds(item).expect("the menu lists the value");
                cx.simulate_click(item.center(), gpui::Modifiers::none());
                cx.run_until_parked();
                let settings =
                    page.read_with(cx, |page, cx| page.app_settings.read(cx).settings().clone());
                let played = cx.update(|_, cx| sound::take_played_for_test(cx));
                (
                    settings.play_sound_when_finished,
                    settings.play_sound_when_input_needed,
                    played,
                )
            };

        assert_eq!(
            pick("sound-when-finished", "MENU_ITEM-Always", cx),
            (PlaySound::Always, PlaySound::Always, vec![Sound::Finished])
        );
        assert_eq!(
            pick(
                "sound-when-input-needed",
                "MENU_ITEM-When in another thread",
                cx
            ),
            (
                PlaySound::Always,
                PlaySound::WhenInAnotherThread,
                vec![Sound::NeedsInput]
            )
        );
        assert_eq!(
            pick("sound-when-finished", "MENU_ITEM-When in another app", cx),
            (
                PlaySound::WhenInAnotherApp,
                PlaySound::WhenInAnotherThread,
                vec![Sound::Finished]
            )
        );
        assert_eq!(
            pick("sound-when-finished", "MENU_ITEM-Never", cx),
            (PlaySound::Never, PlaySound::WhenInAnotherThread, Vec::new())
        );
    }

    /// Thread titles are off until they're turned on; then the CLI, its model and the model's
    /// reasoning effort can be picked, and each pick goes to the machine's server.
    #[gpui::test]
    fn thread_titles_pick_a_cli_its_model_and_effort(cx: &mut TestAppContext) {
        use agentz_protocol::title_generation::{
            TitleGenerationState, TitleModel, TitleProviderInfo,
        };

        let model = |id: &str, name: &str, efforts: &[&str]| TitleModel {
            id: id.into(),
            name: name.into(),
            efforts: efforts.iter().map(|effort| effort.to_string()).collect(),
        };
        let state = TitleGenerationState {
            settings: TitleGeneration::default(),
            providers: vec![
                TitleProviderInfo {
                    provider: TitleProvider::Codex,
                    installed: true,
                    models: vec![
                        model("gpt-6-luna", "GPT-6-Luna", &["low", "high"]),
                        model("gpt-mini", "GPT-Mini", &[]),
                    ],
                },
                TitleProviderInfo {
                    provider: TitleProvider::Claude,
                    installed: false,
                    models: vec![model("claude-haiku-4-5", "Claude Haiku 4.5", &[])],
                },
                TitleProviderInfo {
                    provider: TitleProvider::Antigravity,
                    installed: false,
                    models: Vec::new(),
                },
            ],
        };
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            client.update(cx, |client, cx| {
                client.set_title_generation_for_test(state, cx)
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::General, window, cx)
        });
        cx.run_until_parked();
        let click = |name: &'static str, cx: &mut gpui::VisualTestContext| {
            let bounds = cx
                .debug_bounds(name)
                .unwrap_or_else(|| panic!("{name} is shown"));
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        let last_sent = |cx: &mut gpui::VisualTestContext| {
            client.read_with(cx, |client, _| client.sent_for_test().last().cloned())
        };
        let sent = |settings: TitleGeneration| Some(Request::SetTitleGeneration(settings));

        assert!(cx.debug_bounds("title-provider").is_none());
        click("generate-titles", cx);
        let enabled = TitleGeneration {
            enabled: true,
            ..TitleGeneration::default()
        };
        assert_eq!(last_sent(cx), sent(enabled.clone()));

        // Codex's default model offers its efforts.
        click("title-effort", cx);
        click("MENU_ITEM-High", cx);
        assert_eq!(
            last_sent(cx),
            sent(TitleGeneration {
                effort: Some("high".into()),
                ..enabled.clone()
            })
        );

        // A model without efforts drops the effort, and its row.
        click("title-model", cx);
        click("MENU_ITEM-GPT-Mini", cx);
        assert_eq!(
            last_sent(cx),
            sent(TitleGeneration {
                model: Some("gpt-mini".into()),
                ..enabled.clone()
            })
        );
        assert!(cx.debug_bounds("title-effort").is_none());

        // Another CLI starts at its own default model, and one that isn't installed says so.
        click("title-provider", cx);
        click("MENU_ITEM-Claude (not installed)", cx);
        assert_eq!(
            last_sent(cx),
            sent(TitleGeneration {
                provider: TitleProvider::Claude,
                ..enabled
            })
        );
    }

    #[gpui::test]
    fn letting_go_of_the_volume_saves_it_and_plays_the_finished_sound(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            crate::machines::init_for_test(vec![client], cx);
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Notifications, window, cx)
        });
        cx.run_until_parked();
        let volume = |cx: &mut gpui::VisualTestContext| {
            page.read_with(cx, |page, cx| {
                page.app_settings.read(cx).settings().sound_volume
            })
        };
        let played =
            |cx: &mut gpui::VisualTestContext| cx.update(|_, cx| sound::take_played_for_test(cx));
        assert_eq!(volume(cx), 1.);
        let slider = cx
            .debug_bounds("sound-volume")
            .expect("the volume has a slider");
        let at = |fraction: f32| {
            gpui::point(
                slider.left() + gpui::px(7.) + (slider.size.width - gpui::px(14.)) * fraction,
                slider.center().y,
            )
        };
        let none = gpui::Modifiers::none();

        // Dragging moves the knob without saving or playing anything.
        cx.simulate_mouse_down(at(1.), gpui::MouseButton::Left, none);
        cx.simulate_mouse_move(at(0.5), gpui::MouseButton::Left, none);
        cx.run_until_parked();
        assert_eq!(volume(cx), 1.);
        assert_eq!(played(cx), []);

        // Past the slider's end, it stops at silent, and letting go anywhere saves it.
        let below = gpui::point(
            slider.left() - gpui::px(40.),
            slider.bottom() + gpui::px(30.),
        );
        cx.simulate_mouse_move(below, gpui::MouseButton::Left, none);
        cx.simulate_mouse_up(below, gpui::MouseButton::Left, none);
        cx.run_until_parked();
        assert_eq!(volume(cx), 0.);
        assert_eq!(played(cx), [Sound::Finished]);

        // A press on the track sets the level there.
        cx.simulate_click(at(0.25), none);
        cx.run_until_parked();
        assert!((volume(cx) - 0.25).abs() < 0.01, "{}", volume(cx));
        assert_eq!(played(cx), [Sound::Finished]);

        // Moving the mouse afterwards changes nothing.
        cx.simulate_mouse_move(at(0.9), None, none);
        cx.run_until_parked();
        assert!((volume(cx) - 0.25).abs() < 0.01);
        assert_eq!(played(cx), []);
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

    #[gpui::test]
    fn custom_agents_are_added_and_configured_from_a_form(cx: &mut TestAppContext) {
        let mut custom = listing(
            "custom-opencode",
            "OpenCode 2",
            InstallState::Installed {
                version: "2.0.21".into(),
                update_available: false,
            },
        );
        custom.custom_command = Some(AgentCommand {
            path: "/opt/opencode/bin/opencode".into(),
            args: vec!["acp".into(), "--quiet".into()],
            env: [("OPENCODE_LOG".to_string(), "debug".to_string())]
                .into_iter()
                .collect(),
            env_remove: Vec::new(),
        });
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
                                "claude",
                                "Claude Agent",
                                InstallState::Installed {
                                    version: "2.0.0".into(),
                                    update_available: false,
                                },
                            ),
                            custom,
                        ],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client], cx);
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| page.show_agents(window, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("agents-add").is_some());
        assert!(cx.debug_bounds("agent-row-custom-opencode").is_some());
        let form_error = |page: &Entity<SettingsPage>, cx: &mut gpui::VisualTestContext| {
            page.read_with(cx, |page, _| match &page.agents_page {
                AgentsPage::CustomAgent(form) => form.error.as_ref().map(|error| error.to_string()),
                _ => None,
            })
        };

        // Add Custom Agent starts on the name, which may stay blank, but needs a command.
        page.update_in(cx, |page, window, cx| {
            page.open_custom_agent_form(None, window, cx)
        });
        cx.run_until_parked();
        cx.simulate_input("My Agent");
        page.read_with(cx, |page, cx| {
            let AgentsPage::CustomAgent(form) = &page.agents_page else {
                panic!("the form is open");
            };
            assert_eq!(form.name.read(cx).text().as_ref(), "My Agent");
        });
        let save = cx
            .debug_bounds("custom-agent-save")
            .expect("the form has Save");
        cx.simulate_click(save.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            form_error(&page, cx).as_deref(),
            Some("Command is required.")
        );
        assert!(cx.debug_bounds("custom-agent-error").is_some());

        // The server starts the agent to check it; its answer shows in the form.
        page.update(cx, |page, cx| {
            if let AgentsPage::CustomAgent(form) = &page.agents_page {
                form.command
                    .update(cx, |input, cx| input.set_text("/bin/agent", cx));
            }
        });
        cx.run_until_parked();
        // The error moved it down.
        let save = cx
            .debug_bounds("custom-agent-save")
            .expect("the form has Save");
        cx.simulate_click(save.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(
            form_error(&page, cx).is_some_and(|error| error.contains("not connected")),
            "a failed save says why"
        );
        let back = cx
            .debug_bounds("agents-back")
            .expect("the form has a way back");
        cx.simulate_click(back.center(), gpui::Modifiers::none());
        assert!(cx.debug_bounds("agent-row-claude").is_some());

        // The ACP Registry lists only the registry's agents.
        page.update_in(cx, |page, window, cx| {
            page.show_agents_page(AgentsPage::Registry, window, cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("registry-card-claude").is_some());
        assert!(cx.debug_bounds("registry-card-custom-opencode").is_none());

        // Configuring a custom agent fills the form in from it, and Cancel goes back to its page.
        page.update_in(cx, |page, window, cx| {
            page.show_agents_page(AgentsPage::Installed, window, cx)
        });
        cx.run_until_parked();
        let row = cx
            .debug_bounds("agent-row-custom-opencode")
            .expect("the custom agent is listed");
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        assert_eq!(agent_page_id(&page, cx).as_deref(), Some("custom-opencode"));
        page.update_in(cx, |page, window, cx| {
            let listing = page
                .registry(cx)
                .read(cx)
                .agent(&AgentId::new("custom-opencode".to_string()))
                .cloned();
            page.open_custom_agent_form(listing.as_ref(), window, cx)
        });
        cx.run_until_parked();
        page.read_with(cx, |page, cx| {
            let AgentsPage::CustomAgent(form) = &page.agents_page else {
                panic!("the form is open");
            };
            assert_eq!(
                form.agent_id.as_ref().map(|id| id.0.as_ref()),
                Some("custom-opencode")
            );
            assert_eq!(form.name.read(cx).text().as_ref(), "OpenCode 2");
            assert_eq!(
                form.command.read(cx).text().as_ref(),
                "/opt/opencode/bin/opencode"
            );
            assert_eq!(form.args.read(cx).text().as_ref(), "acp --quiet");
            let env: Vec<(String, String)> = form
                .env
                .iter()
                .map(|row| {
                    (
                        row.key.read(cx).text().to_string(),
                        row.value.read(cx).text().to_string(),
                    )
                })
                .collect();
            assert_eq!(env, [("OPENCODE_LOG".to_string(), "debug".to_string())]);
        });
        let cancel = cx
            .debug_bounds("custom-agent-cancel")
            .expect("the form has Cancel");
        cx.simulate_click(cancel.center(), gpui::Modifiers::none());
        assert_eq!(agent_page_id(&page, cx).as_deref(), Some("custom-opencode"));
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
                account: None,
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
            page.read_with(cx, |page, _| {
                match page.agent_panel()?.sessions.as_ref()? {
                    SessionList::Listing { .. } => Some("listing"),
                    SessionList::Listed(_) => Some("listed"),
                    SessionList::Failed(_) => Some("failed"),
                }
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
            if let Some(panel) = page.agent_panel_mut() {
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
            page.agent_panel()
                .and_then(|panel| panel.import_error.clone())
        });
        assert!(
            import_error.is_some_and(|error| error.contains("not connected")),
            "a failed import says why"
        );
        assert!(cx.debug_bounds("session-import-s-00").is_some());

        // Another project shows its own sessions.
        page.update(cx, |page, cx| {
            if let Some(panel) = page.agent_panel_mut() {
                panel.sessions_project = Some(empty);
            }
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("agent-session-s-00").is_none());
    }

    /// The settings page under a modal layer like the shell's, which shows its account dialog.
    struct DialogHost {
        page: Entity<SettingsPage>,
        dialog: Option<(Entity<AccountDialog>, Subscription)>,
        _subscription: Subscription,
    }

    impl DialogHost {
        fn new(cx: &mut Context<Self>) -> Self {
            let page = cx.new(SettingsPage::new);
            let subscription = cx.subscribe(&page, |this, _, event: &SettingsPageEvent, cx| {
                if let SettingsPageEvent::OpenDialog(dialog) = event {
                    let dismissed = cx.subscribe(dialog, |this, _, _: &DismissEvent, cx| {
                        this.dialog = None;
                        cx.notify();
                    });
                    this.dialog = Some((dialog.clone(), dismissed));
                    cx.notify();
                }
            });
            Self {
                page,
                dialog: None,
                _subscription: subscription,
            }
        }
    }

    impl Render for DialogHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .relative()
                .child(self.page.clone())
                .children(self.dialog.as_ref().map(|(dialog, _)| {
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .occlude()
                        .child(dialog.clone())
                }))
        }
    }

    fn open_page_with_dialogs(
        cx: &mut TestAppContext,
    ) -> (Entity<SettingsPage>, &mut gpui::VisualTestContext) {
        let (host, cx) = cx.add_window_view(|_, cx| DialogHost::new(cx));
        let page = host.read_with(cx, |host, _| host.page.clone());
        (page, cx)
    }

    /// Add Account is a dialog: how to log in and Copy settings from, the login's progress or
    /// why it failed, then the account added. An account that's already there is refused, and
    /// Cancel removes the empty account.
    #[gpui::test]
    fn adding_an_account_is_a_dialog_from_login_to_result(cx: &mut TestAppContext) {
        let hour = Duration::from_secs(3600);
        let mock = AgentId::new("mock");
        let mut mock_listing = listing(
            "mock",
            "Mock",
            InstallState::Installed {
                version: "2.0.0".into(),
                update_available: false,
            },
        );
        mock_listing.accounts = Some(AccountSupport {
            folder: "/tmp/agentz-test/accounts/mock".into(),
            reads_usage: true,
            ..AccountSupport::default()
        });
        let mut accounts = AgentAccounts {
            external_logged_in: Some(true),
            external_status: Some(StatusRead {
                status: AccountStatus {
                    email: Some("alex@hey.com".into()),
                    windows: vec![limit("5-hour", 38., 2 * hour, 5 * hour)],
                    ..AccountStatus::default()
                },
                read_at: SystemTime::now(),
            }),
            ..AgentAccounts::default()
        };
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let next_id = Rc::new(std::cell::Cell::new(3));
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            super::init(cx);
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
                        agents: vec![mock_listing],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            let requests = requests.clone();
            let next_id = next_id.clone();
            let accounts = accounts.clone();
            client.update(cx, |client, cx| {
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    match request {
                        Request::OpenLoginSession { account, .. } => {
                            Some(Response::LoginSessionOpened(account.map_or(100, |id| id.0)))
                        }
                        Request::SubscribeThread(ConnectionId::LoginSession(id)) => {
                            Some(Response::Thread(login_session(*id == 100)))
                        }
                        Request::AddAccount(_) => {
                            let id = next_id.get();
                            next_id.set(id + 1);
                            Some(Response::AccountAdded(AccountId(id)))
                        }
                        Request::RemoveAccount { .. } | Request::Authenticate { .. } => {
                            Some(Response::Ok)
                        }
                        _ => None,
                    }
                });
                client.set_accounts_for_test([(mock.clone(), accounts)].into(), cx);
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        let (page, cx) = open_page_with_dialogs(cx);
        page.update_in(cx, |page, window, cx| {
            page.show_agent_accounts(MachineId::Local, &mock, false, window, cx)
        });
        cx.run_until_parked();
        let click = |selector: &'static str, cx: &mut gpui::VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} is shown"));
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        let sent = |request: &Request| requests.borrow().contains(request);
        let removed = |id: u64| Request::RemoveAccount {
            agent_id: mock.clone(),
            account: AccountId(id),
        };
        // The server adds the account it was asked for, which then has its login session.
        let publish = |accounts: &AgentAccounts, cx: &mut gpui::VisualTestContext| {
            client.update(cx, |client, cx| {
                client.set_accounts_for_test([(mock.clone(), accounts.clone())].into(), cx)
            });
            cx.run_until_parked();
        };
        let list =
            |accounts: &mut AgentAccounts, account: Account, cx: &mut gpui::VisualTestContext| {
                accounts.last_id = account.id.0;
                accounts.accounts.push(account);
                publish(accounts, cx);
            };
        let connection = |id: u64, cx: &mut gpui::VisualTestContext| {
            page.read_with(cx, |page, _| {
                page.agent_panel()
                    .and_then(|panel| panel.session(Some(AccountId(id))))
                    .map(|session| session.connection.clone())
                    .expect("the account has its login session")
            })
        };

        // One account is its card, with no lines.
        assert!(cx.debug_bounds("account-card-external").is_some());
        assert!(cx.debug_bounds("account-lines").is_none());
        click("account-add", cx);
        assert!(sent(&Request::AddAccount(mock.clone())));
        assert!(cx.debug_bounds("add-account-dialog").is_some());
        let mut new_account = account(3, None, None);
        new_account.settings_from = SettingsSource::External;
        list(&mut accounts, new_account, cx);

        // 1: how to log in, and Copy settings from. The account isn't on the page yet.
        assert!(cx.debug_bounds("add-account-dialog").is_some());
        assert!(cx.debug_bounds("copy-settings-3").is_some());
        assert!(cx.debug_bounds("add-account-back").is_none());
        assert!(cx.debug_bounds("account-lines").is_none());
        assert!(cx.debug_bounds("account-card-3").is_none());
        click("login-method-login", cx);
        assert!(requests.borrow().iter().any(|request| matches!(
            request,
            Request::Authenticate {
                connection: ConnectionId::LoginSession(3),
                ..
            }
        )));

        // 2: its progress, with Back; then why it failed, with Try Again.
        let thread = connection(3, cx);
        thread.update(cx, |thread, cx| {
            thread.update_state_for_test(
                |state| state.authenticating = Some(acp::AuthMethodId::new("login")),
                cx,
            )
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("copy-settings-3").is_none());
        assert!(cx.debug_bounds("add-account-back").is_some());
        thread.update(cx, |thread, cx| {
            thread.update_state_for_test(
                |state| {
                    state.authenticating = None;
                    state.auth_error = Some("The code expired.".into());
                },
                cx,
            )
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("login-try-again").is_some());
        click("add-account-back", cx);
        assert!(cx.debug_bounds("login-method-login").is_some());
        assert!(cx.debug_bounds("login-try-again").is_none());

        // 3: logged in, the account added, with its limits, and Done.
        click("login-method-login", cx);
        thread.update(cx, |thread, cx| {
            thread.update_state_for_test(
                |state| {
                    state.auth_error = None;
                    state.status = ConnectionStatus::Ready;
                    state.logged_in = Some(true);
                },
                cx,
            )
        });
        accounts.accounts[0] = Account {
            settings_from: SettingsSource::External,
            ..account(
                3,
                None,
                Some(AccountStatus {
                    email: Some("work@acme.co".into()),
                    plan: Some("Pro".into()),
                    windows: vec![limit("5-hour", 10., 2 * hour, 5 * hour)],
                    ..AccountStatus::default()
                }),
            )
        };
        publish(&accounts, cx);
        assert!(cx.debug_bounds("add-account-added").is_some());
        assert!(cx.debug_bounds("limit-added-3-window-0").is_some());
        click("add-account-done", cx);
        assert!(cx.debug_bounds("add-account-dialog").is_none());
        assert!(!sent(&removed(3)));
        // It sits in the list, which is lines now.
        assert!(cx.debug_bounds("account-line-external").is_some());
        assert!(cx.debug_bounds("account-line-3").is_some());

        // A login that's an account already there adds nothing, and says so.
        click("account-add", cx);
        list(&mut accounts, account(4, None, None), cx);
        assert!(cx.debug_bounds("login-method-login").is_some());
        accounts
            .accounts
            .retain(|account| account.id != AccountId(4));
        accounts.duplicate = Some(DuplicateLogin {
            account: AccountId(4),
            email: "work@acme.co".into(),
        });
        publish(&accounts, cx);
        assert!(cx.debug_bounds("add-account-duplicate").is_some());
        click("add-account-done", cx);
        assert!(cx.debug_bounds("add-account-dialog").is_none());
        assert!(!sent(&removed(4)));

        // Cancel, or closing the dialog, removes the account added for it.
        click("account-add", cx);
        list(&mut accounts, account(5, None, None), cx);
        click("add-account-cancel", cx);
        assert!(cx.debug_bounds("add-account-dialog").is_none());
        assert!(sent(&removed(5)));
        click("account-add", cx);
        list(&mut accounts, account(6, None, None), cx);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(cx.debug_bounds("add-account-dialog").is_none());
        assert!(sent(&removed(6)));
    }

    fn limit(label: &str, used_percent: f64, resets_in: Duration, length: Duration) -> LimitWindow {
        LimitWindow {
            label: label.into(),
            used_percent,
            resets_at: Some(SystemTime::now() + resets_in),
            length: Some(length),
        }
    }

    pub(super) fn account(id: u64, label: Option<&str>, status: Option<AccountStatus>) -> Account {
        Account {
            id: AccountId(id),
            choices: AccountChoices {
                label: label.map(str::to_string),
                ..AccountChoices::default()
            },
            settings: AgentSettings::default(),
            settings_from: SettingsSource::default(),
            logged_in: Some(status.is_some()),
            logs_in_with_key: false,
            status: status.map(|status| StatusRead {
                status,
                read_at: SystemTime::now() - Duration::from_secs(180),
            }),
        }
    }

    /// A login session as the server sends it: logged in, or offering its login.
    fn login_session(logged_in: bool) -> ThreadView {
        ThreadView {
            state: ThreadState {
                status: if logged_in {
                    ConnectionStatus::Ready
                } else {
                    ConnectionStatus::AuthRequired
                },
                agent_name: "Mock".into(),
                logged_in: Some(logged_in),
                auth_methods: if logged_in {
                    Vec::new()
                } else {
                    vec![acp::AuthMethod::Agent(acp::AuthMethodAgent::new(
                        "login", "Log In",
                    ))]
                },
                ..ThreadState::default()
            },
            entries: Vec::new(),
        }
    }

    #[gpui::test]
    fn several_accounts_are_lines_whose_cards_open_in_a_dialog(cx: &mut TestAppContext) {
        let hour = Duration::from_secs(3600);
        let mock = AgentId::new("mock");
        let mut mock_listing = listing(
            "mock",
            "Mock",
            InstallState::Installed {
                version: "2.0.0".into(),
                update_available: false,
            },
        );
        mock_listing.accounts = Some(AccountSupport {
            folder: "/tmp/agentz-test/accounts/mock".into(),
            reads_usage: true,
            usage_page: Some("https://example.com/usage".into()),
            extra_usage_page: Some("https://example.com/extra-usage".into()),
            copies_settings_files: true,
            loads_skills: false,
        });
        let work = AccountStatus {
            email: Some("alex@acme.co".into()),
            plan: Some("Team".into()),
            windows: vec![
                limit("5-hour", 100., 2 * hour, 5 * hour),
                limit("Weekly", 56., 72 * hour, 168 * hour),
            ],
            // Codex-like: a reset it was granted.
            limit_resets: Some(LimitResets {
                available: 1,
                next_expires_at: Some(SystemTime::now() + 27 * 24 * hour),
            }),
            extra_usage: Some(ExtraUsage {
                label: "Credits".into(),
                summary: "1,240 left".into(),
            }),
            ..AccountStatus::default()
        };
        let mut accounts = AgentAccounts {
            accounts: vec![account(1, Some("Work"), Some(work)), account(2, None, None)],
            default_account: Some(AccountId(1)),
            external_logged_in: Some(true),
            external_status: Some(StatusRead {
                // Droid-like: pools, a balance and its choice at a limit.
                status: AccountStatus {
                    email: Some("alex@hey.com".into()),
                    plan: Some("Max 5x".into()),
                    windows: vec![limit("5-hour", 38., 2 * hour, 5 * hour)],
                    pool: Some("Standard".into()),
                    other_pools: vec![LimitPool {
                        label: "Droid Core".into(),
                        windows: vec![
                            limit("5-hour", 3., 2 * hour, 5 * hour),
                            limit("Weekly", 4., 72 * hour, 168 * hour),
                        ],
                    }],
                    credits: Some("$12.40".into()),
                    overage: Some(Overage {
                        preference: None,
                        can_change: true,
                        extra_usage_allowed: true,
                    }),
                    ..AccountStatus::default()
                },
                read_at: SystemTime::now(),
            }),
            last_id: 2,
            ..AgentAccounts::default()
        };
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            super::init(cx);
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
                        agents: vec![mock_listing],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            let requests = requests.clone();
            let accounts = accounts.clone();
            client.update(cx, |client, cx| {
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    match request {
                        // The External account's session is 100, the others' their account's.
                        Request::OpenLoginSession { account, .. } => {
                            Some(Response::LoginSessionOpened(account.map_or(100, |id| id.0)))
                        }
                        Request::SubscribeThread(ConnectionId::LoginSession(id)) => {
                            Some(Response::Thread(login_session(*id == 100 || *id == 1)))
                        }
                        Request::AddAccount(_) => Some(Response::AccountAdded(AccountId(3))),
                        Request::RemoveAccount { .. }
                        | Request::UpdateAccount { .. }
                        | Request::RefreshUsage { .. }
                        | Request::SwitchToDroidCore { .. }
                        | Request::UseLimitReset { .. } => Some(Response::Ok),
                        _ => None,
                    }
                });
                client.set_accounts_for_test([(AgentId::new("mock"), accounts)].into(), cx);
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        let (page, cx) = open_page_with_dialogs(cx);
        let confirms: Rc<RefCell<Vec<ConfirmRequest>>> = Rc::default();
        cx.update(|_, cx| {
            let confirms = confirms.clone();
            cx.subscribe(&page, move |_, event: &SettingsPageEvent, _| {
                if let SettingsPageEvent::Confirm(request) = event {
                    confirms.borrow_mut().push(request.clone());
                }
            })
            .detach();
        });
        page.update_in(cx, |page, window, cx| page.show_agents(window, cx));
        cx.run_until_parked();
        let click = |selector: &'static str, cx: &mut gpui::VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} is shown"));
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        click("agent-row-mock", cx);
        let sent = |request: &Request| requests.borrow().contains(request);

        // Several accounts are lines: the External account first, then agentZ's in the order
        // they were added, each with a cell per window. Their cards open in a dialog.
        let external = cx
            .debug_bounds("account-line-external")
            .expect("the External account is listed");
        let work = cx
            .debug_bounds("account-line-1")
            .expect("an agentZ account is listed");
        let new = cx
            .debug_bounds("account-line-2")
            .expect("a new account is listed");
        assert!(external.top() < work.top() && work.top() < new.top());
        assert!(cx.debug_bounds("account-card-1").is_none());
        assert!(cx.debug_bounds("limit-line-1-bar-0").is_some());
        assert!(cx.debug_bounds("limit-line-1-bar-1").is_some());
        assert!(cx.debug_bounds("limit-line-external-bar-1").is_none());
        assert!(cx.debug_bounds("limit-line-2-bar-0").is_none());
        assert!(cx.debug_bounds("account-dialog").is_none());

        click("account-line-external", cx);
        assert!(cx.debug_bounds("account-dialog").is_some());
        assert!(cx.debug_bounds("account-card-external").is_some());
        assert!(cx.debug_bounds("account-tag-outside").is_some());
        assert!(cx.debug_bounds("account-tag-default-external").is_none());
        // The head says when the limits were read, before the ⋯ menu and ×.
        let read = cx
            .debug_bounds("account-read-external")
            .expect("the read's time is shown");
        let menu = cx
            .debug_bounds("account-menu-external")
            .expect("the account has a menu");
        let close = cx
            .debug_bounds("account-dialog-close")
            .expect("the dialog has ×");
        assert!(read.right() <= menu.left() && menu.right() <= close.left());

        // A row per window, with a line where even spending would be: 2 hours of 5 are left.
        assert!(cx.debug_bounds("limit-external-window-0").is_some());
        assert!(cx.debug_bounds("limit-external-window-1").is_none());
        let bar = cx
            .debug_bounds("limit-external-bar-0")
            .expect("the window has a bar");
        let hairline = cx
            .debug_bounds("limit-external-hairline-0")
            .expect("the bar has its line");
        let share = f32::from(hairline.left() - bar.left()) / f32::from(bar.size.width);
        assert!((share - 0.4).abs() < 0.02, "the line is at {share}");

        // An account with pools has a tab for each, and one for its extra usage balance.
        let tabs = cx
            .debug_bounds("limit-tabs-external")
            .expect("the External account's pools are tabs");
        assert!(tabs.bottom() <= bar.top());
        assert!(cx.debug_bounds("limit-external-1-window-1").is_none());
        cx.simulate_click(tabs.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("limit-external-1-window-1").is_some());
        assert!(cx.debug_bounds("limit-external-window-0").is_none());
        // The dialog is centered, so it moves as the tab changes its height.
        let tabs = cx
            .debug_bounds("limit-tabs-external")
            .expect("the tabs are still there");
        cx.simulate_click(
            gpui::point(tabs.right() - px(4.), tabs.center().y),
            gpui::Modifiers::none(),
        );
        cx.run_until_parked();
        assert!(cx.debug_bounds("limit-external-balance").is_some());
        assert!(cx.debug_bounds("limit-external-1-window-1").is_none());

        // Droid's own "When a limit is reached" saves Switch to Droid Core.
        let overage = cx
            .debug_bounds("overage-external")
            .expect("the External account has Droid's choice");
        assert!(cx.debug_bounds("at-limit-external").is_some());
        cx.simulate_click(overage.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("overage-external-extra-usage").is_some());
        click("overage-external-droid-core", cx);
        assert!(sent(&Request::SwitchToDroidCore {
            agent_id: mock.clone(),
            account: None,
        }));
        assert!(cx.debug_bounds("limit-resets-external").is_none());
        assert!(cx.debug_bounds("extra-usage-external").is_none());
        click("account-dialog-close", cx);
        assert!(cx.debug_bounds("account-dialog").is_none());

        // An account with limit resets has a line for them under its limits, whose Use Reset
        // asks first.
        click("account-line-1", cx);
        assert!(cx.debug_bounds("account-tag-default-1").is_some());
        assert!(cx.debug_bounds("overage-1").is_none());
        assert!(cx.debug_bounds("limit-tabs-1").is_none());
        let resets = cx
            .debug_bounds("limit-resets-1")
            .expect("Work's resets are under its limits");
        let weekly = cx
            .debug_bounds("limit-1-window-1")
            .expect("Work has a weekly window");
        assert!(resets.top() >= weekly.bottom());
        // Its credits are under them, with the vendor's page to manage them.
        let extra_usage = cx
            .debug_bounds("extra-usage-1")
            .expect("Work's credits are under its limits");
        assert!(extra_usage.top() >= resets.bottom());
        assert!(cx.debug_bounds("extra-usage-manage-1").is_some());
        click("use-limit-reset-1", cx);
        assert!(
            !requests
                .borrow()
                .iter()
                .any(|request| matches!(request, Request::UseLimitReset { .. }))
        );
        let confirm = confirms
            .borrow_mut()
            .pop()
            .expect("using a reset asks first");
        assert_eq!(confirm.title.as_ref(), "Use a limit reset?");
        assert_eq!(
            confirm.message.as_ref(),
            "This clears Work's 5-hour and weekly limits now (alex@acme.co, Mock). It uses your \
             only reset and can't be undone."
        );
        cx.update(|window, cx| (confirm.on_confirm)(window, cx));
        cx.run_until_parked();
        assert!(sent(&Request::UseLimitReset {
            agent_id: mock.clone(),
            account: Some(AccountId(1)),
        }));

        // The ⋯ menu refreshes the usage.
        click("account-menu-1", cx);
        click("account-menu-refresh", cx);
        assert!(sent(&Request::RefreshUsage {
            agent_id: mock.clone(),
            account: Some(AccountId(1)),
        }));
        assert!(cx.debug_bounds("account-menu-refresh").is_none());

        // "When a limit is reached" offers Stop and Continue at reset, under the limits.
        let at_limit = cx
            .debug_bounds("at-limit-1")
            .expect("the account has the choice");
        assert!(at_limit.top() > extra_usage.bottom());
        cx.simulate_click(at_limit.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("at-limit-1-Stop").is_some());
        click("at-limit-1-ContinueAtReset", cx);
        assert!(sent(&Request::UpdateAccount {
            agent_id: mock.clone(),
            account: Some(AccountId(1)),
            change: AccountChange::SetAtLimit(AtLimit::ContinueAtReset),
        }));

        // Rename edits the name in place: Enter keeps it, Escape doesn't.
        let rename = |cx: &mut gpui::VisualTestContext| {
            page.update_in(cx, |page, window, cx| {
                page.start_account_rename(Some(AccountId(1)), window, cx);
                page.focus_account_rename(window, cx);
            });
            cx.run_until_parked();
        };
        rename(cx);
        page.read_with(cx, |page, cx| {
            let rename = page
                .agent_panel()
                .and_then(|panel| panel.renaming.as_ref())
                .expect("the name is being edited");
            assert_eq!(rename.input.read(cx).text().as_ref(), "Work");
        });
        cx.simulate_input("Job");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(sent(&Request::UpdateAccount {
            agent_id: mock.clone(),
            account: Some(AccountId(1)),
            change: AccountChange::Rename(Some("Job".into())),
        }));
        assert!(page.read_with(cx, |page, _| {
            page.agent_panel()
                .is_some_and(|panel| panel.renaming.is_none())
        }));
        requests.borrow_mut().clear();
        rename(cx);
        cx.simulate_input("Side");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(
            !requests
                .borrow()
                .iter()
                .any(|request| matches!(request, Request::UpdateAccount { .. }))
        );
        assert!(page.read_with(cx, |page, _| {
            page.agent_panel()
                .is_some_and(|panel| panel.renaming.is_none())
        }));
        // Focus is back in the dialog, which Escape closes.
        assert!(cx.debug_bounds("account-dialog").is_some());
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(cx.debug_bounds("account-dialog").is_none());

        // An account that hasn't logged in yet is the New account card, which Cancel removes
        // without asking.
        click("account-line-2", cx);
        assert!(cx.debug_bounds("account-menu-2").is_none());
        assert!(cx.debug_bounds("limit-2-window-0").is_none());
        click("account-cancel-2", cx);
        assert!(sent(&Request::RemoveAccount {
            agent_id: mock.clone(),
            account: AccountId(2),
        }));
        assert!(confirms.borrow().is_empty());
        accounts
            .accounts
            .retain(|account| account.id != AccountId(2));
        client.update(cx, |client, cx| {
            client.set_accounts_for_test([(mock.clone(), accounts.clone())].into(), cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("account-dialog").is_none());
        assert!(cx.debug_bounds("account-line-2").is_none());

        // Removing an account asks first.
        page.update(cx, |page, cx| {
            page.confirm_remove_account(AccountId(1), "Work".into(), cx)
        });
        let confirm = confirms.borrow_mut().pop().expect("removing asks first");
        assert_eq!(confirm.title.as_ref(), "Remove Work?");
        cx.update(|window, cx| (confirm.on_confirm)(window, cx));
        cx.run_until_parked();
        assert!(sent(&Request::RemoveAccount {
            agent_id: mock,
            account: AccountId(1),
        }));
    }

    /// Scrolling lays the page out every frame, so laying out every account's line made a
    /// scroll lag.
    #[gpui::test]
    fn an_agents_page_lays_out_only_the_account_lines_in_view(cx: &mut TestAppContext) {
        let mut mock_listing = listing(
            "mock",
            "Mock",
            InstallState::Installed {
                version: "2.0.0".into(),
                update_available: false,
            },
        );
        mock_listing.accounts = Some(AccountSupport {
            folder: "/tmp/agentz-test/accounts/mock".into(),
            reads_usage: true,
            usage_page: None,
            extra_usage_page: None,
            copies_settings_files: false,
            loads_skills: false,
        });
        let accounts = AgentAccounts {
            accounts: (1..=100).map(|id| account(id, None, None)).collect(),
            last_id: 100,
            ..AgentAccounts::default()
        };
        cx.update(|cx| {
            crate::init_for_test(cx);
            super::init(cx);
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
                        agents: vec![mock_listing],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            client.update(cx, |client, cx| {
                client.answer_for_test(|request| match request {
                    Request::OpenLoginSession { account, .. } => {
                        Some(Response::LoginSessionOpened(account.map_or(100, |id| id.0)))
                    }
                    Request::SubscribeThread(ConnectionId::LoginSession(_)) => {
                        Some(Response::Thread(login_session(false)))
                    }
                    _ => None,
                });
                client.set_accounts_for_test([(AgentId::new("mock"), accounts)].into(), cx);
            });
            crate::machines::init_for_test(vec![client], cx);
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.show_agent_accounts(MachineId::Local, &AgentId::new("mock"), false, window, cx)
        });
        cx.run_until_parked();

        let first = cx
            .debug_bounds("account-line-1")
            .expect("the first account is in view");
        assert!(cx.debug_bounds("account-line-100").is_none());

        cx.simulate_event(gpui::ScrollWheelEvent {
            position: first.center(),
            delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(-100_000.))),
            ..Default::default()
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("account-line-100").is_some());
        assert!(cx.debug_bounds("account-line-1").is_none());
    }

    /// Settings › Usage has a table per agent: a column per window, "All N accounts" with what's
    /// left across them, then a row per account, the External one first, without the agents
    /// that read no usage or the accounts found logged out. A row opens its account on the
    /// agent's page.
    #[gpui::test]
    fn the_usage_page_has_a_table_per_agent(cx: &mut TestAppContext) {
        let hour = Duration::from_secs(3600);
        let mock = AgentId::new("mock");
        let other = AgentId::new("other");
        let installed = InstallState::Installed {
            version: "2.0.0".into(),
            update_available: false,
        };
        let mut mock_listing = listing("mock", "Mock", installed.clone());
        mock_listing.accounts = Some(AccountSupport {
            folder: "/tmp/agentz-test/accounts/mock".into(),
            reads_usage: true,
            ..AccountSupport::default()
        });
        let mut other_listing = listing("other", "Other", installed);
        other_listing.accounts = Some(AccountSupport::default());
        let status = |windows: Vec<LimitWindow>| AccountStatus {
            windows,
            ..AccountStatus::default()
        };
        let five_hour = |used_percent: f64| limit("5-hour", used_percent, 2 * hour, 5 * hour);
        let mut logged_out = account(3, Some("Old"), Some(status(vec![five_hour(0.)])));
        logged_out.logged_in = Some(false);
        let accounts = AgentAccounts {
            accounts: vec![
                account(
                    1,
                    Some("Work"),
                    Some(status(vec![
                        five_hour(100.),
                        limit("Weekly", 56., 72 * hour, 168 * hour),
                    ])),
                ),
                account(2, Some("Side"), Some(status(vec![five_hour(0.)]))),
                logged_out,
            ],
            external_logged_in: Some(true),
            external_status: Some(StatusRead {
                status: status(vec![five_hour(38.)]),
                read_at: SystemTime::now(),
            }),
            last_id: 3,
            ..AgentAccounts::default()
        };
        let other_accounts = AgentAccounts {
            external_logged_in: Some(true),
            external_status: Some(StatusRead {
                status: status(vec![five_hour(10.)]),
                read_at: SystemTime::now(),
            }),
            ..AgentAccounts::default()
        };

        // Each account counts the same: 62, 0 and 100 left are 54 across them.
        let table = UsageTable::new(account_entries(&accounts));
        assert_eq!(table.columns, vec!["5-hour", "Weekly"]);
        let rows: Vec<Option<AccountId>> = table.rows.iter().map(|entry| entry.account).collect();
        assert_eq!(rows, vec![None, Some(AccountId(1)), Some(AccountId(2))]);
        let now = SystemTime::now();
        let pooled: Vec<Option<u8>> = (0..3)
            .map(|column| {
                table
                    .pooled(column, now)
                    .map(|window| window.left_percent())
            })
            .collect();
        assert_eq!(pooled, vec![Some(54), Some(44), None]);
        // Past their resets, the 5-hour windows are left out until they're read again.
        let later = now + 3 * hour;
        let pooled: Vec<Option<u8>> = (0..2)
            .map(|column| {
                table
                    .pooled(column, later)
                    .map(|window| window.left_percent())
            })
            .collect();
        assert_eq!(pooled, vec![None, Some(44)]);

        cx.update(|cx| {
            crate::init_for_test(cx);
            super::init(cx);
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
                        agents: vec![mock_listing, other_listing],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            client.update(cx, |client, cx| {
                client.answer_for_test(move |request| match request {
                    Request::OpenLoginSession { account, .. } => {
                        Some(Response::LoginSessionOpened(account.map_or(100, |id| id.0)))
                    }
                    Request::SubscribeThread(ConnectionId::LoginSession(_)) => {
                        Some(Response::Thread(login_session(true)))
                    }
                    _ => None,
                });
                client.set_accounts_for_test(
                    [(mock.clone(), accounts), (other, other_accounts)].into(),
                    cx,
                );
            });
            crate::machines::init_for_test(vec![client], cx);
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Usage, window, cx)
        });
        cx.run_until_parked();

        assert!(cx.debug_bounds("usage-mock").is_some());
        assert!(cx.debug_bounds("usage-other").is_none());
        let all = cx
            .debug_bounds("usage-mock-all")
            .expect("the accounts together");
        let external = cx
            .debug_bounds("usage-mock-external")
            .expect("the External account");
        let work = cx.debug_bounds("usage-mock-1").expect("an agentZ account");
        let side = cx
            .debug_bounds("usage-mock-2")
            .expect("another agentZ account");
        assert!(all.top() < external.top() && external.top() < work.top());
        assert!(work.top() < side.top());
        assert!(cx.debug_bounds("usage-mock-3").is_none());
        assert!(cx.debug_bounds("limit-usage-mock-all-bar-1").is_some());
        assert!(cx.debug_bounds("limit-usage-mock-external-bar-0").is_some());
        assert!(cx.debug_bounds("limit-usage-mock-external-bar-1").is_none());

        let weekly = cx
            .debug_bounds("limit-usage-mock-1-bar-1")
            .expect("Work's weekly window");
        assert!(weekly.left() > work.left() + work.size.width / 2.);
        cx.simulate_click(work.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(agent_page_id(&page, cx).as_deref(), Some("mock"));
        page.read_with(cx, |page, _| {
            assert!(page.section == Section::Agents);
            assert!(
                page.agent_panel()
                    .is_some_and(|panel| panel.tab == AgentTab::Account)
            );
        });
    }

    /// Under the tables, the timeline has a lane per account under its agent, drawing its
    /// longest window that fits two weeks, a tick where its limit reset expires, and a lane
    /// for an account with no window counting down. 5-hour draws only the 5-hour windows, and
    /// a lane opens its account on the agent's page.
    #[gpui::test]
    fn the_usage_timeline_has_a_lane_per_account(cx: &mut TestAppContext) {
        let hour = Duration::from_secs(3600);
        let installed = InstallState::Installed {
            version: "2.0.0".into(),
            update_available: false,
        };
        let support = AccountSupport {
            folder: "/tmp/agentz-test/accounts/mock".into(),
            reads_usage: true,
            ..AccountSupport::default()
        };
        let mut mock_listing = listing("mock", "Mock", installed.clone());
        mock_listing.accounts = Some(support.clone());
        let mut codex_listing = listing("codex", "Codex", installed);
        codex_listing.accounts = Some(support);
        let status = |windows: Vec<LimitWindow>| AccountStatus {
            windows,
            ..AccountStatus::default()
        };
        let mut unused_weekly = limit("Weekly", 0., hour, 168 * hour);
        unused_weekly.resets_at = None;
        let mock_accounts = AgentAccounts {
            accounts: vec![
                account(
                    1,
                    Some("Work"),
                    Some(status(vec![
                        limit("5-hour", 30., 2 * hour, 5 * hour),
                        limit("Weekly", 56., 72 * hour, 168 * hour),
                        limit("Monthly", 20., 400 * hour, 720 * hour),
                    ])),
                ),
                account(2, Some("Side"), Some(status(vec![unused_weekly]))),
            ],
            last_id: 2,
            ..AgentAccounts::default()
        };
        let codex_accounts = AgentAccounts {
            external_logged_in: Some(true),
            external_status: Some(StatusRead {
                status: AccountStatus {
                    limit_resets: Some(LimitResets {
                        available: 1,
                        next_expires_at: Some(SystemTime::now() + 100 * hour),
                    }),
                    ..status(vec![limit("Weekly", 19., 120 * hour, 168 * hour)])
                },
                read_at: SystemTime::now(),
            }),
            ..AgentAccounts::default()
        };
        cx.update(|cx| {
            crate::init_for_test(cx);
            super::init(cx);
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
                        agents: vec![mock_listing, codex_listing],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            client.update(cx, |client, cx| {
                client.set_accounts_for_test(
                    [
                        (AgentId::new("mock"), mock_accounts),
                        (AgentId::new("codex"), codex_accounts),
                    ]
                    .into(),
                    cx,
                );
            });
            crate::machines::init_for_test(vec![client], cx);
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        cx.simulate_resize(gpui::size(px(1100.), px(4000.)));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Usage, window, cx)
        });
        cx.run_until_parked();

        let timeline = cx.debug_bounds("usage-timeline").expect("the timeline");
        let table = cx.debug_bounds("usage-mock").expect("Mock's table");
        assert!(timeline.top() > table.bottom());
        // Codex sorts first, as its table does.
        let codex = cx
            .debug_bounds("timeline-codex-external")
            .expect("Codex's lane");
        let work = cx.debug_bounds("timeline-mock-1").expect("Work's lane");
        let side = cx.debug_bounds("timeline-mock-2").expect("Side's lane");
        assert!(codex.top() < work.top() && work.top() < side.top());
        assert!(cx.debug_bounds("timeline-mock-1-current").is_some());
        assert!(
            cx.debug_bounds("timeline-codex-external-limit-reset")
                .is_some()
        );
        assert!(cx.debug_bounds("timeline-mock-2-idle").is_some());
        assert!(cx.debug_bounds("usage-timeline-legend").is_some());

        let zoom = cx
            .debug_bounds("usage-timeline-zoom")
            .expect("the zoom switch");
        let five_hour = gpui::point(zoom.right() - px(20.), zoom.center().y);
        cx.simulate_click(five_hour, gpui::Modifiers::none());
        cx.run_until_parked();
        page.read_with(cx, |page, _| {
            assert_eq!(page.usage_timeline_zoom, TimelineZoom::FiveHour)
        });
        let work = cx
            .debug_bounds("timeline-mock-1")
            .expect("Work's 5-hour lane");
        assert!(cx.debug_bounds("timeline-codex-external").is_none());
        assert!(cx.debug_bounds("timeline-mock-2").is_none());

        cx.simulate_click(
            gpui::point(work.left() + px(40.), work.center().y),
            gpui::Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(agent_page_id(&page, cx).as_deref(), Some("mock"));
    }

    #[gpui::test]
    fn the_tabs_show_the_picked_accounts_settings(cx: &mut TestAppContext) {
        let mock = AgentId::new("mock");
        let mut mock_listing = listing(
            "mock",
            "Mock",
            InstallState::Installed {
                version: "2.0.0".into(),
                update_available: false,
            },
        );
        mock_listing.accounts = Some(AccountSupport {
            folder: "/tmp/agentz-test/accounts/mock".into(),
            copies_settings_files: true,
            ..AccountSupport::default()
        });
        let options: Vec<acp::SessionConfigOption> = serde_json::from_value(serde_json::json!([
            {"id": "model", "name": "Model", "type": "select", "currentValue": "sonnet",
             "options": [{"value": "sonnet", "name": "Sonnet"}, {"value": "haiku", "name": "Haiku"}]},
        ]))
        .expect("options");
        let settings = |variable: &str, model: Option<&str>| AgentSettings {
            env: BTreeMap::from([(variable.to_string(), "1".to_string())]),
            known_config_options: options.clone(),
            default_config_options: model
                .map(|model| {
                    (
                        "model".to_string(),
                        acp::SessionConfigOptionValue::value_id(model.to_string()),
                    )
                })
                .into_iter()
                .collect(),
            ..AgentSettings::default()
        };
        let mut work = account(1, Some("Work"), Some(AccountStatus::default()));
        work.settings = settings("WORK_TOKEN", Some("haiku"));
        let mut accounts = AgentAccounts {
            accounts: vec![work, account(2, None, None)],
            default_account: Some(AccountId(1)),
            external_logged_in: Some(true),
            last_id: 2,
            ..AgentAccounts::default()
        };
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            super::init(cx);
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
                        agents: vec![mock_listing],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            let requests = requests.clone();
            let accounts = accounts.clone();
            let external = settings("EXTERNAL_TOKEN", None);
            client.update(cx, |client, cx| {
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    match request {
                        Request::OpenLoginSession { account, .. } => {
                            Some(Response::LoginSessionOpened(account.map_or(100, |id| id.0)))
                        }
                        Request::SubscribeThread(ConnectionId::LoginSession(id)) => {
                            Some(Response::Thread(login_session(*id != 2)))
                        }
                        Request::ListAgentSessions { .. } => {
                            Some(Response::AgentSessions(AgentSessions::Listed(Vec::new())))
                        }
                        Request::CopyAccountSettings { .. }
                        | Request::UpdateAgentSettings { .. } => Some(Response::Ok),
                        _ => None,
                    }
                });
                client.set_accounts_for_test([(mock.clone(), accounts)].into(), cx);
                client.set_agent_settings_for_test([(mock.clone(), external)].into(), cx);
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        let (page, cx) = open_page_with_dialogs(cx);
        page.update_in(cx, |page, window, cx| page.show_agents(window, cx));
        cx.run_until_parked();
        let click = |selector: &'static str, cx: &mut gpui::VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} is shown"));
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        let sent = |request: &Request| requests.borrow().contains(request);
        let count = |request: &Request| {
            requests
                .borrow()
                .iter()
                .filter(|sent| *sent == request)
                .count()
        };
        let set_model = |account: Option<AccountId>, model: &str| Request::UpdateAgentSettings {
            agent_id: mock.clone(),
            account,
            change: AgentSettingsChange::SetDefaultConfigOption {
                config_id: "model".into(),
                value: Some(acp::SessionConfigOptionValue::value_id(model.to_string())),
            },
        };
        let list_sessions = |account: Option<AccountId>| Request::ListAgentSessions {
            agent_id: mock.clone(),
            account,
        };
        click("agent-row-mock", cx);

        // Only the account the agent hasn't logged in yet offers Copy settings from, in its
        // dialog: the default account first, then the others, then Nothing.
        assert!(cx.debug_bounds("account-picker").is_none());
        click("account-line-1", cx);
        assert!(cx.debug_bounds("copy-settings-1").is_none());
        click("account-dialog-close", cx);
        click("account-line-2", cx);
        click("copy-settings-2", cx);
        let from_work = cx
            .debug_bounds("copy-settings-from-1")
            .expect("Work is offered");
        let from_external = cx
            .debug_bounds("copy-settings-from-external")
            .expect("the External account is offered");
        let from_nothing = cx
            .debug_bounds("copy-settings-from-nothing")
            .expect("Nothing is offered");
        assert!(from_work.top() < from_external.top() && from_external.top() < from_nothing.top());
        assert!(cx.debug_bounds("copy-settings-from-2").is_none());
        let opened = Request::OpenLoginSession {
            agent_id: mock.clone(),
            account: Some(AccountId(2)),
        };
        assert_eq!(count(&opened), 1);
        click("copy-settings-from-external", cx);
        assert!(sent(&Request::CopyAccountSettings {
            agent_id: mock.clone(),
            account: AccountId(2),
            from: SettingsSource::External,
        }));
        // It starts again with the variables copied.
        assert_eq!(count(&opened), 2);
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(cx.debug_bounds("account-dialog").is_none());

        // Defaults are the account for new threads' at first, then the picked one's.
        click("agent-tab-defaults", cx);
        click("agent-default-model", cx);
        click("MENU_ITEM-Sonnet", cx);
        assert!(sent(&set_model(Some(AccountId(1)), "sonnet")));
        click("account-picker", cx);
        click("account-picker-external", cx);
        click("agent-default-model", cx);
        click("MENU_ITEM-Haiku", cx);
        assert!(sent(&set_model(None, "haiku")));

        // The Environment tab shows and saves the picked account's variables.
        click("agent-tab-environment", cx);
        let env = |cx: &mut gpui::VisualTestContext| {
            page.read_with(cx, |page, cx| {
                let panel = page.agent_panel().expect("the agent is open");
                let keys: Vec<String> = panel
                    .env_rows
                    .iter()
                    .map(|row| row.key.read(cx).text().to_string())
                    .collect();
                (panel.env_account, keys)
            })
        };
        assert_eq!(env(cx), (None, vec!["EXTERNAL_TOKEN".to_string()]));
        click("account-picker", cx);
        click("account-picker-1", cx);
        assert_eq!(
            env(cx),
            (Some(AccountId(1)), vec!["WORK_TOKEN".to_string()])
        );
        page.update(cx, |page, cx| {
            if let Some(panel) = page.agent_panel_mut() {
                panel.env_rows.clear();
            }
            page.save_env(cx);
        });
        assert!(sent(&Request::UpdateAgentSettings {
            agent_id: mock.clone(),
            account: Some(AccountId(1)),
            change: AgentSettingsChange::SetEnv(BTreeMap::new()),
        }));

        // The Threads tab lists the picked account's sessions, and lists again for another.
        click("agent-tab-threads", cx);
        assert!(sent(&list_sessions(Some(AccountId(1)))));
        assert!(!sent(&list_sessions(None)));
        click("account-picker", cx);
        click("account-picker-external", cx);
        assert!(sent(&list_sessions(None)));

        // An account that's no longer listed gives way to the account for new threads.
        accounts.external_logged_in = Some(false);
        client.update(cx, |client, cx| {
            client.set_accounts_for_test([(mock.clone(), accounts.clone())].into(), cx)
        });
        cx.run_until_parked();
        assert_eq!(count(&list_sessions(Some(AccountId(1)))), 2);

        // With one account, there's nothing to pick.
        accounts
            .accounts
            .retain(|account| account.id == AccountId(1));
        client.update(cx, |client, cx| {
            client.set_accounts_for_test([(mock.clone(), accounts)].into(), cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("account-picker").is_none());

        // A new thread's Add Account… goes to the Account tab of the page already open, and
        // adds an account there.
        let external_session = Request::OpenLoginSession {
            agent_id: mock.clone(),
            account: None,
        };
        let external_sessions = count(&external_session);
        page.update_in(cx, |page, window, cx| {
            page.show_agent_accounts(MachineId::Local, &mock, true, window, cx)
        });
        cx.run_until_parked();
        assert!(page.read_with(cx, |page, _| {
            page.agent_panel()
                .is_some_and(|panel| panel.tab == AgentTab::Account)
        }));
        assert!(sent(&Request::AddAccount(mock.clone())));
        assert_eq!(count(&external_session), external_sessions);
    }

    fn checkout(id: u64, path: &str) -> Project {
        Project {
            id: ProjectId(id),
            path: path.into(),
            custom_name: None,
            icon: None,
            workspaces: Vec::new(),
            repository: Some(projects::RepositoryIdentity {
                canonical_key: "github.com/agentz/agentz".into(),
                root_path: path.into(),
                remote_name: "origin".into(),
                remote_url: "https://github.com/agentz/agentz.git".into(),
                display_name: Some("agentz/agentz".into()),
                owner: Some("agentz".into()),
                name: Some("agentz".into()),
            }),
        }
    }

    fn folder(id: u64, path: &str) -> Project {
        Project {
            repository: None,
            ..checkout(id, path)
        }
    }

    /// This Mac with two checkouts of one repository and a folder, and Devbox 1 with a third
    /// checkout and a folder of its own.
    fn machines_with_copies(
        cx: &mut TestAppContext,
    ) -> (Entity<ServerClient>, Entity<ServerClient>) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let local = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let devbox = ServerClient::new_for_test(
                MachineId::Remote(1),
                "Devbox 1".into(),
                SpacesSnapshot::default(),
                cx,
            );
            for (client, projects) in [
                (
                    &local,
                    vec![
                        checkout(1, "/work/agentz"),
                        checkout(2, "/work/agentz-2"),
                        folder(3, "/work/notes"),
                    ],
                ),
                (
                    &devbox,
                    vec![
                        checkout(1, "/home/me/agentz"),
                        folder(2, "/home/me/scratch"),
                    ],
                ),
            ] {
                client.read(cx).projects().clone().update(cx, |store, cx| {
                    store.set_snapshot(
                        ProjectsSnapshot {
                            projects,
                            ..Default::default()
                        },
                        cx,
                    )
                });
            }
            crate::machines::init_for_test(vec![local.clone(), devbox.clone()], cx);
            (local, devbox)
        })
    }

    fn shown_section(page: &Entity<SettingsPage>, cx: &mut gpui::VisualTestContext) -> Section {
        page.read_with(cx, |page, _| page.section)
    }

    #[gpui::test]
    fn a_combined_project_is_one_row_whose_page_picks_the_copy(cx: &mut TestAppContext) {
        machines_with_copies(cx);
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        cx.run_until_parked();
        let this_mac = ProjectKey {
            machine: MachineId::Local,
            project: ProjectId(1),
        };
        let devbox = ProjectKey {
            machine: MachineId::Remote(1),
            project: ProjectId(1),
        };

        // The three checkouts are one row, which opens This Mac's first. The folders keep
        // their own rows.
        let row = cx
            .debug_bounds("settings-nav-project-local-1")
            .expect("the combined project is listed");
        for copy in [
            "settings-nav-project-local-2",
            "settings-nav-project-remote-1-1",
        ] {
            assert!(cx.debug_bounds(copy).is_none(), "{copy} has no row");
        }
        assert!(cx.debug_bounds("settings-nav-project-local-3").is_some());
        assert!(cx.debug_bounds("settings-nav-project-remote-1-2").is_some());
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(shown_section(&page, cx), Section::Project(this_mac));
        assert!(cx.debug_bounds("project-copy-local-1").is_some());

        // The machine picker's menu lists each copy, and tells the two on This Mac apart by
        // folder.
        let (labels, kept) = page.update(cx, |page, cx| {
            let copies = page.group_members(this_mac, cx);
            let labels: Vec<SharedString> = copies
                .iter()
                .map(|(copy, _)| copy_label(*copy, &copies, cx))
                .collect();
            let kept = [this_mac, devbox].map(|copy| kept_copies(copy, &copies, cx));
            (labels, kept)
        });
        assert_eq!(
            labels,
            [
                "This Mac · /work/agentz",
                "This Mac · /work/agentz-2",
                "Devbox 1"
            ]
        );
        assert_eq!(
            kept,
            ["The other copies stay.", "This Mac keeps its copies."]
        );
        let picker = cx
            .debug_bounds("project-copy-picker")
            .expect("a combined project has a machine picker");
        cx.simulate_click(picker.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        for option in ["project-copy-option-local-1", "project-copy-option-local-2"] {
            assert!(cx.debug_bounds(option).is_some(), "{option} is listed");
        }
        let devbox_option = cx
            .debug_bounds("project-copy-option-remote-1-1")
            .expect("Devbox 1's copy is listed");
        cx.simulate_click(devbox_option.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(shown_section(&page, cx), Section::Project(devbox));
        assert!(cx.debug_bounds("project-copy-remote-1-1").is_some());
        assert!(cx.debug_bounds("project-copy-local-1").is_none());
        // Still one row, standing for the copy shown.
        assert!(cx.debug_bounds("settings-nav-project-remote-1-1").is_some());
        assert!(cx.debug_bounds("settings-nav-project-local-1").is_none());

        // A thread's Project Settings opens on its copy.
        page.update_in(cx, |page, window, cx| {
            page.select(Section::General, window, cx);
            page.show_project(devbox, window, cx);
        });
        cx.run_until_parked();
        assert_eq!(shown_section(&page, cx), Section::Project(devbox));

        // A project with one copy has nothing to pick.
        let notes = cx
            .debug_bounds("settings-nav-project-local-3")
            .expect("the folder is listed");
        cx.simulate_click(notes.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("project-copy-local-3").is_some());
        assert!(cx.debug_bounds("project-copy-picker").is_none());
    }

    #[gpui::test]
    fn removing_a_copy_leaves_the_others_and_shows_the_next(cx: &mut TestAppContext) {
        let (local, devbox) = machines_with_copies(cx);
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        let on_devbox = ProjectKey {
            machine: MachineId::Remote(1),
            project: ProjectId(1),
        };
        page.update_in(cx, |page, window, cx| {
            page.show_project(on_devbox, window, cx)
        });
        cx.run_until_parked();

        let remove = cx
            .debug_bounds("remove-project")
            .expect("the copy can be removed");
        cx.simulate_click(remove.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.has_pending_prompt());
        cx.simulate_prompt_answer("Remove");
        cx.run_until_parked();
        let removes = |client: &Entity<ServerClient>, cx: &mut gpui::VisualTestContext| {
            client.read_with(cx, |client, _| {
                client
                    .sent_for_test()
                    .into_iter()
                    .filter(|request| matches!(request, Request::RemoveProject(_)))
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(
            removes(&devbox, cx),
            vec![Request::RemoveProject(ProjectId(1))]
        );
        assert!(removes(&local, cx).is_empty());

        // Once Devbox 1's server has removed it, the page shows the next copy: the one before
        // it, the last being gone.
        devbox.update(cx, |client, cx| {
            client.projects().clone().update(cx, |store, cx| {
                store.set_snapshot(
                    ProjectsSnapshot {
                        projects: vec![folder(2, "/home/me/scratch")],
                        ..Default::default()
                    },
                    cx,
                )
            })
        });
        cx.run_until_parked();
        assert_eq!(
            shown_section(&page, cx),
            Section::Project(ProjectKey {
                machine: MachineId::Local,
                project: ProjectId(2),
            })
        );
        assert!(cx.debug_bounds("project-copy-local-2").is_some());
    }

    /// The icon chosen is the whole project's, a file inside it too; a file picked outside it
    /// is on This Mac, so only This Mac's copies take it.
    #[gpui::test]
    fn a_project_icon_is_every_copy_s(cx: &mut TestAppContext) {
        let (local, devbox) = machines_with_copies(cx);
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        let this_mac = ProjectKey {
            machine: MachineId::Local,
            project: ProjectId(1),
        };
        page.update_in(cx, |page, window, cx| {
            page.show_project(this_mac, window, cx)
        });
        cx.run_until_parked();
        let opened = Rc::new(RefCell::new(Vec::new()));
        cx.update(|_, cx| {
            let opened = opened.clone();
            cx.subscribe(&page, move |_, event: &SettingsPageEvent, _| match event {
                SettingsPageEvent::ChooseIcon(_) => opened.borrow_mut().push("icon"),
                SettingsPageEvent::ChooseIconFile(_) => opened.borrow_mut().push("file"),
                _ => {}
            })
            .detach()
        });
        for button in ["choose-icon", "choose-icon-file"] {
            let bounds = cx.debug_bounds(button).expect("the icon can be chosen");
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        }
        assert_eq!(*opened.borrow(), ["icon", "file"]);

        let icons = |client: &Entity<ServerClient>, cx: &mut gpui::VisualTestContext| {
            client.read_with(cx, |client, _| {
                client
                    .sent_for_test()
                    .into_iter()
                    .filter_map(|request| match request {
                        Request::SetProjectIcon { project_id, icon } => Some((project_id, icon)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            })
        };
        let rocket = Some(ProjectIcon::Emoji {
            emoji: "🚀".into()
        });
        let inside = Some(ProjectIcon::Image {
            path: "public/logo.png".into(),
        });
        let outside = Some(ProjectIcon::Image {
            path: "/Users/me/logo.png".into(),
        });
        for icon in [&rocket, &inside, &outside] {
            page.update(cx, |page, cx| {
                page.set_group_icon(this_mac, icon.clone(), cx)
            });
        }
        assert_eq!(
            icons(&local, cx),
            [
                (ProjectId(1), rocket.clone()),
                (ProjectId(2), rocket.clone()),
                (ProjectId(1), inside.clone()),
                (ProjectId(2), inside.clone()),
                (ProjectId(1), outside.clone()),
                (ProjectId(2), outside),
            ]
        );
        assert_eq!(
            icons(&devbox, cx),
            [(ProjectId(1), rocket), (ProjectId(1), inside)]
        );
    }

    #[test]
    fn the_next_copy_follows_the_removed_one() {
        let this_mac = |id| ProjectKey {
            machine: MachineId::Local,
            project: ProjectId(id),
        };
        let copies = [this_mac(1), this_mac(2), this_mac(3)];
        assert_eq!(next_copy(&copies, this_mac(2), |_| true), Some(this_mac(3)));
        assert_eq!(next_copy(&copies, this_mac(3), |_| true), Some(this_mac(2)));
        assert_eq!(
            next_copy(&copies, this_mac(1), |copy| copy == this_mac(3)),
            Some(this_mac(3))
        );
        assert_eq!(next_copy(&copies[..1], this_mac(1), |_| true), None);
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
        assert_eq!(
            login_method_subject("Use an API key").as_deref(),
            Some("an API key")
        );
        assert_eq!(login_method_subject("Log In"), None);
        assert_eq!(login_method_subject("Login"), None);
    }
}
