//! Adding a project when there are other machines (t3code's add-project palette): pick the
//! machine, then type the folder's path there with its folders suggested as you go. This Mac's
//! folders come from the system's folder picker instead.

use std::path::PathBuf;
use std::time::Duration;

use agentz_protocol::{CAPABILITY_BROWSE_DIRECTORIES, DirectoryEntry};
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Task, Window, actions,
};
use text_input::{TextInput, TextInputEvent};
use ui::{ListItem, ListItemSpacing, WithScrollbar as _, prelude::*};

use crate::machines::{MachineId, Machines, ProjectKey};
use crate::server_client::ServerClient;

const KEY_CONTEXT: &str = "AddProjectModal";
/// Typing fast shouldn't send the machine a listing request per keystroke.
const BROWSE_DEBOUNCE: Duration = Duration::from_millis(80);

actions!(
    add_project,
    [
        /// Completes the path to the selected folder, as a shell's Tab does.
        CompleteFolder,
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
        KeyBinding::new("tab", CompleteFolder, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Machine,
    Path(MachineId),
}

#[derive(Clone)]
enum PathRow {
    /// Adds the typed path.
    Add(String),
    Folder(DirectoryEntry),
}

pub enum AddProjectModalEvent {
    ProjectAdded(ProjectKey),
    /// This Mac was picked; its folders come from the system's picker.
    ChooseLocalFolder,
}

pub struct AddProjectModal {
    machines: Entity<Machines>,
    input: Entity<TextInput>,
    step: Step,
    /// Whether the machine was left to pick here, so the path step can go back to it.
    picks_machine: bool,
    machine_rows: Vec<MachineId>,
    path_rows: Vec<PathRow>,
    /// Folders the machine suggested for the typed path, or why it couldn't.
    folders: Result<Vec<DirectoryEntry>, SharedString>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    adding: bool,
    error: Option<SharedString>,
    _browse: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for AddProjectModal {}
impl EventEmitter<AddProjectModalEvent> for AddProjectModal {}

impl Focusable for AddProjectModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl AddProjectModal {
    /// With no `machine`, the user picks the machine first.
    pub fn new(machine: Option<MachineId>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let machines = Machines::global(cx);
        let input = cx.new(|cx| TextInput::new("", cx));
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &TextInputEvent, cx| {
                this.input_changed(cx)
            }),
            cx.observe(&machines, |this, _, cx| this.update_rows(cx)),
        ];
        window.focus(&input.focus_handle(cx), cx);
        let mut this = Self {
            machines,
            input,
            step: Step::Machine,
            picks_machine: machine.is_none(),
            machine_rows: Vec::new(),
            path_rows: Vec::new(),
            folders: Ok(Vec::new()),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            adding: false,
            error: None,
            _browse: Task::ready(()),
            _subscriptions: subscriptions,
        };
        match machine {
            Some(machine) => this.go_to(Step::Path(machine), cx),
            None => this.go_to(Step::Machine, cx),
        }
        this
    }

    fn client(&self, machine: MachineId, cx: &App) -> Option<Entity<ServerClient>> {
        self.machines.read(cx).client(machine, cx)
    }

    fn go_to(&mut self, step: Step, cx: &mut Context<Self>) {
        self.step = step;
        self.selected_index = 0;
        self.error = None;
        self.folders = Ok(Vec::new());
        self.scroll_handle.set_offset(gpui::point(px(0.), px(0.)));
        let (placeholder, text) = match step {
            Step::Machine => ("Search machines…", ""),
            Step::Path(_) => ("Path to the project's folder", "~/"),
        };
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, cx);
            input.set_text(text, cx);
        });
    }

    fn input_changed(&mut self, cx: &mut Context<Self>) {
        self.selected_index = 0;
        self.error = None;
        if let Step::Path(machine) = self.step {
            self.browse(machine, cx);
        }
        self.update_rows(cx);
    }

    fn browse(&mut self, machine: MachineId, cx: &mut Context<Self>) {
        let Some(client) = self.client(machine, cx) else {
            return;
        };
        if !client.read(cx).is_online() {
            self.folders = Err(format!("{} is offline", client.read(cx).label()).into());
            return;
        }
        if !client
            .read(cx)
            .has_capability(CAPABILITY_BROWSE_DIRECTORIES)
        {
            self.folders = Err("This machine's agentz-server can't suggest folders".into());
            return;
        }
        let partial_path = self.input.read(cx).text().to_string();
        self._browse = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(BROWSE_DEBOUNCE).await;
            let listing =
                client.update(cx, |client, cx| client.browse_directories(partial_path, cx));
            let folders = listing
                .await
                .map(|listing| listing.entries)
                .map_err(|error| SharedString::from(format!("{error:#}")));
            this.update(cx, |this, cx| {
                this.folders = folders;
                this.update_rows(cx);
            })
            .ok();
        });
    }

    fn update_rows(&mut self, cx: &mut Context<Self>) {
        let query = self.input.read(cx).text().trim().to_string();
        match self.step {
            Step::Machine => {
                let query = query.to_lowercase();
                let machines = self.machines.read(cx);
                self.machine_rows = machines
                    .clients()
                    .iter()
                    .map(|client| client.read(cx))
                    .filter(|client| {
                        query.is_empty() || client.label().to_lowercase().contains(&query)
                    })
                    .map(|client| client.machine())
                    .collect();
            }
            Step::Path(_) => {
                let add = (!query.is_empty()).then(|| PathRow::Add(query));
                let folders = self.folders.as_ref().map(Vec::as_slice).unwrap_or(&[]);
                self.path_rows = add
                    .into_iter()
                    .chain(folders.iter().cloned().map(PathRow::Folder))
                    .collect();
            }
        }
        self.selected_index = self.selected_index.min(self.row_count().saturating_sub(1));
        cx.notify();
    }

    fn row_count(&self) -> usize {
        match self.step {
            Step::Machine => self.machine_rows.len(),
            Step::Path(_) => self.path_rows.len(),
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
            Step::Machine => {
                if let Some(machine) = self.machine_rows.get(self.selected_index).copied() {
                    self.choose_machine(machine, cx);
                }
            }
            Step::Path(machine) => {
                if let Some(row) = self.path_rows.get(self.selected_index).cloned() {
                    self.choose_path_row(machine, row, cx);
                }
            }
        }
    }

    /// Goes into the selected folder, or the first one when the typed path is selected.
    fn complete_folder(&mut self, _: &CompleteFolder, _: &mut Window, cx: &mut Context<Self>) {
        let folder = self
            .path_rows
            .get(self.selected_index)
            .and_then(|row| match row {
                PathRow::Folder(folder) => Some(folder),
                PathRow::Add(_) => None,
            })
            .or_else(|| {
                self.path_rows.iter().find_map(|row| match row {
                    PathRow::Folder(folder) => Some(folder),
                    PathRow::Add(_) => None,
                })
            })
            .cloned();
        if let Some(folder) = folder {
            self.enter_folder(&folder, cx);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.adding {
            return;
        }
        match self.step {
            Step::Path(_) if self.picks_machine => self.go_to(Step::Machine, cx),
            _ => cx.emit(DismissEvent),
        }
    }

    fn choose_machine(&mut self, machine: MachineId, cx: &mut Context<Self>) {
        match machine {
            MachineId::Local => cx.emit(AddProjectModalEvent::ChooseLocalFolder),
            MachineId::Remote(_) => {
                if self.machines.read(cx).is_online(machine, cx) {
                    self.go_to(Step::Path(machine), cx);
                }
            }
        }
    }

    fn choose_path_row(&mut self, machine: MachineId, row: PathRow, cx: &mut Context<Self>) {
        match row {
            PathRow::Add(path) => self.add_project(machine, path, cx),
            PathRow::Folder(folder) => self.enter_folder(&folder, cx),
        }
    }

    /// Replaces the typed path's last part with the folder, keeping a leading `~` as typed.
    fn enter_folder(&mut self, folder: &DirectoryEntry, cx: &mut Context<Self>) {
        let text = self.input.read(cx).text().to_string();
        let path = completed_path(&text, folder);
        self.input.update(cx, |input, cx| input.set_text(path, cx));
    }

    fn add_project(&mut self, machine: MachineId, path: String, cx: &mut Context<Self>) {
        if self.adding {
            return;
        }
        let Some(client) = self.client(machine, cx) else {
            return;
        };
        if !client.read(cx).is_online() {
            self.error = Some(format!("{} is offline", client.read(cx).label()).into());
            cx.notify();
            return;
        }
        let store = client.read(cx).projects().clone();
        let added = store.update(cx, |store, cx| store.add_project(PathBuf::from(path), cx));
        self.adding = true;
        self.error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let added = added.await;
            this.update(cx, |this, cx| {
                this.adding = false;
                match added {
                    Ok(project) => cx.emit(AddProjectModalEvent::ProjectAdded(ProjectKey {
                        machine,
                        project,
                    })),
                    Err(error) => {
                        this.error = Some(format!("Couldn't add the project: {error:#}").into());
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn render_machine_row(
        &self,
        index: usize,
        machine: MachineId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(client) = self.client(machine, cx) else {
            return div().into_any_element();
        };
        let client = client.read(cx);
        let is_offline = !client.is_online();
        let detail: SharedString = match (machine, is_offline) {
            (MachineId::Local, _) => "Choose a folder…".into(),
            (MachineId::Remote(_), true) => "Offline".into(),
            (MachineId::Remote(_), false) => client
                .connection()
                .map(|connection| connection.welcome().machine.hostname.clone())
                .unwrap_or_default()
                .into(),
        };
        let is_disabled = is_offline && machine != MachineId::Local;
        ListItem::new(("add-project-machine", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .disabled(is_disabled)
            .start_slot(
                Icon::new(match machine {
                    MachineId::Local => IconName::Screen,
                    MachineId::Remote(_) => IconName::Server,
                })
                .size(IconSize::Small)
                .color(Color::Muted),
            )
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div().flex_none().child(
                            Label::new(client.label().clone())
                                .when(is_disabled, |label| label.color(Color::Disabled)),
                        ),
                    )
                    .child(
                        div().min_w_0().child(
                            Label::new(detail)
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.choose_machine(machine, cx)))
            .into_any_element()
    }

    fn render_path_row(
        &self,
        index: usize,
        machine: MachineId,
        row: PathRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (icon, label, detail): (IconName, SharedString, Option<SharedString>) = match &row {
            PathRow::Add(path) => (IconName::Plus, format!("Add {path}").into(), None),
            PathRow::Folder(folder) => (
                IconName::Folder,
                folder.name.clone().into(),
                Some(folder.path.display().to_string().into()),
            ),
        };
        ListItem::new(("add-project-path", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .start_slot(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().min_w_0().child(Label::new(label).truncate()))
                    .children(detail.map(|detail| {
                        div().min_w_0().child(
                            Label::new(detail)
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        )
                    })),
            )
            .on_click(
                cx.listener(move |this, _, _, cx| this.choose_path_row(machine, row.clone(), cx)),
            )
            .into_any_element()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let title: AnyElement = match self.step {
            Step::Machine => Label::new("Add a project on…")
                .color(Color::Muted)
                .into_any_element(),
            Step::Path(machine) => h_flex()
                .gap_1p5()
                .when(self.picks_machine, |row| {
                    row.child(
                        IconButton::new("add-project-back", IconName::ArrowLeft)
                            .icon_size(IconSize::Small)
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.adding {
                                    this.go_to(Step::Machine, cx)
                                }
                            })),
                    )
                })
                .child(
                    Icon::new(match machine {
                        MachineId::Local => IconName::Screen,
                        MachineId::Remote(_) => IconName::Server,
                    })
                    .size(IconSize::Small)
                    .color(Color::Muted),
                )
                .child(
                    Label::new(format!(
                        "Add a project on {}",
                        self.machines.read(cx).label(machine, cx)
                    ))
                    .color(Color::Muted),
                )
                .into_any_element(),
        };
        h_flex()
            .px_3()
            .py_2p5()
            .gap_3()
            .border_b_1()
            .border_color(border_variant)
            .child(div().flex_none().child(title))
            .child(div().flex_1().min_w_0().child(self.input.clone()))
    }
}

/// The typed path with its last part replaced by `folder`, ending in `/` so the folder's own
/// folders are suggested next.
fn completed_path(typed: &str, folder: &DirectoryEntry) -> String {
    let base = match typed.rfind('/') {
        Some(index) => &typed[..=index],
        None if typed == "~" => "~/",
        None => return format!("{}/", folder.path.display()),
    };
    format!("{base}{}/", folder.name)
}

impl Render for AddProjectModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let rows: Vec<AnyElement> = match self.step {
            Step::Machine => self
                .machine_rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, machine)| self.render_machine_row(index, machine, cx))
                .collect(),
            Step::Path(machine) => self
                .path_rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, row)| self.render_path_row(index, machine, row, cx))
                .collect(),
        };
        let note: Option<SharedString> = match (&self.step, &self.folders) {
            (Step::Machine, _) => rows.is_empty().then(|| "No matching machines".into()),
            (Step::Path(_), Err(error)) => Some(error.clone()),
            (Step::Path(_), Ok(folders)) => (folders.is_empty()
                && !self.input.read(cx).text().is_empty())
            .then(|| "No matching folders".into()),
        };
        let error = self.error.clone().map(|error| {
            div()
                .px_3()
                .pb_2()
                .child(Label::new(error).size(LabelSize::Small).color(Color::Error))
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
            .on_action(cx.listener(Self::complete_folder))
            .child(self.render_header(cx))
            .child(
                div()
                    .id("add-project-rows-scroll")
                    .child(
                        v_flex()
                            .id("add-project-rows")
                            .max_h(rems(26.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(rows)
                            .children(note.map(|note| {
                                div().px_2().py_1p5().child(
                                    Label::new(note).size(LabelSize::Small).color(Color::Muted),
                                )
                            })),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
            .children(error)
            .when(matches!(self.step, Step::Path(_)), |modal| {
                modal.child(
                    h_flex()
                        .px_3()
                        .py_1p5()
                        .gap_3()
                        .border_t_1()
                        .border_color(border_variant)
                        .child(
                            Label::new(if self.adding {
                                "Adding…"
                            } else {
                                "Tab completes a folder · Enter adds the selected path"
                            })
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                        ),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::completed_path;
    use agentz_protocol::DirectoryEntry;
    use std::path::PathBuf;

    #[test]
    fn completes_paths_as_typed() {
        let folder = DirectoryEntry {
            name: "agentZ".into(),
            path: PathBuf::from("/home/me/projects/agentZ"),
        };
        assert_eq!(
            completed_path("~/projects/ag", &folder),
            "~/projects/agentZ/"
        );
        assert_eq!(completed_path("~/projects/", &folder), "~/projects/agentZ/");
        assert_eq!(completed_path("~", &folder), "~/agentZ/");
        assert_eq!(
            completed_path("/home/me/projects/a", &folder),
            "/home/me/projects/agentZ/"
        );
        assert_eq!(completed_path("odd", &folder), "/home/me/projects/agentZ/");
    }
}
