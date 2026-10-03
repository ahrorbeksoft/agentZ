//! The project picker behind the title bar's project button, modeled on Zed's recent-projects
//! popover: search, "All projects", the projects with their icons, and Open Folder. With other
//! machines, projects are listed under their machine, and combined ones above them.

use std::rc::Rc;

use crate::machines::{GroupKey, MachineId, Machines, ProjectKey, Scope};
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{
    ButtonLike, Divider, HighlightedLabel, KeyBinding as KeyBindingHint, ListItem, ListItemSpacing,
    ListSubHeader, Tooltip, WithScrollbar as _, prelude::*,
};

use crate::OpenFolder;
use crate::project_info::{ProjectInfoStore, render_project_icon};
use crate::sidebar::render_status_dot;

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

#[derive(Clone, PartialEq)]
enum Entry {
    AllProjects,
    Project(GroupKey),
}

impl Entry {
    fn scope(&self) -> Scope {
        match self {
            Entry::AllProjects => Scope::All,
            Entry::Project(key) => Scope::Group(key.clone()),
        }
    }
}

pub struct ProjectSwitcher {
    machines: Entity<Machines>,
    open_project_settings: Rc<dyn Fn(ProjectKey, &mut Window, &mut App)>,
    search: Entity<TextInput>,
    entries: Vec<Entry>,
    /// The heading each project entry is listed under.
    sections: Vec<SharedString>,
    /// Byte positions of the search's letters in each entry's name, for highlighting.
    match_positions: Vec<Vec<usize>>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for ProjectSwitcher {}

impl Focusable for ProjectSwitcher {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl ProjectSwitcher {
    pub fn new(
        open_project_settings: impl Fn(ProjectKey, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let machines = Machines::global(cx);
        let search = cx.new(|cx| TextInput::new("Search projects…", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.update_entries(cx)
            }),
            cx.observe(&machines, |this, _, cx| this.update_entries(cx)),
            cx.observe(&ProjectInfoStore::global(cx), |_, _, cx| cx.notify()),
        ];
        window.focus(&search.focus_handle(cx), cx);

        let mut this = Self {
            machines,
            open_project_settings: Rc::new(open_project_settings),
            search,
            entries: Vec::new(),
            sections: Vec::new(),
            match_positions: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        this.update_entries(cx);
        let current = match this.machines.read(cx).scope(cx) {
            Scope::All => Entry::AllProjects,
            Scope::Group(key) => Entry::Project(key),
        };
        if let Some(index) = this.entries.iter().position(|entry| *entry == current) {
            this.selected_index = index;
        }
        this
    }

    fn update_entries(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let machines = self.machines.read(cx);

        let mut entries = Vec::new();
        let mut sections = Vec::new();
        let mut match_positions = Vec::new();
        if let Some(positions) = fuzzy_match(&query, ALL_PROJECTS_LABEL) {
            entries.push(Entry::AllProjects);
            sections.push(SharedString::default());
            match_positions.push(positions);
        }
        let mut groups = machines.project_groups(cx);
        let section = |group: &crate::machines::ProjectGroup| -> (usize, SharedString) {
            if !machines.has_remotes() {
                return (0, "Projects".into());
            }
            match group.machines().as_slice() {
                [machine] => (
                    1 + machines
                        .clients()
                        .iter()
                        .position(|client| client.read(cx).machine() == *machine)
                        .unwrap_or(usize::MAX - 1),
                    machines.label(*machine, cx),
                ),
                _ => (0, "On several machines".into()),
            }
        };
        // Stable, so each section keeps the projects' order.
        groups.sort_by_cached_key(|group| section(group).0);
        for group in groups {
            let positions = fuzzy_match(&query, &group.name()).or_else(|| {
                // A match on the path or machine alone has nothing in the name to highlight.
                group
                    .members
                    .iter()
                    .any(|(machine, project)| {
                        project
                            .path
                            .to_string_lossy()
                            .to_lowercase()
                            .contains(&query)
                            || (*machine != MachineId::Local
                                && machines.label(*machine, cx).to_lowercase().contains(&query))
                    })
                    .then(Vec::new)
            });
            if let Some(positions) = positions {
                sections.push(section(&group).1);
                entries.push(Entry::Project(group.key));
                match_positions.push(positions);
            }
        }

        self.entries = entries;
        self.sections = sections;
        self.match_positions = match_positions;
        self.selected_index = self
            .selected_index
            .min(self.entries.len().saturating_sub(1));
        cx.notify();
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if !self.entries.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.entries.len();
            self.scroll_to_selection();
            cx.notify();
        }
    }

    /// Whether the entry is the first of its section, under a heading.
    fn starts_section(&self, index: usize) -> bool {
        matches!(self.entries.get(index), Some(Entry::Project(_)))
            && (index == 0
                || !matches!(self.entries.get(index - 1), Some(Entry::Project(_)))
                || self.sections.get(index - 1) != self.sections.get(index))
    }

    /// Keeps the selected row in view. The list's children are its rows plus a heading before
    /// each section.
    fn scroll_to_selection(&self) {
        let headings = (0..=self.selected_index)
            .filter(|index| self.starts_section(*index))
            .count();
        self.scroll_handle
            .scroll_to_item(self.selected_index + headings);
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

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.entries.get(self.selected_index).cloned() {
            self.choose(&entry, cx);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn choose(&mut self, entry: &Entry, cx: &mut Context<Self>) {
        Machines::set_scope(entry.scope(), cx);
        cx.emit(DismissEvent);
    }

    fn render_entry(&self, index: usize, entry: Entry, cx: &mut Context<Self>) -> AnyElement {
        let machines = self.machines.read(cx);
        let is_current = machines.scope(cx) == entry.scope();
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
            .on_click({
                let entry = entry.clone();
                cx.listener(move |this, _, _, cx| this.choose(&entry, cx))
            });

        match entry {
            Entry::AllProjects => {
                let count = machines.project_groups(cx).len();
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
            Entry::Project(key) => {
                let Some(group) = machines.group(&key, cx) else {
                    return div().into_any_element();
                };
                let Some((machine, project)) = group.primary() else {
                    return div().into_any_element();
                };
                let id = ProjectKey {
                    machine,
                    project: project.id,
                };
                let info = ProjectInfoStore::global(cx)
                    .read(cx)
                    .info(machine, project.id);
                let name = group.name();
                // A project on one machine is under that machine's heading.
                let machine_label = (group.machines().len() > 1)
                    .then(|| machines.group_machines_label(&group, cx))
                    .flatten();
                let path: SharedString = group
                    .members
                    .iter()
                    .map(|(machine, project)| match machine {
                        MachineId::Local => compact_path(&project.path),
                        machine => {
                            format!(
                                "{}: {}",
                                machines.label(*machine, cx),
                                project.path.display()
                            )
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
                    .into();
                let tooltip_title = name.clone();
                let open_project_settings = self.open_project_settings.clone();
                let status = machines
                    .group_status(&group, cx)
                    .map(|status| render_status_dot(status, cx));
                let is_offline = machines.is_group_offline(&group, cx);
                item.start_slot(render_project_icon(project, info, px(16.), cx))
                    .child(
                        // Like Zed's popover, the path shows on hover rather than in the row.
                        h_flex()
                            .id(("project-switcher-row", index))
                            .min_w_0()
                            .gap_1()
                            .child(HighlightedLabel::new(name, positions))
                            .children(machine_label.map(|label| {
                                Label::new(label)
                                    .size(LabelSize::Small)
                                    .color(Color::Muted)
                                    .truncate()
                            }))
                            .when(is_offline, |row| row.opacity(0.5))
                            .children(status)
                            .children(check)
                            .tooltip(move |_, cx| {
                                Tooltip::with_meta(tooltip_title.clone(), None, path.clone(), cx)
                            }),
                    )
                    .end_slot(
                        IconButton::new(("project-settings", index), IconName::Settings)
                            .icon_size(IconSize::Small)
                            .icon_color(Color::Muted)
                            .tooltip(Tooltip::text("Project Settings"))
                            .on_click(cx.listener(move |_, _, window, cx| {
                                cx.stop_propagation();
                                cx.emit(DismissEvent);
                                // After the popover has closed and given focus back, so
                                // settings can take it.
                                let open_project_settings = open_project_settings.clone();
                                window.defer(cx, move |window, cx| {
                                    open_project_settings(id, window, cx)
                                });
                            })),
                    )
                    .into_any_element()
            }
        }
    }
}

impl Render for ProjectSwitcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let has_projects = !self.machines.read(cx).project_groups(cx).is_empty();
        let mut rows = Vec::with_capacity(self.entries.len() + 1);
        for (index, entry) in self.entries.clone().into_iter().enumerate() {
            if self.starts_section(index) {
                let title = self.sections.get(index).cloned().unwrap_or_default();
                rows.push(
                    v_flex()
                        .w_full()
                        .gap_1()
                        .when(index > 0, |this| this.mt_1().child(Divider::horizontal()))
                        .child(ListSubHeader::new(title).inset(true))
                        .into_any_element(),
                );
            }
            rows.push(self.render_entry(index, entry, cx));
        }

        v_flex()
            .key_context(KEY_CONTEXT)
            // Like Zed's menus, a click anywhere outside closes it.
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
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
                // The scrollbar sits on this non-scrolling wrapper so it stays put, as in Zed.
                div()
                    .id("project-switcher-scroll")
                    .child(
                        v_flex()
                            .id("project-switcher-entries")
                            .max_h(rems(24.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
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
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
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
pub(crate) fn fuzzy_match(query: &str, candidate: &str) -> Option<Vec<usize>> {
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
