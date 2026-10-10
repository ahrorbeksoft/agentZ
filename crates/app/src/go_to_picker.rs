//! Cmd-P: go to a workspace, a tab or a pane, or a thread in Agents, as herdr's Go To picker
//! does. The view on screen's places come first; typing filters them by name or by where
//! they are.

use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{
    Divider, HighlightedLabel, ListItem, ListItemSpacing, ListSubHeader, WithScrollbar as _,
    prelude::*,
};

use crate::controls::AgentIcon;
use crate::machines::{Machines, ProjectKey, ThreadKey};
use crate::project_info::render_project_icon;
use crate::project_switcher::fuzzy_match;
use crate::sidebar::thread_agent_icon;
use crate::spaces_view::{PaneKey, SpaceKey, TabKey};

const KEY_CONTEXT: &str = "GoToPicker";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Space(SpaceKey),
    Tab(TabKey),
    Pane(PaneKey),
    Thread(ThreadKey),
}

#[derive(Clone)]
pub enum PlaceIcon {
    /// Colored already: a thread's agent icon can be on its account's color.
    Icon(AgentIcon),
    /// The project's own icon, as its rows show it.
    Project(ProjectKey),
}

#[derive(Clone)]
pub struct PlaceEntry {
    pub place: Place,
    pub icon: PlaceIcon,
    pub label: SharedString,
    /// Where it is, beside its name.
    pub detail: SharedString,
    pub section: SharedString,
}

/// The Agents view's threads, then its chats, as its sidebar lists them.
pub fn thread_places(cx: &App) -> Vec<PlaceEntry> {
    let machines = Machines::global(cx);
    let machines = machines.read(cx);
    let has_remotes = machines.has_remotes();
    let chats = machines
        .chat_threads(cx)
        .into_iter()
        .map(|(machine, thread)| {
            let detail = if has_remotes {
                machines.label(machine, cx)
            } else {
                SharedString::default()
            };
            PlaceEntry {
                place: Place::Thread(ThreadKey {
                    machine,
                    thread: thread.id,
                }),
                icon: PlaceIcon::Icon(thread_agent_icon(machine, &thread, Color::Muted, cx)),
                label: thread.title.into(),
                detail,
                section: "Chats".into(),
            }
        });
    machines
        .active_threads(cx)
        .into_iter()
        .map(|(machine, thread)| {
            let project = machines.projects(machine, cx).and_then(|store| {
                store
                    .read(cx)
                    .project(thread.project_id)
                    .map(|project| machines.project_label(machine, project, cx))
            });
            let detail: SharedString = match (has_remotes, project) {
                (true, Some(project)) => {
                    format!("{} · {project}", machines.label(machine, cx)).into()
                }
                (true, None) => machines.label(machine, cx),
                (false, project) => project.unwrap_or_default(),
            };
            PlaceEntry {
                place: Place::Thread(ThreadKey {
                    machine,
                    thread: thread.id,
                }),
                icon: PlaceIcon::Icon(thread_agent_icon(machine, &thread, Color::Muted, cx)),
                label: thread.title.into(),
                detail,
                section: "Threads".into(),
            }
        })
        .chain(chats)
        .collect()
}

pub struct GoToPicker {
    places: Vec<PlaceEntry>,
    on_choose: Rc<dyn Fn(Place, &mut Window, &mut App)>,
    search: Entity<TextInput>,
    /// The places that match, by index, with where their names matched.
    matches: Vec<(usize, Vec<usize>)>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    _subscription: Subscription,
}

impl EventEmitter<DismissEvent> for GoToPicker {}

impl Focusable for GoToPicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl GoToPicker {
    pub fn new(
        places: Vec<PlaceEntry>,
        on_choose: impl Fn(Place, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Go to a workspace, tab, pane or thread…", cx));
        let subscription = cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
            this.selected_index = 0;
            this.update_matches(cx);
        });
        window.focus(&search.focus_handle(cx), cx);
        let mut this = Self {
            places,
            on_choose: Rc::new(on_choose),
            search,
            matches: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            _subscription: subscription,
        };
        this.update_matches(cx);
        this
    }

    fn update_matches(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        self.matches = self
            .places
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                fuzzy_match(&query, &entry.label)
                    .or_else(|| entry.detail.to_lowercase().contains(&query).then(Vec::new))
                    .map(|positions| (index, positions))
            })
            .collect();
        self.selected_index = self
            .selected_index
            .min(self.matches.len().saturating_sub(1));
        self.scroll_handle.scroll_to_item(0);
        cx.notify();
    }

    fn section(&self, index: usize) -> Option<&SharedString> {
        let (place, _) = self.matches.get(index)?;
        Some(&self.places[*place].section)
    }

    fn starts_section(&self, index: usize) -> bool {
        index == 0 || self.section(index - 1) != self.section(index)
    }

    fn scroll_to_selection(&self) {
        let headings = (0..=self.selected_index)
            .filter(|index| self.starts_section(*index))
            .count();
        self.scroll_handle
            .scroll_to_item(self.selected_index + headings);
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if !self.matches.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.matches.len();
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
        if !self.matches.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.matches.len() - 1);
            self.scroll_to_selection();
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((place, _)) = self.matches.get(self.selected_index) {
            let place = self.places[*place].place;
            self.choose(place, window, cx);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn choose(&mut self, place: Place, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
        let on_choose = self.on_choose.clone();
        // After the picker has closed and given focus back, so the place can take it.
        window.defer(cx, move |window, cx| on_choose(place, window, cx));
    }

    fn render_icon(icon: &PlaceIcon, cx: &App) -> AnyElement {
        match icon {
            PlaceIcon::Icon(icon) => icon.clone().size(IconSize::Small).into_any_element(),
            PlaceIcon::Project(key) => {
                let project = Machines::global(cx)
                    .read(cx)
                    .projects(key.machine, cx)
                    .and_then(|store| store.read(cx).project(key.project).cloned());
                match project {
                    Some(project) => render_project_icon(key.machine, &project, px(16.), cx),
                    None => Icon::new(IconName::Folder)
                        .size(IconSize::Small)
                        .color(Color::Muted)
                        .into_any_element(),
                }
            }
        }
    }

    fn render_match(&self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (place, positions) = self.matches.get(index)?;
        let entry = &self.places[*place];
        let place = entry.place;
        Some(
            ListItem::new(("go-to-entry", index))
                .inset(true)
                .spacing(ListItemSpacing::Sparse)
                .toggle_state(index == self.selected_index)
                .start_slot(Self::render_icon(&entry.icon, cx))
                .child(
                    h_flex()
                        .debug_selector(|| format!("go-to-{}", entry.label))
                        .min_w_0()
                        .gap_2()
                        .child(HighlightedLabel::new(
                            entry.label.clone(),
                            positions.clone(),
                        ))
                        .child(
                            Label::new(entry.detail.clone())
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                )
                .on_click(cx.listener(move |this, _, window, cx| this.choose(place, window, cx)))
                .into_any_element(),
        )
    }
}

impl Render for GoToPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let mut rows = Vec::with_capacity(self.matches.len() * 2);
        for index in 0..self.matches.len() {
            if self.starts_section(index)
                && let Some(section) = self.section(index).cloned()
            {
                rows.push(
                    v_flex()
                        .w_full()
                        .gap_1()
                        .when(index > 0, |this| this.mt_1().child(Divider::horizontal()))
                        .child(ListSubHeader::new(section).inset(true))
                        .into_any_element(),
                );
            }
            rows.extend(self.render_match(index, cx));
        }

        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(34.))
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
                    .id("go-to-scroll")
                    .child(
                        v_flex()
                            .id("go-to-entries")
                            .max_h(rems(24.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(rows)
                            .when(self.matches.is_empty(), |list| {
                                list.child(
                                    div()
                                        .px_2()
                                        .py_1p5()
                                        .child(Label::new("Nothing matches").color(Color::Muted)),
                                )
                            }),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
    }
}
