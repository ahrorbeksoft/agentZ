//! A workspace's New Worktree and Open Worktree… (herdr's worktree overlays): a new worktree
//! or pasture of the workspace's project on a branch named here, or one the project already
//! has. Either opens as a new workspace with a shell there.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agentz_protocol::workspace::{PastureSupport, ProjectGit};
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Task, Window,
};
use projects::{Workspace, WorkspaceKind};
use text_input::{TextInput, TextInputEvent};
use ui::{CommonAnimationExt as _, ListItem, ListItemSpacing, WithScrollbar as _, prelude::*};

use crate::machines::{MachineId, Machines, ProjectKey};
use crate::project_info::{ProjectInfoStore, render_project_icon, workspace_icon};
use crate::project_store::ProjectStore;
use crate::project_switcher::compact_path;

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
    /// The input filters the project's worktrees and pastures.
    Open,
}

pub enum WorktreeModalEvent {
    /// Open a workspace with a shell in the folder.
    Open {
        project: ProjectKey,
        folder: PathBuf,
    },
}

pub struct WorktreeModal {
    project: ProjectKey,
    mode: WorktreeModalMode,
    projects: Option<Entity<ProjectStore>>,
    input: Entity<TextInput>,
    git: Option<ProjectGit>,
    /// What [`WorktreeModalMode::Open`] lists.
    workspaces: Vec<Workspace>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    creating: Option<SharedString>,
    error: Option<SharedString>,
    _load_git: Task<()>,
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
        project: ProjectKey,
        mode: WorktreeModalMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let machines = Machines::global(cx);
        let projects = machines.read(cx).projects(project.machine, cx);
        let input = cx.new(|cx| match mode {
            WorktreeModalMode::New => {
                let mut input = TextInput::new("Branch name", cx);
                input.set_text(generated_branch(seed()), cx);
                input.select_all_text(cx);
                input
            }
            WorktreeModalMode::Open => TextInput::new("Search worktrees…", cx),
        });
        let mut subscriptions = vec![
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
        if let Some(projects) = &projects {
            subscriptions.push(cx.observe(projects, |this, _, cx| this.update_rows(cx)));
        }
        window.focus(&input.focus_handle(cx), cx);
        let load_git = match (mode, &projects) {
            (WorktreeModalMode::New, Some(projects)) => {
                let git = projects.read(cx).project_git(project.project, cx);
                cx.spawn(async move |this, cx| {
                    // An older server, or one that can't read the repository, offers neither.
                    let git = git.await.unwrap_or_default();
                    this.update(cx, |this, cx| {
                        this.git = Some(git);
                        cx.notify();
                    })
                    .ok();
                })
            }
            _ => Task::ready(()),
        };
        let mut this = Self {
            project,
            mode,
            projects,
            input,
            git: None,
            workspaces: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            creating: None,
            error: None,
            _load_git: load_git,
            _subscriptions: subscriptions,
        };
        this.update_rows(cx);
        this
    }

    fn update_rows(&mut self, cx: &mut Context<Self>) {
        if self.mode == WorktreeModalMode::Open {
            let query = self.input.read(cx).text().trim().to_lowercase();
            let heads = ProjectInfoStore::global(cx).read(cx);
            let machine = self.project.machine;
            self.workspaces = self
                .projects
                .as_ref()
                .and_then(|projects| projects.read(cx).project(self.project.project))
                .map(|project| project.workspaces.clone())
                .unwrap_or_default()
                .into_iter()
                .filter(|workspace| {
                    let branch = heads
                        .workspace_head(machine, &workspace.path)
                        .map(|head| head.branch.clone())
                        .or_else(|| workspace.branch.clone())
                        .unwrap_or_default();
                    let text = format!(
                        "{} {branch} {}",
                        workspace.kind.label(),
                        workspace.path.display()
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
            WorktreeModalMode::Open => self.workspaces.len(),
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
                if let Some(workspace) = self.workspaces.get(self.selected_index).cloned() {
                    self.open(workspace, cx);
                }
            }
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.creating.is_none() {
            cx.emit(DismissEvent);
        }
    }

    /// Why a new worktree or pasture can't be made here yet, if it can't.
    fn unavailable(&self, kind: WorkspaceKind) -> Option<String> {
        let git = self.git.as_ref()?;
        if !git.is_repository {
            return Some("Not a git repository".into());
        }
        match (kind, &git.pastures) {
            (WorkspaceKind::Pasture, PastureSupport::Unsupported(reason)) => Some(reason.clone()),
            _ => None,
        }
    }

    fn create(&mut self, kind: WorkspaceKind, cx: &mut Context<Self>) {
        if self.creating.is_some() || self.git.is_none() || self.unavailable(kind).is_some() {
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
            self.project.project,
            kind,
            Some(branch.clone()),
            cx,
        );
        self.creating =
            Some(format!("Making the {} for {branch}…", kind.label().to_lowercase()).into());
        self.error = None;
        cx.notify();
        let project = self.project;
        cx.spawn(async move |this, cx| {
            let created = created.await;
            this.update(cx, |this, cx| {
                this.creating = None;
                match created {
                    Ok(folder) => cx.emit(WorktreeModalEvent::Open { project, folder }),
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

    fn open(&mut self, workspace: Workspace, cx: &mut Context<Self>) {
        if self.is_missing(&workspace) {
            return;
        }
        cx.emit(WorktreeModalEvent::Open {
            project: self.project,
            folder: workspace.path,
        });
    }

    /// Only this Mac's folders can be checked from here.
    fn is_missing(&self, workspace: &Workspace) -> bool {
        self.project.machine == MachineId::Local && !workspace.path.exists()
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
                let (detail, color) = match self.git.as_ref().map(|git| &git.pastures) {
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
        let disabled = self.git.is_none() || unavailable.is_some() || self.creating.is_some();
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

    fn render_workspace_row(
        &self,
        index: usize,
        workspace: Workspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let branch = ProjectInfoStore::global(cx)
            .read(cx)
            .workspace_head(self.project.machine, &workspace.path)
            .map(|head| head.branch.clone())
            .or_else(|| workspace.branch.clone())
            .unwrap_or_else(|| workspace.kind.label().to_string());
        let is_missing = self.is_missing(&workspace);
        ListItem::new(("worktree-modal-workspace", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .disabled(is_missing)
            .start_slot(
                Icon::new(workspace_icon(workspace.kind))
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(
                        Label::new(branch).when(is_missing, |label| label.color(Color::Disabled)),
                    ))
                    .child(
                        div().min_w_0().child(
                            Label::new(compact_path(&workspace.path))
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .end_slot(
                Label::new(workspace.kind.label())
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open(workspace.clone(), cx)))
            .into_any_element()
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let project = self
            .projects
            .as_ref()
            .and_then(|projects| projects.read(cx).project(self.project.project).cloned());
        let info = ProjectInfoStore::global(cx)
            .read(cx)
            .info(self.project.machine, self.project.project)
            .cloned();
        let machine_label = (self.project.machine != MachineId::Local).then(|| {
            Machines::global(cx)
                .read(cx)
                .label(self.project.machine, cx)
        });
        let name = project
            .as_ref()
            .map(|project| project.name().to_string())
            .unwrap_or_default();
        let title = match self.mode {
            WorktreeModalMode::New => format!("New worktree of {name}"),
            WorktreeModalMode::Open => format!("Open a worktree of {name}"),
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
                    .children(
                        project.as_ref().map(|project| {
                            render_project_icon(project, info.as_ref(), px(14.), cx)
                        }),
                    )
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
            WorktreeModalMode::New => {
                let base = self
                    .git
                    .as_ref()
                    .and_then(|git| git.branch.clone())
                    .unwrap_or_else(|| "HEAD".to_string());
                format!("The branch starts from {base} · Enter makes it and opens a terminal there")
            }
            WorktreeModalMode::Open => "Enter opens a terminal there".to_string(),
        };
        footer
            .child(Label::new(hint).size(LabelSize::Small).color(Color::Muted))
            .into_any_element()
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
                .workspaces
                .clone()
                .into_iter()
                .enumerate()
                .map(|(index, workspace)| self.render_workspace_row(index, workspace, cx))
                .collect(),
        };
        let note: Option<SharedString> = match self.mode {
            WorktreeModalMode::Open if rows.is_empty() => {
                Some(if self.input.read(cx).text().trim().is_empty() {
                    "The project has no worktrees or pastures yet".into()
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
    use gpui::TestAppContext;
    use projects::{Project, ProjectId, ProjectsSnapshot};

    use super::*;
    use crate::server_client::ServerClient;

    fn workspace(kind: WorkspaceKind, branch: &str) -> Workspace {
        Workspace {
            kind,
            path: PathBuf::from(format!("/tmp/agentz-test-workspaces/{branch}")),
            branch: Some(branch.to_string()),
            base: Some("main".to_string()),
            created_at: SystemTime::UNIX_EPOCH,
        }
    }

    #[gpui::test]
    fn open_worktree_lists_the_projects_workspaces(cx: &mut TestAppContext) {
        let project = ProjectKey {
            machine: MachineId::Remote(1),
            project: ProjectId(7),
        };
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
            let client = ServerClient::new_for_test(
                project.machine,
                "Server".into(),
                SpacesSnapshot::default(),
                cx,
            );
            client.read(cx).projects().clone().update(cx, |store, cx| {
                store.set_snapshot(
                    ProjectsSnapshot {
                        projects: vec![Project {
                            id: project.project,
                            path: PathBuf::from("/tmp/demo"),
                            custom_name: None,
                            icon: None,
                            workspaces: vec![
                                workspace(WorkspaceKind::Worktree, "fix-login"),
                                workspace(WorkspaceKind::Pasture, "grazing"),
                            ],
                            repository: None,
                        }],
                        ..ProjectsSnapshot::default()
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
        let (modal, cx) = cx.add_window_view(|window, cx| {
            WorktreeModal::new(project, WorktreeModalMode::Open, window, cx)
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
        cx.run_until_parked();
        let branches = |cx: &mut gpui::VisualTestContext| {
            modal.read_with(cx, |modal, _| {
                modal
                    .workspaces
                    .iter()
                    .filter_map(|workspace| workspace.branch.clone())
                    .collect::<Vec<_>>()
            })
        };
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
            Some(PathBuf::from("/tmp/agentz-test-workspaces/grazing"))
        );
    }

    #[gpui::test]
    fn new_worktree_suggests_a_branch(cx: &mut TestAppContext) {
        let project = ProjectKey {
            machine: MachineId::Remote(1),
            project: ProjectId(7),
        };
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
            let client = ServerClient::new_for_test(
                project.machine,
                "Server".into(),
                SpacesSnapshot::default(),
                cx,
            );
            crate::machines::init_for_test(vec![client], cx);
            crate::project_info::init(cx);
        });
        let (modal, cx) = cx.add_window_view(|window, cx| {
            WorktreeModal::new(project, WorktreeModalMode::New, window, cx)
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
