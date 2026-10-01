use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, Subscription, Window,
};
use projects::{ProjectId, ProjectScope, ProjectStore};
use text_input::{TextInput, TextInputEvent};
use ui::{ButtonLike, KeyBinding as KeyBindingHint, ListItem, ListItemSpacing, prelude::*};

use crate::OpenFolder;

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
        ];
        window.focus(&search.focus_handle(cx), cx);

        let mut this = Self {
            store,
            search,
            entries: Vec::new(),
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
        if query.is_empty() || fuzzy_matches(&query, ALL_PROJECTS_LABEL) {
            entries.push(Entry::AllProjects);
        }
        entries.extend(
            store
                .projects()
                .iter()
                .filter(|project| {
                    query.is_empty()
                        || fuzzy_matches(&query, &project.name())
                        || project
                            .path
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&query)
                })
                .map(|project| Entry::Project(project.id)),
        );

        self.entries = entries;
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

        let (icon, label, detail) = match entry {
            Entry::AllProjects => {
                let count = store.projects().len();
                let detail = match count {
                    1 => "1 project".to_string(),
                    count => format!("{count} projects"),
                };
                (
                    IconName::ListTree,
                    SharedString::from(ALL_PROJECTS_LABEL),
                    detail,
                )
            }
            Entry::Project(id) => {
                let Some(project) = store.project(id) else {
                    return div().into_any_element();
                };
                (
                    IconName::Folder,
                    project.name(),
                    compact_path(&project.path),
                )
            }
        };

        ListItem::new(("project-switcher-entry", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .start_slot(Icon::new(icon).color(Color::Muted).size(IconSize::Small))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(Label::new(label)))
                    .child(
                        div().min_w_0().child(
                            Label::new(detail)
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .when(is_current, |item| {
                item.end_slot(
                    Icon::new(IconName::Check)
                        .size(IconSize::Small)
                        .color(Color::Accent),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| this.choose(entry, cx)))
            .into_any_element()
    }
}

impl Render for ProjectSwitcher {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let has_projects = !self.store.read(cx).projects().is_empty();
        let mut rows = Vec::with_capacity(self.entries.len());
        for (index, entry) in self.entries.clone().into_iter().enumerate() {
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

/// Case-insensitive subsequence match; `query` must already be lowercase.
fn fuzzy_matches(query: &str, candidate: &str) -> bool {
    let mut candidate_chars = candidate.chars().flat_map(char::to_lowercase);
    query
        .chars()
        .all(|query_char| candidate_chars.any(|candidate_char| candidate_char == query_char))
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
    use super::fuzzy_matches;

    #[test]
    fn fuzzy_matching() {
        assert!(fuzzy_matches("shp", "shop-landing"));
        assert!(fuzzy_matches("all", "All projects"));
        assert!(!fuzzy_matches("xyz", "shop-landing"));
        assert!(fuzzy_matches("", "anything"));
    }
}
