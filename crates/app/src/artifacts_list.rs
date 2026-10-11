//! The Artifacts button's list (`design/artifacts-2`, topics 1 and 2), modeled on the project
//! picker: a search, All · Documents · Designs, and the picked project's artifacts (every one
//! under All projects) grouped by the thread that published them, each opening in the browser.
//! "Open the artifacts page" ends it.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use agentz_protocol::artifacts::{Artifact, ArtifactId, ArtifactKind, DesignState};
use gpui::{
    Action, AnyElement, App, ClipboardItem, Context, DismissEvent, Entity, EventEmitter,
    FocusHandle, Focusable, KeyBinding, PromptLevel, ScrollHandle, Subscription, WeakEntity,
    Window,
};
use projects::ThreadId;
use text_input::{TextInput, TextInputEvent};
use ui::{
    ButtonLike, ContextMenu, HighlightedLabel, ListItem, ListItemSpacing, ToggleButtonGroup,
    ToggleButtonGroupStyle, ToggleButtonSimple, WithScrollbar as _, prelude::*, right_click_menu,
};
use util::ResultExt as _;

use crate::machines::{MachineId, Machines, Scope, ThreadKey};
use crate::project_switcher::fuzzy_match;
use crate::sidebar::format_relative_time;

const KEY_CONTEXT: &str = "ArtifactsList";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

/// Opens the Artifacts button's list at an artifact: a card's Show in List.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = artifacts, no_json)]
pub struct ShowInList {
    pub machine: MachineId,
    pub id: ArtifactId,
}

pub fn artifact_icon(kind: ArtifactKind) -> IconName {
    match kind {
        ArtifactKind::Document => IconName::FileTextOutlined,
        ArtifactKind::Design => IconName::LayoutDashboard,
    }
}

/// A design's state as lists tag it: "Decided" with a check, "2 of 3 decided" or "Waiting for
/// you".
pub fn render_design_state(design: &DesignState, cx: &App) -> AnyElement {
    let is_decided = design.submitted_at.is_some();
    let (color, border) = if is_decided {
        (Color::Success, cx.theme().status().success)
    } else {
        (Color::Accent, cx.theme().colors().text_accent)
    };
    h_flex()
        .flex_none()
        .h(px(18.))
        .px_1p5()
        .gap_0p5()
        .rounded_full()
        .border_1()
        .border_color(border.opacity(0.55))
        .when(is_decided, |tag| {
            tag.child(
                Icon::new(IconName::Check)
                    .size(IconSize::XSmall)
                    .color(color),
            )
        })
        .child(
            Label::new(design.status())
                .size(LabelSize::XSmall)
                .color(color),
        )
        .into_any_element()
}

#[derive(Clone, Copy, PartialEq)]
enum KindFilter {
    All,
    Documents,
    Designs,
}

impl KindFilter {
    fn includes(self, kind: ArtifactKind) -> bool {
        match self {
            KindFilter::All => true,
            KindFilter::Documents => kind == ArtifactKind::Document,
            KindFilter::Designs => kind == ArtifactKind::Design,
        }
    }
}

struct Row {
    machine: MachineId,
    artifact: Artifact,
    /// Its index in [`ArtifactsList::groups`].
    group: usize,
    /// Byte positions of the search's letters in the title, for highlighting.
    positions: Vec<usize>,
}

/// The thread a run of rows came from.
pub(crate) struct Group {
    pub(crate) label: SharedString,
    pub(crate) is_archived: bool,
    /// Whether the thread is still there, for Show Thread.
    pub(crate) has_thread: bool,
}

pub struct ArtifactsList {
    machines: Entity<Machines>,
    open_thread: Rc<dyn Fn(ThreadKey, &mut Window, &mut App)>,
    search: Entity<TextInput>,
    filter: KindFilter,
    groups: Vec<Group>,
    rows: Vec<Row>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    /// A row's right-click menu while it's open: a click on it isn't a click outside the list.
    open_menu: Rc<RefCell<Option<WeakEntity<ContextMenu>>>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for ArtifactsList {}

impl Focusable for ArtifactsList {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl ArtifactsList {
    /// `selected` is the artifact to select, when it's listed.
    pub fn new(
        selected: Option<(MachineId, ArtifactId)>,
        open_thread: impl Fn(ThreadKey, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let machines = Machines::global(cx);
        let placeholder = match machines.read(cx).scope(cx) {
            Scope::Group(key) => match machines.read(cx).group(&key, cx) {
                Some(group) => format!("Search {}’s artifacts…", group.name()),
                None => "Search artifacts…".to_string(),
            },
            Scope::All => "Search artifacts…".to_string(),
        };
        let search = cx.new(|cx| TextInput::new(placeholder, cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.selected_index = 0;
                this.update_rows(None, cx);
            }),
            cx.observe(&machines, |this, _, cx| {
                let selected = this.selected_artifact();
                this.update_rows(selected, cx);
            }),
        ];
        window.focus(&search.focus_handle(cx), cx);

        let mut this = Self {
            machines,
            open_thread: Rc::new(open_thread),
            search,
            filter: KindFilter::All,
            groups: Vec::new(),
            rows: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            open_menu: Rc::default(),
            _subscriptions: subscriptions,
        };
        this.update_rows(selected, cx);
        this.scroll_to_selection();
        this
    }

    fn selected_artifact(&self) -> Option<(MachineId, ArtifactId)> {
        self.rows
            .get(self.selected_index)
            .map(|row| (row.machine, row.artifact.id.clone()))
    }

    /// Lists what the scope, the filter and the search let through, newest first in groups by
    /// their thread, selecting `selected` if it's among them.
    fn update_rows(&mut self, selected: Option<(MachineId, ArtifactId)>, cx: &mut Context<Self>) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let machines = self.machines.read(cx);
        let under_all = machines.scope(cx) == Scope::All;
        let mut keys: Vec<Option<(MachineId, ThreadId)>> = Vec::new();
        let mut groups = Vec::new();
        let mut grouped: Vec<Vec<Row>> = Vec::new();
        for (machine, artifact) in machines.scoped_artifacts(cx) {
            if !self.filter.includes(artifact.kind) {
                continue;
            }
            let (key, group) = thread_group(machines, machine, &artifact, under_all, cx);
            // A match on the thread alone has nothing in the title to highlight.
            let Some(positions) = fuzzy_match(&query, &artifact.title)
                .or_else(|| group.label.to_lowercase().contains(&query).then(Vec::new))
            else {
                continue;
            };
            let index = match keys.iter().position(|existing| *existing == key) {
                Some(index) => index,
                None => {
                    keys.push(key);
                    groups.push(group);
                    grouped.push(Vec::new());
                    grouped.len() - 1
                }
            };
            if let Some(rows) = grouped.get_mut(index) {
                rows.push(Row {
                    machine,
                    artifact,
                    group: index,
                    positions,
                });
            }
        }
        self.groups = groups;
        self.rows = grouped.into_iter().flatten().collect();
        if let Some((machine, id)) = selected
            && let Some(index) = self
                .rows
                .iter()
                .position(|row| row.machine == machine && row.artifact.id == id)
        {
            self.selected_index = index;
        }
        self.selected_index = self.selected_index.min(self.rows.len().saturating_sub(1));
        cx.notify();
    }

    fn set_filter(&mut self, filter: KindFilter, cx: &mut Context<Self>) {
        self.filter = filter;
        self.selected_index = 0;
        self.update_rows(None, cx);
    }

    fn starts_group(&self, index: usize) -> bool {
        let group = self.rows.get(index).map(|row| row.group);
        index == 0
            || index
                .checked_sub(1)
                .and_then(|previous| self.rows.get(previous))
                .map(|row| row.group)
                != group
    }

    /// Keeps the selected row in view. The list's children are its rows plus a heading before
    /// each group.
    fn scroll_to_selection(&self) {
        let Some(row) = self.rows.get(self.selected_index) else {
            return;
        };
        self.scroll_handle
            .scroll_to_item(self.selected_index + row.group + 1);
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if !self.rows.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.rows.len();
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
        if !self.rows.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.rows.len() - 1);
            self.scroll_to_selection();
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        self.open(self.selected_index, cx);
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn open(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        if let Some(link) =
            self.machines
                .read(cx)
                .artifact_link(row.machine, &row.artifact.id, None, cx)
        {
            cx.open_url(&link);
        }
        cx.emit(DismissEvent);
    }

    /// The artifacts page, at the picked project (topic 2's C).
    fn open_artifacts_page(&mut self, cx: &mut Context<Self>) {
        let machines = self.machines.read(cx);
        let project = match machines.scope(cx) {
            Scope::Group(key) => machines.group(&key, cx).map(|group| group.name()),
            Scope::All => None,
        };
        if let Some(link) = machines.artifacts_home_link(project.as_deref(), cx) {
            cx.open_url(&link);
        }
        cx.emit(DismissEvent);
    }

    fn render_group(&self, group: usize, index: usize) -> AnyElement {
        let Some(group) = self.groups.get(group) else {
            return div().into_any_element();
        };
        h_flex()
            .w_full()
            .min_w_0()
            .h_6()
            .px_2()
            .gap_1()
            .when(index > 0, |heading| heading.mt_1())
            .child(
                Label::new(group.label.clone())
                    .size(LabelSize::Small)
                    .color(Color::Muted)
                    .truncate(),
            )
            .when(group.is_archived, |heading| {
                heading.child(
                    h_flex()
                        .flex_none()
                        .gap_1()
                        .child(Label::new("·").size(LabelSize::Small).color(Color::Muted))
                        .child(
                            Icon::new(IconName::Archive)
                                .size(IconSize::XSmall)
                                .color(Color::Muted),
                        )
                        .child(
                            Label::new("Archived")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_row(&self, index: usize, row: &Row, cx: &mut Context<Self>) -> AnyElement {
        let end = match &row.artifact.design {
            Some(design) => render_design_state(design, cx),
            None => Label::new(format_relative_time(
                SystemTime::UNIX_EPOCH + Duration::from_millis(row.artifact.published_at()),
                SystemTime::now(),
            ))
            .size(LabelSize::Small)
            .color(Color::Muted)
            .into_any_element(),
        };
        let selector = format!(
            "artifacts-list-{}-{}",
            row.machine.slug(),
            row.artifact.id.0
        );
        let item = ListItem::new(("artifacts-list-row", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .start_slot(
                Icon::new(artifact_icon(row.artifact.kind))
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                h_flex()
                    .debug_selector(move || selector)
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div().flex_1().min_w_0().child(
                            HighlightedLabel::new(
                                row.artifact.title.clone(),
                                row.positions.clone(),
                            )
                            .truncate(),
                        ),
                    )
                    .child(end),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.open(index, cx)));
        right_click_menu(("artifacts-list-menu", index))
            .trigger(move |_, _, _| item)
            .menu(self.row_menu(row, cx))
            .into_any_element()
    }

    /// A row's right-click menu, the first round's (`design/artifacts`, topics 5 and 14): Open,
    /// Show Thread, Copy Link and Delete….
    fn row_menu(
        &self,
        row: &Row,
        cx: &mut Context<Self>,
    ) -> impl Fn(&mut Window, &mut App) -> Entity<ContextMenu> + 'static {
        let list = cx.entity().downgrade();
        let open_menu = self.open_menu.clone();
        let open_thread = self.open_thread.clone();
        let machine = row.machine;
        let id = row.artifact.id.clone();
        let title = row.artifact.title.clone();
        let link = self.machines.read(cx).artifact_link(machine, &id, None, cx);
        let thread = row
            .artifact
            .thread_id
            .filter(|_| {
                self.groups
                    .get(row.group)
                    .is_some_and(|group| group.has_thread)
            })
            .map(|thread| ThreadKey { machine, thread });
        move |window, cx| {
            let dismiss = {
                let list = list.clone();
                move |cx: &mut App| {
                    list.update(cx, |_, cx| cx.emit(DismissEvent)).log_err();
                }
            };
            let link = link.clone();
            let open_thread = open_thread.clone();
            let id = id.clone();
            let title = title.clone();
            let menu = ContextMenu::build(window, cx, move |menu, _, _| {
                let menu = menu.entry("Open", None, {
                    let link = link.clone();
                    let dismiss = dismiss.clone();
                    move |_, cx| {
                        if let Some(link) = &link {
                            cx.open_url(link);
                        }
                        dismiss(cx);
                    }
                });
                let menu = menu.when_some(thread, |menu, thread| {
                    let open_thread = open_thread.clone();
                    let dismiss = dismiss.clone();
                    menu.entry("Show Thread", None, move |window, cx| {
                        dismiss(cx);
                        // After the list has closed and given focus back, so the thread can
                        // take it.
                        let open_thread = open_thread.clone();
                        window.defer(cx, move |window, cx| open_thread(thread, window, cx));
                    })
                });
                let menu = menu.when_some(link.clone(), |menu, link| {
                    menu.entry("Copy Link", None, move |_, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(link.clone()))
                    })
                });
                let id = id.clone();
                let title = title.clone();
                let dismiss = dismiss.clone();
                menu.entry("Delete…", None, move |window, cx| {
                    dismiss(cx);
                    let id = id.clone();
                    let title = title.clone();
                    window.defer(cx, move |window, cx| {
                        confirm_delete(machine, id, &title, window, cx)
                    });
                })
            });
            open_menu.replace(Some(menu.downgrade()));
            menu
        }
    }

    fn render_filter(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = |label: &'static str, filter: KindFilter, cx: &mut Context<Self>| {
            ToggleButtonSimple::new(
                label,
                cx.listener(move |this, _, _, cx| this.set_filter(filter, cx)),
            )
        };
        ToggleButtonGroup::single_row(
            "artifacts-list-kinds",
            [
                tab("All", KindFilter::All, cx),
                tab("Documents", KindFilter::Documents, cx),
                tab("Designs", KindFilter::Designs, cx),
            ],
        )
        .style(ToggleButtonGroupStyle::Outlined)
        .auto_width()
        .label_size(LabelSize::Small)
        .selected_index(match self.filter {
            KindFilter::All => 0,
            KindFilter::Documents => 1,
            KindFilter::Designs => 2,
        })
    }
}

/// The group an artifact is listed under: its thread, "Chat · …" for a chat's, with its project
/// under All projects; or one group for every deleted thread's.
pub(crate) fn thread_group(
    machines: &Machines,
    machine: MachineId,
    artifact: &Artifact,
    under_all: bool,
    cx: &App,
) -> (Option<(MachineId, ThreadId)>, Group) {
    let deleted = || {
        (
            None,
            Group {
                label: "From a deleted thread".into(),
                is_archived: false,
                has_thread: false,
            },
        )
    };
    let Some((thread_id, store)) = artifact.thread_id.zip(machines.projects(machine, cx)) else {
        return deleted();
    };
    let store = store.read(cx);
    let Some(thread) = store.thread(thread_id) else {
        return deleted();
    };
    let project = artifact
        .project_id
        .and_then(|id| store.project(id))
        .map(|project| project.name());
    let label = match project {
        _ if artifact.is_from_chat() => format!("Chat · {}", thread.title),
        Some(project) if under_all => format!("{project} › {}", thread.title),
        _ => thread.title.clone(),
    };
    (
        Some((machine, thread_id)),
        Group {
            label: label.into(),
            is_archived: thread.archived_at.is_some(),
            has_thread: true,
        },
    )
}

fn confirm_delete(
    machine: MachineId,
    id: ArtifactId,
    title: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let answer = window.prompt(
        PromptLevel::Warning,
        &format!("Delete “{title}”?"),
        Some("The artifact and every version of it will be removed. This can't be undone."),
        &["Delete", "Cancel"],
        cx,
    );
    let machines = Machines::global(cx);
    cx.spawn(async move |cx| {
        if answer.await == Ok(0) {
            machines.update(cx, |machines, cx| {
                if let Some(client) = machines.client(machine, cx) {
                    client
                        .read(cx)
                        .delete_artifact(id, cx)
                        .detach_and_log_err(cx);
                }
            });
        }
    })
    .detach();
}

impl Render for ArtifactsList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let has_artifacts = !self.machines.read(cx).scoped_artifacts(cx).is_empty();
        let mut items = Vec::with_capacity(self.rows.len() + self.groups.len());
        for (index, row) in self.rows.iter().enumerate() {
            if self.starts_group(index) {
                items.push(self.render_group(row.group, index));
            }
            items.push(self.render_row(index, row, cx));
        }
        let count = match self.rows.len() {
            1 => "1 artifact".to_string(),
            count => format!("{count} artifacts"),
        };

        v_flex()
            .key_context(KEY_CONTEXT)
            .debug_selector(|| "artifacts-list".into())
            // Like Zed's menus, a click anywhere outside closes it.
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                let has_open_menu = this
                    .open_menu
                    .borrow()
                    .as_ref()
                    .is_some_and(|menu| menu.upgrade().is_some());
                if !has_open_menu {
                    cx.emit(DismissEvent);
                }
            }))
            .w(rems(24.))
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
                h_flex()
                    .px_2()
                    .pt_2()
                    .pb_1()
                    .gap_2()
                    .child(self.render_filter(cx))
                    .child(div().flex_1())
                    .child(Label::new(count).size(LabelSize::Small).color(Color::Muted)),
            )
            .child(
                // The scrollbar sits on this non-scrolling wrapper so it stays put, as in Zed.
                div()
                    .id("artifacts-list-scroll")
                    .child(
                        v_flex()
                            .id("artifacts-list-rows")
                            .max_h(rems(24.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(items)
                            .when(self.rows.is_empty(), |list| {
                                list.child(
                                    div().px_2().py_1p5().child(
                                        Label::new(if has_artifacts {
                                            "No matching artifacts"
                                        } else {
                                            "No artifacts yet"
                                        })
                                        .color(Color::Muted),
                                    ),
                                )
                            }),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
            .child(
                h_flex()
                    .p_1()
                    .border_t_1()
                    .border_color(border_variant)
                    .child(
                        ButtonLike::new("open-artifacts-page")
                            .full_width()
                            .child(
                                h_flex()
                                    .w_full()
                                    .px_1()
                                    .gap_2()
                                    .debug_selector(|| "open-artifacts-page".into())
                                    .child(
                                        Icon::new(IconName::ArrowUpRight)
                                            .size(IconSize::Small)
                                            .color(Color::Accent),
                                    )
                                    .child(
                                        Label::new("Open the artifacts page").color(Color::Accent),
                                    ),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.open_artifacts_page(cx))),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use agentz_protocol::artifacts::{
        Artifact, ArtifactId, ArtifactKind, ArtifactPages, ArtifactVersion, Artifacts, DesignState,
    };
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::{Entity, TestAppContext, VisualTestContext};
    use projects::{Project, ProjectId, ProjectsSnapshot, Thread, ThreadId};
    use serde_json::json;

    use super::{ArtifactsList, KindFilter};
    use crate::machines::{MachineId, Machines, Scope};
    use crate::server_client::ServerClient;

    fn thread(id: u64, project_id: ProjectId) -> Thread {
        let mut thread: Thread = serde_json::from_value(json!({
            "id": id,
            "project_id": 1,
            "title": format!("Thread {id}"),
            "agent_id": "mock",
        }))
        .expect("a thread");
        thread.project_id = project_id;
        thread
    }

    fn artifact(id: &str, thread: u64, project_id: ProjectId, minutes_ago: u64) -> Artifact {
        Artifact {
            id: ArtifactId(id.to_string()),
            title: format!("Artifact {id}"),
            kind: ArtifactKind::Document,
            versions: vec![ArtifactVersion {
                published_at: (SystemTime::now() - Duration::from_secs(minutes_ago * 60))
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64,
                bytes: 120,
                files: Vec::new(),
            }],
            thread_id: Some(ThreadId(thread)),
            project_id: Some(project_id),
            agent_id: None,
            preview: String::new(),
            design: None,
            frame_key: "key".into(),
        }
    }

    /// Two projects, demo and other: thread 1 in demo, thread 2 in demo and archived, thread
    /// 3 in other, and chat 4. Thread 9, which published "orphan", is gone.
    fn set_up(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            client.update(cx, |client, cx| client.set_online_for_test(cx));
            let project = |id: u64, path: &str| Project {
                id: ProjectId(id),
                path: path.into(),
                custom_name: None,
                icon: None,
                workspaces: Vec::new(),
                repository: None,
            };
            let mut archived = thread(2, ProjectId(1));
            archived.archived_at = Some(SystemTime::now());
            client.read(cx).projects().clone().update(cx, |store, cx| {
                store.set_snapshot(
                    ProjectsSnapshot {
                        projects: vec![project(1, "/tmp/demo"), project(2, "/tmp/other")],
                        threads: vec![
                            thread(1, ProjectId(1)),
                            archived,
                            thread(3, ProjectId(2)),
                            thread(4, ProjectId::CHATS),
                        ],
                        ..Default::default()
                    },
                    cx,
                )
            });
            let mut plans = artifact("plans", 1, ProjectId(1), 1);
            plans.kind = ArtifactKind::Design;
            plans.design = Some(DesignState {
                topics: 1,
                variants: 5,
                ..Default::default()
            });
            client.update(cx, |client, cx| {
                client.set_artifacts_for_test(
                    Artifacts {
                        artifacts: vec![
                            plans,
                            artifact("notes", 2, ProjectId(1), 2),
                            artifact("research", 1, ProjectId(1), 3),
                            artifact("orphan", 9, ProjectId(1), 4),
                            artifact("elsewhere", 3, ProjectId(2), 5),
                            artifact("in-chat", 4, ProjectId::CHATS, 6),
                        ],
                        pages: Some(ArtifactPages {
                            address: "127.0.0.1:4000".into(),
                            token: "token".into(),
                        }),
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client], cx);
        });
    }

    fn pick_demo(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let group = Machines::global(cx)
                .read(cx)
                .project_groups(cx)
                .into_iter()
                .find(|group| group.name() == "demo")
                .expect("demo");
            Machines::set_scope(Scope::Group(group.key), cx);
        });
    }

    fn open_list<'a>(
        selected: Option<&str>,
        cx: &'a mut TestAppContext,
    ) -> (Entity<ArtifactsList>, &'a mut VisualTestContext) {
        let selected = selected.map(|id| (MachineId::Local, ArtifactId(id.to_string())));
        let (list, cx) = cx.add_window_view(move |window, cx| {
            ArtifactsList::new(selected, |_, _, _| {}, window, cx)
        });
        cx.run_until_parked();
        (list, cx)
    }

    fn top(selector: &'static str, cx: &mut VisualTestContext) -> Option<gpui::Pixels> {
        cx.debug_bounds(selector).map(|bounds| bounds.origin.y)
    }

    fn group_labels(list: &Entity<ArtifactsList>, cx: &mut VisualTestContext) -> Vec<String> {
        list.read_with(cx, |list, _| {
            list.groups
                .iter()
                .map(|group| {
                    let archived = if group.is_archived { " (archived)" } else { "" };
                    format!("{}{archived}", group.label)
                })
                .collect()
        })
    }

    /// The picked project's artifacts, without the chats', grouped by their thread (an
    /// archived one says so); the arrow keys and Enter open one.
    #[gpui::test]
    fn lists_the_picked_projects_artifacts_by_thread(cx: &mut TestAppContext) {
        set_up(cx);
        pick_demo(cx);
        let (list, cx) = open_list(None, cx);

        assert_eq!(
            group_labels(&list, cx),
            ["Thread 1", "Thread 2 (archived)", "From a deleted thread"]
        );
        let order = [
            "artifacts-list-local-plans",
            "artifacts-list-local-research",
            "artifacts-list-local-notes",
            "artifacts-list-local-orphan",
        ];
        for pair in order.windows(2) {
            assert!(top(pair[0], cx) < top(pair[1], cx), "{pair:?}");
        }
        assert!(top("artifacts-list-local-elsewhere", cx).is_none());
        assert!(top("artifacts-list-local-in-chat", cx).is_none());

        cx.simulate_keystrokes("down enter");
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("http://127.0.0.1:4000/a/research?t=token")
        );
    }

    #[gpui::test]
    fn narrows_by_kind_and_search(cx: &mut TestAppContext) {
        set_up(cx);
        pick_demo(cx);
        let (list, cx) = open_list(None, cx);

        list.update(cx, |list, cx| list.set_filter(KindFilter::Designs, cx));
        cx.run_until_parked();
        assert!(top("artifacts-list-local-plans", cx).is_some());
        assert!(top("artifacts-list-local-research", cx).is_none());

        list.update(cx, |list, cx| {
            list.set_filter(KindFilter::Documents, cx);
            list.search
                .update(cx, |search, cx| search.set_text("rsrch", cx));
        });
        cx.run_until_parked();
        assert!(top("artifacts-list-local-plans", cx).is_none());
        assert!(top("artifacts-list-local-research", cx).is_some());
        assert!(top("artifacts-list-local-notes", cx).is_none());

        // A thread's title finds what it published.
        list.update(cx, |list, cx| {
            list.search
                .update(cx, |search, cx| search.set_text("thread 2", cx));
        });
        cx.run_until_parked();
        assert!(top("artifacts-list-local-notes", cx).is_some());
        assert!(top("artifacts-list-local-research", cx).is_none());
    }

    /// Under All projects, every artifact, chats' too, with their project before the thread.
    #[gpui::test]
    fn lists_every_artifact_under_all_projects(cx: &mut TestAppContext) {
        set_up(cx);
        let (list, cx) = open_list(Some("in-chat"), cx);

        assert_eq!(
            group_labels(&list, cx),
            [
                "demo › Thread 1",
                "demo › Thread 2 (archived)",
                "From a deleted thread",
                "other › Thread 3",
                "Chat · Thread 4",
            ]
        );
        // Show in List selects it.
        cx.simulate_keystrokes("enter");
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("http://127.0.0.1:4000/a/in-chat?t=token")
        );
    }

    #[gpui::test]
    fn opens_the_artifacts_page_at_the_picked_project(cx: &mut TestAppContext) {
        set_up(cx);
        pick_demo(cx);
        let (_list, cx) = open_list(None, cx);
        let footer = cx
            .debug_bounds("open-artifacts-page")
            .expect("the footer is drawn");
        cx.simulate_click(footer.center(), gpui::Modifiers::none());
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("http://127.0.0.1:4000/?t=token&p=demo")
        );
    }
}
