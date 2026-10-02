//! The project picker behind the title bar's project button, modeled on Zed's recent-projects
//! popover: search, "All projects", the projects with their icons, and Open Folder.

use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, Subscription, Window,
};
use projects::{ProjectId, ProjectScope, ProjectStore};
use text_input::{TextInput, TextInputEvent};
use ui::{
    ButtonLike, Divider, HighlightedLabel, KeyBinding as KeyBindingHint, ListItem, ListItemSpacing,
    ListSubHeader, Tooltip, prelude::*,
};

use crate::OpenFolder;
use crate::project_info::{ProjectInfoStore, render_project_icon};

const KEY_CONTEXT: &str = "ProjectSwitcher";
const ALL_PROJECTS_LABEL: &str = "All projects";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, PartialEq)]
enum Entry {
    AllProjects,
    Project(ProjectId),
}

impl Entry {
    fn scope(self) -> ProjectScope {
        match self {
            Entry::AllProjects => ProjectScope::All,
            Entry::Project(id) => ProjectScope::Project(id),
        }
    }
}

pub struct ProjectSwitcher {
    store: Entity<ProjectStore>,
    search: Entity<TextInput>,
    entries: Vec<Entry>,
    /// Byte positions of the search's letters in each entry's name, for highlighting.
    match_positions: Vec<Vec<usize>>,
    selected_index: usize,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for ProjectSwitcher {}

impl Focusable for ProjectSwitcher {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl ProjectSwitcher {
    pub fn new(store: Entity<ProjectStore>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new("Search projects…", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.update_entries(cx)
            }),
            cx.observe(&store, |this, _, cx| this.update_entries(cx)),
            cx.observe(&ProjectInfoStore::global(cx), |_, _, cx| cx.notify()),
        ];
        window.focus(&search.focus_handle(cx), cx);

        let mut this = Self {
            store,
            search,
            entries: Vec::new(),
            match_positions: Vec::new(),
            selected_index: 0,
            _subscriptions: subscriptions,
        };
        this.update_entries(cx);
        let current = match this.store.read(cx).scope() {
            ProjectScope::All => Entry::AllProjects,
            ProjectScope::Project(id) => Entry::Project(id),
        };
        if let Some(index) = this.entries.iter().position(|entry| *entry == current) {
            this.selected_index = index;
        }
        this
    }

    fn update_entries(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let store = self.store.read(cx);

        let mut entries = Vec::new();
        let mut match_positions = Vec::new();
        if let Some(positions) = fuzzy_match(&query, ALL_PROJECTS_LABEL) {
            entries.push(Entry::AllProjects);
            match_positions.push(positions);
        }
        for project in store.projects() {
            let positions = fuzzy_match(&query, &project.name()).or_else(|| {
                // A match on the path alone has nothing in the name to highlight.
                project
                    .path
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&query)
                    .then(Vec::new)
            });
            if let Some(positions) = positions {
                entries.push(Entry::Project(project.id));
                match_positions.push(positions);
            }
        }

        self.entries = entries;
        self.match_positions = match_positions;
        self.selected_index = self
            .selected_index
            .min(self.entries.len().saturating_sub(1));
        cx.notify();
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if !self.entries.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.entries.len();
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
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.entries.get(self.selected_index).copied() {
            self.choose(entry, cx);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn choose(&mut self, entry: Entry, cx: &mut Context<Self>) {
        self.store
            .update(cx, |store, cx| store.set_scope(entry.scope(), cx));
        cx.emit(DismissEvent);
    }

    fn render_entry(&self, index: usize, entry: Entry, cx: &mut Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let is_current = store.scope() == entry.scope();
        let positions = self.match_positions.get(index).cloned().unwrap_or_default();
        let check = is_current.then(|| {
            Icon::new(IconName::Check)
                .size(IconSize::Small)
                .color(Color::Accent)
        });
        let item = ListItem::new(("project-switcher-entry", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .on_click(cx.listener(move |this, _, _, cx| this.choose(entry, cx)));

        match entry {
            Entry::AllProjects => {
                let count = store.projects().len();
                let detail = match count {
                    1 => "1 project".to_string(),
                    count => format!("{count} projects"),
                };
                item.start_slot(
                    Icon::new(IconName::ListTree)
                        .size(IconSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    h_flex()
                        .min_w_0()
                        .gap_1()
                        .child(HighlightedLabel::new(ALL_PROJECTS_LABEL, positions))
                        .child(Label::new(detail).color(Color::Muted))
                        .children(check),
                )
                .into_any_element()
            }
            Entry::Project(id) => {
                let Some(project) = store.project(id) else {
                    return div().into_any_element();
                };
                let info = ProjectInfoStore::global(cx).read(cx).info().get(&id);
                let branch = info
                    .and_then(|info| info.git_head.as_ref())
                    .map(|git_head| git_head.branch.clone());
                let name = project.name();
                let path: SharedString = compact_path(&project.path).into();
                let tooltip_title: SharedString = match &branch {
                    Some(branch) => format!("{name}/{branch}").into(),
                    None => name.clone(),
                };
                item.start_slot(render_project_icon(project, info, px(16.), cx))
                    .child(
                        // Like Zed's popover, the path shows on hover rather than in the row.
                        h_flex()
                            .id(("project-switcher-row", index))
                            .min_w_0()
                            .gap_1()
                            .child(HighlightedLabel::new(name, positions))
                            .when_some(branch, |row, branch| {
                                row.child(Label::new(branch).color(Color::Muted).truncate())
                            })
                            .children(check)
                            .tooltip(move |_, cx| {
                                Tooltip::with_meta(tooltip_title.clone(), None, path.clone(), cx)
                            }),
                    )
                    .into_any_element()
            }
        }
    }
}

impl Render for ProjectSwitcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let has_projects = !self.store.read(cx).projects().is_empty();
        let mut rows = Vec::with_capacity(self.entries.len() + 1);
        for (index, entry) in self.entries.clone().into_iter().enumerate() {
            let is_first_project = matches!(entry, Entry::Project(_))
                && !matches!(
                    index
                        .checked_sub(1)
                        .and_then(|previous| self.entries.get(previous)),
                    Some(Entry::Project(_))
                );
            if is_first_project {
                rows.push(
                    v_flex()
                        .w_full()
                        .gap_1()
                        .when(index > 0, |this| this.mt_1().child(Divider::horizontal()))
                        .child(ListSubHeader::new("Projects").inset(true))
                        .into_any_element(),
                );
            }
            rows.push(self.render_entry(index, entry, cx));
        }

        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(22.))
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
                v_flex()
                    .id("project-switcher-entries")
                    .max_h(rems(24.))
                    .overflow_y_scroll()
                    .p_1()
                    .children(rows)
                    .when(self.entries.is_empty(), |list| {
                        list.child(
                            div().px_2().py_1p5().child(
                                Label::new(if has_projects {
                                    "No matching projects"
                                } else {
                                    "Open a folder to add your first project"
                                })
                                .color(Color::Muted),
                            ),
                        )
                    }),
            )
            .child(
                h_flex()
                    .p_1()
                    .border_t_1()
                    .border_color(border_variant)
                    .child(
                        ButtonLike::new("open-folder")
                            .full_width()
                            .child(
                                h_flex()
                                    .w_full()
                                    .px_1()
                                    .gap_2()
                                    .justify_between()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(
                                                Icon::new(IconName::FolderOpen)
                                                    .size(IconSize::Small)
                                                    .color(Color::Muted),
                                            )
                                            .child(Label::new("Open Folder…")),
                                    )
                                    .child(KeyBindingHint::for_action(&OpenFolder, cx)),
                            )
                            .on_click(cx.listener(|_, _, window, cx| {
                                window.dispatch_action(Box::new(OpenFolder), cx);
                                cx.emit(DismissEvent);
                            })),
                    ),
            )
    }
}

/// Case-insensitive subsequence match, returning the byte positions of the matched characters;
/// `query` must already be lowercase.
fn fuzzy_match(query: &str, candidate: &str) -> Option<Vec<usize>> {
    let mut positions = Vec::new();
    let mut candidate_chars = candidate.char_indices();
    for query_char in query.chars() {
        let (position, _) = candidate_chars
            .by_ref()
            .find(|(_, candidate_char)| candidate_char.to_lowercase().eq([query_char]))?;
        positions.push(position);
    }
    Some(positions)
}

pub fn compact_path(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(relative) = path.strip_prefix(&home)
    {
        return format!("~/{}", relative.display());
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::fuzzy_match;

    #[test]
    fn fuzzy_matching() {
        assert_eq!(fuzzy_match("shp", "shop-landing"), Some(vec![0, 1, 3]));
        assert_eq!(fuzzy_match("all", "All projects"), Some(vec![0, 1, 2]));
        assert_eq!(fuzzy_match("xyz", "shop-landing"), None);
        assert_eq!(fuzzy_match("", "anything"), Some(vec![]));
        assert_eq!(fuzzy_match("é", "café"), Some(vec![3]));
    }
}
