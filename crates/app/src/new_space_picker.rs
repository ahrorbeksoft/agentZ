//! The Workspaces sidebar's **+** popover: where a new workspace is rooted. Each machine's
//! home folder, and each project's checkout, worktrees and pastures, listed under their
//! machine like the project switcher's projects.

use std::path::PathBuf;
use std::rc::Rc;

use agentz_protocol::CAPABILITY_SPACES;
use agentz_protocol::spaces::{Space, SpaceId};
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Window,
};
use projects::ProjectId;
use text_input::{TextInput, TextInputEvent};
use ui::{
    Divider, HighlightedLabel, ListItem, ListItemSpacing, ListSubHeader, WithScrollbar as _,
    prelude::*,
};

use crate::machines::{MachineId, Machines};
use crate::project_info::{ProjectInfoStore, render_project_icon, workspace_icon};
use crate::project_switcher::{compact_path, fuzzy_match};
use crate::spaces_view::SpaceKey;

const KEY_CONTEXT: &str = "NewSpacePicker";
/// How many recently used workspaces the picker lists first.
const MAX_RECENT: usize = 5;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

/// Where the new workspace goes, even if one is open there already.
#[derive(Clone, Debug, PartialEq)]
pub struct SpaceChoice {
    pub machine: MachineId,
    pub folder: PathBuf,
    pub project_id: Option<ProjectId>,
}

#[derive(Clone)]
enum EntryKind {
    Home,
    Checkout,
    Workspace(projects::WorkspaceKind),
}

#[derive(Clone)]
struct Entry {
    choice: SpaceChoice,
    kind: EntryKind,
    label: SharedString,
    detail: SharedString,
    section: SharedString,
    positions: Vec<usize>,
}

pub struct NewSpacePicker {
    machines: Entity<Machines>,
    /// Open workspaces by when they were last used, most recent first.
    recent: Vec<(MachineId, SpaceId)>,
    on_choose: Rc<dyn Fn(SpaceChoice, &mut Window, &mut App)>,
    search: Entity<TextInput>,
    entries: Vec<Entry>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for NewSpacePicker {}

impl Focusable for NewSpacePicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl NewSpacePicker {
    pub fn new(
        recent: Vec<SpaceKey>,
        on_choose: impl Fn(SpaceChoice, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let machines = Machines::global(cx);
        let search = cx.new(|cx| TextInput::new("Search folders…", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.selected_index = 0;
                this.update_entries(cx)
            }),
            cx.observe(&machines, |this, _, cx| this.update_entries(cx)),
        ];
        window.focus(&search.focus_handle(cx), cx);
        let mut this = Self {
            machines,
            recent: recent
                .into_iter()
                .map(|key| (key.machine, key.space))
                .collect(),
            on_choose: Rc::new(on_choose),
            search,
            entries: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        this.update_entries(cx);
        this
    }

    fn update_entries(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let machines = self.machines.read(cx);
        let has_remotes = machines.has_remotes();
        let mut entries = Vec::new();
        // The open workspaces, whose folders are offered first.
        let open: Vec<(MachineId, Space)> = machines
            .clients()
            .iter()
            .flat_map(|client| {
                let client = client.read(cx);
                client
                    .spaces()
                    .spaces
                    .iter()
                    .map(|space| (client.machine(), space.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        // Recently used workspaces first.
        for (machine, space_id) in self.recent.iter().take(MAX_RECENT) {
            let Some((_, space)) = open
                .iter()
                .find(|(space_machine, space)| space_machine == machine && space.id == *space_id)
            else {
                continue;
            };
            let detail: SharedString = space
                .current
                .as_ref()
                .map(|current| current.display_path.clone())
                .unwrap_or_else(|| space.folder.display().to_string())
                .into();
            let mut entry = Entry {
                choice: SpaceChoice {
                    machine: *machine,
                    folder: space.current_folder().to_path_buf(),
                    project_id: space.project_id,
                },
                // A project's workspace shows the project's icon.
                kind: if space.project_id.is_some() {
                    EntryKind::Checkout
                } else {
                    EntryKind::Home
                },
                label: space.label().into(),
                detail,
                section: "Recent".into(),
                positions: Vec::new(),
            };
            if let Some(positions) = fuzzy_match(&query, &entry.label)
                .or_else(|| entry.detail.to_lowercase().contains(&query).then(Vec::new))
            {
                entry.positions = positions;
                entries.push(entry);
            }
        }
        for client in machines.clients() {
            let client = client.read(cx);
            if !client.has_capability(CAPABILITY_SPACES) {
                continue;
            }
            let machine = client.machine();
            let section: SharedString = if has_remotes {
                client.label().clone()
            } else {
                crate::machines::LOCAL_MACHINE_NAME.into()
            };
            let mut candidates = vec![Entry {
                choice: SpaceChoice {
                    machine,
                    folder: "~".into(),
                    project_id: None,
                },
                kind: EntryKind::Home,
                label: "Home Folder".into(),
                detail: "~".into(),
                section: section.clone(),
                positions: Vec::new(),
            }];
            let describe = |path: &std::path::Path| -> SharedString {
                match machine {
                    MachineId::Local => compact_path(path).into(),
                    MachineId::Remote(_) => path.display().to_string().into(),
                }
            };
            for project in client.projects().read(cx).projects() {
                candidates.push(Entry {
                    choice: SpaceChoice {
                        machine,
                        folder: project.path.clone(),
                        project_id: Some(project.id),
                    },
                    kind: EntryKind::Checkout,
                    label: project.name(),
                    detail: describe(&project.path),
                    section: section.clone(),
                    positions: Vec::new(),
                });
                for workspace in &project.workspaces {
                    let name = workspace.branch.clone().unwrap_or_else(|| {
                        workspace
                            .path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    });
                    candidates.push(Entry {
                        choice: SpaceChoice {
                            machine,
                            folder: workspace.path.clone(),
                            project_id: Some(project.id),
                        },
                        kind: EntryKind::Workspace(workspace.kind),
                        label: format!("{} › {name}", project.name()).into(),
                        detail: format!(
                            "{} · {}",
                            workspace.kind.label(),
                            describe(&workspace.path)
                        )
                        .into(),
                        section: section.clone(),
                        positions: Vec::new(),
                    });
                }
            }
            for mut entry in candidates {
                let positions = fuzzy_match(&query, &entry.label)
                    .or_else(|| entry.detail.to_lowercase().contains(&query).then(Vec::new));
                if let Some(positions) = positions {
                    entry.positions = positions;
                    entries.push(entry);
                }
            }
        }
        self.entries = entries;
        self.selected_index = self
            .selected_index
            .min(self.entries.len().saturating_sub(1));
        cx.notify();
    }

    fn starts_section(&self, index: usize) -> bool {
        index == 0
            || self.entries.get(index - 1).map(|entry| &entry.section)
                != self.entries.get(index).map(|entry| &entry.section)
    }

    fn scroll_to_selection(&self) {
        let headings = (0..=self.selected_index)
            .filter(|index| self.starts_section(*index))
            .count();
        self.scroll_handle
            .scroll_to_item(self.selected_index + headings);
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if !self.entries.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.entries.len();
            self.scroll_to_selection();
            cx.notify();
        }
    }

    fn select_previous(
        &mut self,
        _: &menu::SelectPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.entries.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.entries.len() - 1);
            self.scroll_to_selection();
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.entries.get(self.selected_index) {
            let choice = entry.choice.clone();
            self.choose(choice, window, cx);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn choose(&mut self, choice: SpaceChoice, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
        let on_choose = self.on_choose.clone();
        // After the popover has closed and given focus back, so the new pane can take it.
        window.defer(cx, move |window, cx| on_choose(choice, window, cx));
    }

    fn render_entry(&self, index: usize, entry: Entry, cx: &mut Context<Self>) -> AnyElement {
        let machines = self.machines.read(cx);
        let is_offline = !machines.is_online(entry.choice.machine, cx);
        let icon = match &entry.kind {
            EntryKind::Home => Icon::new(IconName::Folder)
                .size(IconSize::Small)
                .color(Color::Muted)
                .into_any_element(),
            EntryKind::Checkout => {
                let project = entry.choice.project_id.and_then(|project_id| {
                    machines
                        .projects(entry.choice.machine, cx)?
                        .read(cx)
                        .project(project_id)
                        .cloned()
                });
                match project {
                    Some(project) => {
                        let info = ProjectInfoStore::global(cx)
                            .read(cx)
                            .info(entry.choice.machine, project.id);
                        render_project_icon(&project, info, px(16.), cx)
                    }
                    None => Icon::new(IconName::Folder)
                        .size(IconSize::Small)
                        .into_any_element(),
                }
            }
            EntryKind::Workspace(kind) => Icon::new(workspace_icon(*kind))
                .size(IconSize::Small)
                .color(Color::Muted)
                .into_any_element(),
        };
        let choice = entry.choice.clone();
        ListItem::new(("new-space-entry", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .disabled(is_offline)
            .start_slot(icon)
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(HighlightedLabel::new(entry.label, entry.positions))
                    .child(
                        Label::new(entry.detail)
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    ),
            )
            .on_click(
                cx.listener(move |this, _, window, cx| this.choose(choice.clone(), window, cx)),
            )
            .into_any_element()
    }
}

impl Render for NewSpacePicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let mut rows = Vec::with_capacity(self.entries.len() * 2);
        for (index, entry) in self.entries.clone().into_iter().enumerate() {
            if self.starts_section(index) {
                rows.push(
                    v_flex()
                        .w_full()
                        .gap_1()
                        .when(index > 0, |this| this.mt_1().child(Divider::horizontal()))
                        .child(ListSubHeader::new(entry.section.clone()).inset(true))
                        .into_any_element(),
                );
            }
            rows.push(self.render_entry(index, entry, cx));
        }

        v_flex()
            .key_context(KEY_CONTEXT)
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            .w(rems(26.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(border_variant)
                    .child(
                        Icon::new(IconName::MagnifyingGlass)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(self.search.clone()),
            )
            .child(
                div()
                    .id("new-space-scroll")
                    .child(
                        v_flex()
                            .id("new-space-entries")
                            .max_h(rems(24.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(rows)
                            .when(self.entries.is_empty(), |list| {
                                list.child(
                                    div().px_2().py_1p5().child(
                                        Label::new("No matching folders").color(Color::Muted),
                                    ),
                                )
                            }),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
            .child(
                h_flex()
                    .px_3()
                    .py_1p5()
                    .gap_3()
                    .border_t_1()
                    .border_color(border_variant)
                    .child(
                        Label::new("↩ Open")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new(if cfg!(target_os = "macos") {
                            "⌘↩ Open Another"
                        } else {
                            "Ctrl-↩ Open Another"
                        })
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    ),
            )
    }
}
