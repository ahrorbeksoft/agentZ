//! New Thread's project picker, for when several projects are shown. As in t3code, New Thread
//! then opens a draft in the chosen project, whose screen picks the agent, the checkout and the
//! machine. Chat comes first while chats are on, as t3code's No project does.

use crate::machines::{GroupKey, MachineId, Machines, ProjectKey};
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{ListItem, ListItemSpacing, WithScrollbar as _, prelude::*};

use crate::project_info::render_project_icon;
use crate::project_switcher::compact_path;

const KEY_CONTEXT: &str = "NewThreadModal";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

pub enum NewThreadModalEvent {
    /// The checkout to start a draft in: the chosen project's, on the machine used last.
    ProjectChosen(ProjectKey),
    ChatChosen,
}

#[derive(Clone)]
enum Row {
    Chat,
    Project(GroupKey),
}

pub struct NewThreadModal {
    machines: Entity<Machines>,
    search: Entity<TextInput>,
    rows: Vec<Row>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for NewThreadModal {}
impl EventEmitter<NewThreadModalEvent> for NewThreadModal {}

impl Focusable for NewThreadModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl NewThreadModal {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let machines = Machines::global(cx);
        let search = cx.new(|cx| TextInput::new("Search projects…", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.selected_index = 0;
                this.update_rows(cx);
            }),
            cx.observe(&machines, |this, _, cx| this.update_rows(cx)),
        ];
        window.focus(&search.focus_handle(cx), cx);
        let mut this = Self {
            machines,
            search,
            rows: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        this.update_rows(cx);
        this
    }

    fn update_rows(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let machines = self.machines.read(cx);
        let member_matches = |machine: MachineId, project: &projects::Project| {
            project
                .path
                .to_string_lossy()
                .to_lowercase()
                .contains(&query)
                || (machine != MachineId::Local
                    && machines.label(machine, cx).to_lowercase().contains(&query))
        };
        let offers_chat = Machines::chats_enabled(cx) && "chat".contains(query.as_str());
        let projects = machines
            .visible_groups(cx)
            .into_iter()
            .filter(|group| {
                query.is_empty()
                    || group.name().to_lowercase().contains(&query)
                    || group
                        .members
                        .iter()
                        .any(|(machine, project)| member_matches(*machine, project))
            })
            .map(|group| Row::Project(group.key));
        self.rows = offers_chat
            .then_some(Row::Chat)
            .into_iter()
            .chain(projects)
            .collect();
        self.selected_index = self.selected_index.min(self.rows.len().saturating_sub(1));
        cx.notify();
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.rows.len();
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
        let count = self.rows.len();
        if count > 0 {
            self.selected_index = self.selected_index.checked_sub(1).unwrap_or(count - 1);
            self.scroll_handle.scroll_to_item(self.selected_index);
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        match self.rows.get(self.selected_index).cloned() {
            Some(Row::Chat) => cx.emit(NewThreadModalEvent::ChatChosen),
            Some(Row::Project(key)) => self.choose_group(key, cx),
            None => {}
        }
    }

    /// A project row's choice, unless every machine it's on is offline.
    fn choose_group(&mut self, key: GroupKey, cx: &mut Context<Self>) {
        let machines = self.machines.read(cx);
        if let Some(project) = machines
            .group(&key, cx)
            .and_then(|group| machines.new_thread_member(&group, cx))
        {
            cx.emit(NewThreadModalEvent::ProjectChosen(project));
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    /// Chat, above the projects (and a rule): a thread outside every project.
    fn render_chat_row(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let has_projects = self.rows.len() > 1;
        let item = ListItem::new(("new-thread-project", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .start_slot(
                Icon::new(IconName::Chat)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(Label::new("Chat")))
                    .child(
                        Label::new("outside every project")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
            )
            .on_click(cx.listener(|_, _, _, cx| cx.emit(NewThreadModalEvent::ChatChosen)));
        v_flex()
            .debug_selector(|| "new-thread-chat".into())
            .child(item)
            .when(has_projects, |row| {
                row.child(div().my_1().h_px().bg(cx.theme().colors().border_variant))
            })
            .into_any_element()
    }

    fn render_project_row(
        &self,
        index: usize,
        key: GroupKey,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let machines = self.machines.read(cx);
        let Some(group) = machines.group(&key, cx) else {
            return div().into_any_element();
        };
        let Some((machine, project)) = group.primary() else {
            return div().into_any_element();
        };
        let is_offline = machines.is_group_offline(&group, cx);
        let detail = match group.members.as_slice() {
            [(MachineId::Local, project)] => compact_path(&project.path),
            [(machine, project)] => {
                format!(
                    "{}: {}",
                    machines.label(*machine, cx),
                    project.path.display()
                )
            }
            members => match machines.group_machines_label(&group, cx) {
                Some(label) => label.to_string(),
                None => format!("{} checkouts", members.len()),
            },
        };
        let detail = if is_offline {
            format!("{detail} (offline)")
        } else {
            detail
        };
        let name = group.name();
        ListItem::new(("new-thread-project", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .disabled(is_offline)
            .start_slot(render_project_icon(machine, project, px(16.), cx))
            .child(
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(div().flex_none().child(
                        Label::new(name).when(is_offline, |label| label.color(Color::Disabled)),
                    ))
                    .child(
                        div().min_w_0().child(
                            Label::new(detail)
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.choose_group(key.clone(), cx)))
            .into_any_element()
    }
}

impl Render for NewThreadModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let mut rows: Vec<AnyElement> = Vec::new();
        for (index, row) in self.rows.clone().into_iter().enumerate() {
            match row {
                Row::Chat => rows.push(self.render_chat_row(index, cx)),
                Row::Project(key) => rows.push(self.render_project_row(index, key, cx)),
            }
        }
        let empty_state = rows.is_empty().then(|| {
            div()
                .p_3()
                .child(Label::new("No matching projects").color(Color::Muted))
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
            .child(
                h_flex()
                    .px_3()
                    .py_2p5()
                    .gap_3()
                    .border_b_1()
                    .border_color(border_variant)
                    .child(
                        div()
                            .flex_none()
                            .child(Label::new("New thread in…").color(Color::Muted)),
                    )
                    .child(div().flex_1().min_w_0().child(self.search.clone())),
            )
            .child(
                // The scrollbar sits on this non-scrolling wrapper so it stays put, as in Zed.
                div()
                    .id("new-thread-rows-scroll")
                    .child(
                        v_flex()
                            .id("new-thread-rows")
                            .max_h(rems(26.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(rows)
                            .children(empty_state),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::TestAppContext;
    use projects::{Project, ProjectId, ProjectsSnapshot};

    use super::{NewThreadModal, NewThreadModalEvent};
    use crate::machines::{MachineId, ProjectKey};
    use crate::server_client::ServerClient;

    fn project(id: u64, path: &str) -> Project {
        Project {
            id: ProjectId(id),
            path: path.into(),
            custom_name: None,
            icon: None,
            workspaces: Vec::new(),
            repository: None,
        }
    }

    /// Chat comes first, so Enter starts one. Typing leaves it out unless it matches.
    #[gpui::test]
    fn chat_comes_before_the_projects(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            client.update(cx, |client, cx| client.set_online_for_test(cx));
            let projects = client.read(cx).projects().clone();
            projects.update(cx, |store, cx| {
                store.set_snapshot(
                    ProjectsSnapshot {
                        projects: vec![project(1, "/tmp/demo"), project(2, "/tmp/api")],
                        ..Default::default()
                    },
                    cx,
                )
            });
            crate::machines::init_for_test(vec![client], cx);
            super::init(cx);
        });
        let (modal, cx) = cx.add_window_view(NewThreadModal::new);
        let chosen: Rc<RefCell<Vec<Option<ProjectKey>>>> = Rc::default();
        cx.update(|_, cx| {
            let chosen = chosen.clone();
            cx.subscribe(&modal, move |_, event: &NewThreadModalEvent, _| {
                chosen.borrow_mut().push(match event {
                    NewThreadModalEvent::ProjectChosen(project) => Some(*project),
                    NewThreadModalEvent::ChatChosen => None,
                })
            })
            .detach();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("new-thread-chat").is_some());
        cx.simulate_keystrokes("enter");
        assert_eq!(*chosen.borrow(), vec![None]);

        cx.simulate_input("api");
        cx.run_until_parked();
        assert!(cx.debug_bounds("new-thread-chat").is_none());
        cx.simulate_keystrokes("enter");
        assert_eq!(
            chosen.borrow().last(),
            Some(&Some(ProjectKey {
                machine: MachineId::Local,
                project: ProjectId(2),
            }))
        );

        cx.simulate_keystrokes("backspace backspace backspace");
        cx.simulate_input("ch");
        cx.run_until_parked();
        assert!(cx.debug_bounds("new-thread-chat").is_some());
    }
}
