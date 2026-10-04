//! A workspace's New Worktree and Open Worktree… (herdr's worktree overlays), for any git
//! repository the workspace is in, a project or not: a new worktree or pasture on a branch
//! named here, or one of the repository's checkouts. Either opens as a new workspace with a
//! shell there.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use agentz_protocol::workspace::{Checkout, PastureSupport, RepositoryCheckouts};
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Task, Window,
};
use projects::WorkspaceKind;
use text_input::{TextInput, TextInputEvent};
use ui::{
    CommonAnimationExt as _, ContextMenu, ListItem, ListItemSpacing, PopoverMenu,
    WithScrollbar as _, prelude::*,
};

use crate::machines::{MachineId, Machines, project_at};
use crate::project_info::{ProjectInfoStore, render_project_icon, workspace_icon};
use crate::project_store::ProjectStore;
use crate::project_switcher::compact_path;
use crate::sidebar::render_folder_icon;

const KEY_CONTEXT: &str = "WorktreeModal";
/// The server's own prefix for branches it names.
const BRANCH_PREFIX: &str = "agentz/";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorktreeModalMode {
    /// The input names the new branch; the rows pick a worktree or a pasture.
    New,
    /// The input filters the repository's checkouts.
    Open,
}

pub enum WorktreeModalEvent {
    /// Open a workspace with a shell in the folder.
    Open { machine: MachineId, folder: PathBuf },
}

pub struct WorktreeModal {
    machine: MachineId,
    /// The workspace's folder, somewhere in the repository.
    folder: PathBuf,
    /// The repository's name, for the title.
    name: SharedString,
    mode: WorktreeModalMode,
    projects: Option<Entity<ProjectStore>>,
    input: Entity<TextInput>,
    /// What the new branch starts from; what the workspace has checked out when unset.
    base: Option<String>,
    /// Loaded when the modal opens, or why it couldn't be.
    repository: Option<Result<RepositoryCheckouts, SharedString>>,
    /// What [`WorktreeModalMode::Open`] lists: the checkouts besides the workspace's own.
    rows: Vec<Checkout>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    creating: Option<SharedString>,
    error: Option<SharedString>,
    _load: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for WorktreeModal {}
impl EventEmitter<WorktreeModalEvent> for WorktreeModal {}

impl Focusable for WorktreeModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl WorktreeModal {
    pub fn new(
        machine: MachineId,
        folder: PathBuf,
        name: SharedString,
        mode: WorktreeModalMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let projects = Machines::global(cx).read(cx).projects(machine, cx);
        let input = cx.new(|cx| match mode {
            WorktreeModalMode::New => {
                let mut input = TextInput::new("Branch name", cx);
                input.set_text(generated_branch(seed()), cx);
                input.select_all_text(cx);
                input
            }
            WorktreeModalMode::Open => TextInput::new("Search worktrees…", cx),
        });
        let subscriptions = vec![
            cx.subscribe(&input, |this, _, _: &TextInputEvent, cx| {
                this.error = None;
                if this.mode == WorktreeModalMode::Open {
                    this.selected_index = 0;
                    this.update_rows(cx);
                }
                cx.notify();
            }),
            cx.observe(&ProjectInfoStore::global(cx), |_, _, cx| cx.notify()),
        ];
        window.focus(&input.focus_handle(cx), cx);
        let load = match &projects {
            Some(projects) => {
                let checkouts = projects.read(cx).repository_checkouts(folder.clone(), cx);
                cx.spawn(async move |this, cx| {
                    let checkouts = checkouts
                        .await
                        .map_err(|error| SharedString::from(format!("{error:#}")));
                    this.update(cx, |this, cx| this.set_repository(checkouts, cx))
                        .ok();
                })
            }
            None => Task::ready(()),
        };
        Self {
            machine,
            folder,
            name,
            mode,
            projects,
            input,
            base: None,
            repository: None,
            rows: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            creating: None,
            error: None,
            _load: load,
            _subscriptions: subscriptions,
        }
    }

    fn set_repository(
        &mut self,
        repository: Result<RepositoryCheckouts, SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.repository = Some(repository);
        self.update_rows(cx);
    }

    fn checkouts(&self) -> Option<&RepositoryCheckouts> {
        self.repository.as_ref()?.as_ref().ok()
    }

    fn update_rows(&mut self, cx: &mut Context<Self>) {
        if self.mode == WorktreeModalMode::Open {
            let query = self.input.read(cx).text().trim().to_lowercase();
            let checkouts = self
                .checkouts()
                .map(|repository| repository.checkouts.clone())
                .unwrap_or_default();
            let own = own_checkout(&checkouts, &self.folder).map(|checkout| checkout.path.clone());
            self.rows = checkouts
                .into_iter()
                .filter(|checkout| Some(&checkout.path) != own.as_ref())
                .filter(|checkout| {
                    let text = format!(
                        "{} {} {}",
                        checkout_kind_label(checkout.kind),
                        checkout.branch.as_deref().unwrap_or("detached"),
                        checkout.path.display()
                    );
                    query.is_empty() || text.to_lowercase().contains(&query)
                })
                .collect();
        }
        self.selected_index = self.selected_index.min(self.row_count().saturating_sub(1));
        cx.notify();
    }

    fn row_count(&self) -> usize {
        match self.mode {
            WorktreeModalMode::New => 2,
            WorktreeModalMode::Open => self.rows.len(),
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
        match self.mode {
            WorktreeModalMode::New => {
                let kind = new_row_kind(self.selected_index);
                self.create(kind, cx);
            }
            WorktreeModalMode::Open => {
                if let Some(checkout) = self.rows.get(self.selected_index).cloned() {
                    self.open(checkout, cx);
                }
            }
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.creating.is_none() {
            cx.emit(DismissEvent);
        }
    }

    /// Why a new worktree or pasture can't be made, if it can't.
    fn unavailable(&self, kind: WorkspaceKind) -> Option<String> {
        match self.repository.as_ref()? {
            Err(error) => Some(error.to_string()),
            Ok(repository) => match (kind, &repository.git.pastures) {
                (WorkspaceKind::Pasture, PastureSupport::Unsupported(reason)) => {
                    Some(reason.clone())
                }
                _ => None,
            },
        }
    }

    fn create(&mut self, kind: WorkspaceKind, cx: &mut Context<Self>) {
        if self.creating.is_some() || self.checkouts().is_none() || self.unavailable(kind).is_some()
        {
            return;
        }
        let Some(projects) = self.projects.clone() else {
            return;
        };
        let branch = self.input.read(cx).text().trim().to_string();
        if branch.is_empty() {
            self.error = Some("Name the branch".into());
            cx.notify();
            return;
        }
        let created = projects.read(cx).create_workspace(
            self.folder.clone(),
            kind,
            self.base.clone(),
            Some(branch.clone()),
            cx,
        );
        self.creating =
            Some(format!("Making the {} for {branch}…", kind.label().to_lowercase()).into());
        self.error = None;
        cx.notify();
        let machine = self.machine;
        cx.spawn(async move |this, cx| {
            let created = created.await;
            this.update(cx, |this, cx| {
                this.creating = None;
                match created {
                    Ok(folder) => cx.emit(WorktreeModalEvent::Open { machine, folder }),
                    Err(error) => {
                        this.error = Some(format!("{error:#}").into());
                        cx.notify();
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn open(&mut self, checkout: Checkout, cx: &mut Context<Self>) {
        if self.is_missing(&checkout) {
            return;
        }
        cx.emit(WorktreeModalEvent::Open {
            machine: self.machine,
            folder: checkout.path,
        });
    }

    /// Only this Mac's folders can be checked from here.
    fn is_missing(&self, checkout: &Checkout) -> bool {
        self.machine == MachineId::Local && !checkout.path.exists()
    }

    fn render_kind_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let kind = new_row_kind(index);
        let (label, detail, detail_color) = match kind {
            WorkspaceKind::Worktree => (
                "Worktree",
                "A git worktree: tracked files only".to_string(),
                Color::Muted,
            ),
            WorkspaceKind::Pasture => {
                let pastures = self.checkouts().map(|repository| &repository.git.pastures);
                let (detail, color) = match pastures {
                    Some(PastureSupport::CopyOnWrite) => (
                        "A copy-on-write clone of the folder, with dependencies and .env".into(),
                        Color::Muted,
                    ),
                    Some(PastureSupport::FullCopy) => (
                        "A full copy here: slow, and as big as the project".into(),
                        Color::Warning,
                    ),
                    Some(PastureSupport::Unsupported(reason)) => (reason.clone(), Color::Muted),
                    Some(PastureSupport::Unknown(_)) | None => {
                        ("Checking the repository…".into(), Color::Muted)
                    }
                };
                ("Pasture", detail, color)
            }
        };
        let unavailable = self.unavailable(kind);
        let disabled =
            self.checkouts().is_none() || unavailable.is_some() || self.creating.is_some();
        let (detail, detail_color) = match unavailable {
            Some(reason) => (reason, Color::Muted),
            None => (detail, detail_color),
        };
        ListItem::new(("worktree-modal-kind", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .disabled(disabled)
            .start_slot(
                Icon::new(workspace_icon(kind))
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(
                        Label::new(label).when(disabled, |label| label.color(Color::Disabled)),
                    ))
                    .child(
                        div().min_w_0().child(
                            Label::new(detail)
                                .size(LabelSize::Small)
                                .color(detail_color)
                                .truncate(),
                        ),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.create(kind, cx)))
            .into_any_element()
    }

    fn render_checkout_row(
        &self,
        index: usize,
        checkout: Checkout,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let branch = checkout
            .branch
            .clone()
            .unwrap_or_else(|| "detached".to_string());
        let icon = checkout.kind.map_or(IconName::Folder, workspace_icon);
        let is_missing = self.is_missing(&checkout);
        ListItem::new(("worktree-modal-checkout", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .disabled(is_missing)
            .start_slot(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(
                        Label::new(branch).when(is_missing, |label| label.color(Color::Disabled)),
                    ))
                    .child(
                        div().min_w_0().child(
                            Label::new(compact_path(&checkout.path))
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .end_slot(
                Label::new(checkout_kind_label(checkout.kind))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open(checkout.clone(), cx)))
            .into_any_element()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        // The project the workspace is in stands for the repository, as on its row.
        let project = self
            .projects
            .as_ref()
            .and_then(|projects| project_at(projects.read(cx).projects(), &self.folder).cloned());
        let icon = match &project {
            Some(project) => {
                let info = ProjectInfoStore::global(cx)
                    .read(cx)
                    .info(self.machine, project.id)
                    .cloned();
                render_project_icon(project, info.as_ref(), px(14.), cx)
            }
            None => render_folder_icon(),
        };
        let machine_label = (self.machine != MachineId::Local)
            .then(|| Machines::global(cx).read(cx).label(self.machine, cx));
        let title = match self.mode {
            WorktreeModalMode::New => format!("New worktree of {}", self.name),
            WorktreeModalMode::Open => format!("Open a worktree of {}", self.name),
        };
        h_flex()
            .px_3()
            .py_2p5()
            .gap_3()
            .border_b_1()
            .border_color(border_variant)
            .child(
                h_flex()
                    .flex_none()
                    .gap_1p5()
                    .child(icon)
                    .child(Label::new(title).color(Color::Muted))
                    .children(machine_label.map(|label| {
                        Label::new(format!("on {label}"))
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                    })),
            )
            .child(div().flex_1().min_w_0().child(self.input.clone()))
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> AnyElement {
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
        let hint = match self.mode {
            WorktreeModalMode::New => "Enter makes it and opens a terminal there",
            WorktreeModalMode::Open => "Enter opens a terminal there",
        };
        footer
            .child(Label::new(hint).size(LabelSize::Small).color(Color::Muted))
            .into_any_element()
    }
}

impl WorktreeModal {
    /// The branch the new one starts from.
    fn base_branch(&self) -> String {
        self.base
            .clone()
            .or_else(|| self.checkouts()?.git.branch.clone())
            .unwrap_or_else(|| "HEAD".to_string())
    }

    /// Where the selected kind would be made: the server's data folder, the kind's folder,
    /// the repository, and the branch with `/` as `-` (`workspaces::create`).
    fn location(&self, cx: &App) -> Option<String> {
        let repository = self.checkouts()?;
        let name = repository.checkouts.first()?.path.file_name()?;
        let folder = match new_row_kind(self.selected_index) {
            WorkspaceKind::Worktree => "worktrees",
            WorkspaceKind::Pasture => "pastures",
        };
        let branch = self.input.read(cx).text().trim().replace('/', "-");
        Some(format!(
            "{}/{folder}/{}/{branch}",
            repository.data_dir,
            name.to_string_lossy()
        ))
    }

    /// New Worktree's second line: the base branch, which can be changed, and where the
    /// checkout goes.
    fn render_details(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let branches = self
            .checkouts()
            .map(|repository| repository.git.branches.clone())
            .unwrap_or_default();
        let this = cx.entity().downgrade();
        h_flex()
            .px_3()
            .py_1()
            .gap_2()
            .border_b_1()
            .border_color(border_variant)
            .child(
                Label::new("From")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .child(
                PopoverMenu::new("worktree-modal-base")
                    .trigger(
                        Button::new("worktree-modal-base-button", self.base_branch())
                            .label_size(LabelSize::Small)
                            .end_icon(
                                Icon::new(IconName::ChevronDown)
                                    .size(IconSize::XSmall)
                                    .color(Color::Muted),
                            ),
                    )
                    .menu(move |window, cx| {
                        let this = this.clone();
                        let branches = branches.clone();
                        Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                            for branch in &branches {
                                let this = this.clone();
                                let choice = branch.clone();
                                menu = menu.entry(branch.clone(), None, move |_, cx| {
                                    this.update(cx, |this, cx| {
                                        this.base = Some(choice.clone());
                                        cx.notify();
                                    })
                                    .ok();
                                });
                            }
                            menu
                        }))
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .children(self.location(cx).map(|location| {
                        Label::new(format!("in {location}"))
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate_middle()
                    })),
            )
    }
}

/// New Worktree's rows: a worktree first, as herdr offers only those.
fn new_row_kind(index: usize) -> WorkspaceKind {
    if index == 0 {
        WorkspaceKind::Worktree
    } else {
        WorkspaceKind::Pasture
    }
}

/// The checkout `folder` is in: the deepest one holding it, since a worktree may sit inside
/// the main checkout.
fn own_checkout<'a>(checkouts: &'a [Checkout], folder: &Path) -> Option<&'a Checkout> {
    checkouts
        .iter()
        .filter(|checkout| folder.starts_with(&checkout.path))
        .max_by_key(|checkout| checkout.path.components().count())
}

fn checkout_kind_label(kind: Option<WorkspaceKind>) -> &'static str {
    kind.map_or("Main checkout", WorkspaceKind::label)
}

fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or_default()
}

/// herdr's generated branch names (`generated_branch_slug`), under the server's prefix.
fn generated_branch(seed: u64) -> String {
    const ADJECTIVES: [&str; 8] = [
        "brave", "calm", "clear", "green", "lucky", "quiet", "rapid", "silver",
    ];
    const NOUNS: [&str; 8] = [
        "river", "cloud", "field", "forest", "harbor", "meadow", "stone", "valley",
    ];
    let adjective = ADJECTIVES[(seed as usize) % ADJECTIVES.len()];
    let noun = NOUNS[((seed / ADJECTIVES.len() as u64) as usize) % NOUNS.len()];
    let suffix = seed & 0xffff;
    format!("{BRANCH_PREFIX}{adjective}-{noun}-{suffix:04x}")
}

impl Render for WorktreeModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows: Vec<AnyElement> = match self.mode {
            WorktreeModalMode::New => (0..2)
                .map(|index| self.render_kind_row(index, cx))
                .collect(),
            WorktreeModalMode::Open => self
                .rows
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, checkout)| self.render_checkout_row(index, checkout, cx))
                .collect(),
        };
        let note: Option<SharedString> = match (&self.repository, self.mode) {
            (None, WorktreeModalMode::Open) => Some("Reading the repository…".into()),
            (Some(Err(error)), WorktreeModalMode::Open) => Some(error.clone()),
            (Some(Ok(_)), WorktreeModalMode::Open) if rows.is_empty() => {
                Some(if self.input.read(cx).text().trim().is_empty() {
                    "The repository has no other checkouts yet".into()
                } else {
                    "No matching worktrees".into()
                })
            }
            _ => None,
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
            .child(self.render_header(cx))
            .when(self.mode == WorktreeModalMode::New, |modal| {
                modal.child(self.render_details(cx))
            })
            .child(
                div()
                    .id("worktree-modal-rows-scroll")
                    .child(
                        v_flex()
                            .id("worktree-modal-rows")
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
            .child(self.render_footer(cx))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use agentz_protocol::spaces::SpacesSnapshot;
    use agentz_protocol::workspace::ProjectGit;
    use gpui::TestAppContext;

    use super::*;
    use crate::server_client::ServerClient;

    const MACHINE: MachineId = MachineId::Remote(1);

    fn init_machine(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
            let client =
                ServerClient::new_for_test(MACHINE, "Server".into(), SpacesSnapshot::default(), cx);
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
    }

    fn checkout(path: &str, branch: &str, kind: Option<WorkspaceKind>) -> Checkout {
        Checkout {
            path: PathBuf::from(path),
            branch: Some(branch.to_string()),
            kind,
        }
    }

    #[gpui::test]
    fn open_worktree_lists_the_other_checkouts(cx: &mut TestAppContext) {
        init_machine(cx);
        let (modal, cx) = cx.add_window_view(|window, cx| {
            WorktreeModal::new(
                MACHINE,
                PathBuf::from("/repo/main/src"),
                "main".into(),
                WorktreeModalMode::Open,
                window,
                cx,
            )
        });
        let opened = Rc::new(RefCell::new(None));
        cx.update(|_, cx| {
            let opened = opened.clone();
            cx.subscribe(&modal, move |_, event: &WorktreeModalEvent, _| {
                let WorktreeModalEvent::Open { folder, .. } = event;
                opened.replace(Some(folder.clone()));
            })
            .detach();
        });
        modal.update(cx, |modal, cx| {
            modal.set_repository(
                Ok(RepositoryCheckouts {
                    git: ProjectGit::default(),
                    checkouts: vec![
                        checkout("/repo/main", "main", None),
                        checkout(
                            "/repo/main/.worktrees/fix",
                            "fix-login",
                            Some(WorkspaceKind::Worktree),
                        ),
                        checkout(
                            "/data/pastures/grazing",
                            "grazing",
                            Some(WorkspaceKind::Pasture),
                        ),
                    ],
                    data_dir: "~/.agentz".to_string(),
                }),
                cx,
            )
        });
        cx.run_until_parked();
        let branches = |cx: &mut gpui::VisualTestContext| {
            modal.read_with(cx, |modal, _| {
                modal
                    .rows
                    .iter()
                    .filter_map(|checkout| checkout.branch.clone())
                    .collect::<Vec<_>>()
            })
        };
        // The workspace is in the main checkout, so that one isn't offered.
        assert_eq!(branches(cx), vec!["fix-login", "grazing"]);

        modal.update(cx, |modal, cx| {
            modal
                .input
                .update(cx, |input, cx| input.set_text("pasture", cx))
        });
        cx.run_until_parked();
        assert_eq!(branches(cx), vec!["grazing"]);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            opened.borrow().clone(),
            Some(PathBuf::from("/data/pastures/grazing"))
        );
    }

    #[gpui::test]
    fn new_worktree_suggests_a_branch(cx: &mut TestAppContext) {
        init_machine(cx);
        let (modal, cx) = cx.add_window_view(|window, cx| {
            WorktreeModal::new(
                MACHINE,
                PathBuf::from("/repo/main"),
                "main".into(),
                WorktreeModalMode::New,
                window,
                cx,
            )
        });
        let branch = modal.read_with(cx, |modal, cx| modal.input.read(cx).text().clone());
        assert!(branch.starts_with(BRANCH_PREFIX), "{branch}");
    }

    #[test]
    fn generated_branches_follow_herdr() {
        assert_eq!(generated_branch(0), "agentz/brave-river-0000");
        assert_eq!(generated_branch(9), "agentz/calm-cloud-0009");
    }
}
