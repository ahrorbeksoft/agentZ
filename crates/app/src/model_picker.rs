//! A thread's model and effort, as one menu (`design/composer-2`, topic 2 A, from Codex's
//! /model): a search over the agent's models, then its effort's levels as a row of segments.
//! The composer's "Opus 5.5 · High" chip opens it.

use agent_client_protocol::schema::v1 as acp;
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{HighlightedLabel, ListItem, ListItemSpacing, WithScrollbar as _, prelude::*};

use crate::project_switcher::fuzzy_match;
use crate::thread_entity::AgentThread;

const KEY_CONTEXT: &str = "ModelPicker";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

/// The agent's model and effort settings, when it has them as choices.
pub(crate) fn model_and_effort(
    options: &[acp::SessionConfigOption],
) -> (
    Option<&acp::SessionConfigOption>,
    Option<&acp::SessionConfigOption>,
) {
    let select = |category: acp::SessionConfigOptionCategory| {
        options.iter().find(|option| {
            option.category.as_ref() == Some(&category)
                && matches!(option.kind, acp::SessionConfigKind::Select(_))
        })
    };
    (
        select(acp::SessionConfigOptionCategory::Model),
        select(acp::SessionConfigOptionCategory::ThoughtLevel),
    )
}

/// A select setting's choices, each with the group it's in, if any.
pub(crate) fn select_choices(
    options: &acp::SessionConfigSelectOptions,
) -> Vec<(
    Option<SharedString>,
    acp::SessionConfigValueId,
    SharedString,
)> {
    match options {
        acp::SessionConfigSelectOptions::Ungrouped(options) => options
            .iter()
            .map(|choice| (None, choice.value.clone(), choice.name.clone().into()))
            .collect(),
        acp::SessionConfigSelectOptions::Grouped(groups) => groups
            .iter()
            .flat_map(|group| {
                let group_name: SharedString = group.name.clone().into();
                group.options.iter().map(move |choice| {
                    (
                        Some(group_name.clone()),
                        choice.value.clone(),
                        choice.name.clone().into(),
                    )
                })
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The name of a select setting's current choice.
pub(crate) fn current_choice_name(option: &acp::SessionConfigOption) -> Option<SharedString> {
    let acp::SessionConfigKind::Select(select) = &option.kind else {
        return None;
    };
    Some(
        select_choices(&select.options)
            .into_iter()
            .find(|(_, value, _)| *value == select.current_value)
            .map_or_else(
                || select.current_value.0.to_string().into(),
                |(_, _, name)| name,
            ),
    )
}

#[derive(Clone)]
struct ModelEntry {
    group: Option<SharedString>,
    value: acp::SessionConfigValueId,
    name: SharedString,
    positions: Vec<usize>,
}

/// An effort setting's levels and the current one.
struct Effort {
    id: acp::SessionConfigId,
    name: SharedString,
    levels: Vec<(acp::SessionConfigValueId, SharedString)>,
    current: acp::SessionConfigValueId,
}

pub struct ModelPicker {
    thread: Entity<AgentThread>,
    model_option: acp::SessionConfigId,
    search: Entity<TextInput>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for ModelPicker {}

impl Focusable for ModelPicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl ModelPicker {
    pub fn new(
        thread: Entity<AgentThread>,
        model_option: acp::SessionConfigId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search models…", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.selected_index = 0;
                cx.notify();
            }),
            // The agent answers a change with its settings, which may offer other levels.
            cx.observe(&thread, |_, _, cx| cx.notify()),
        ];
        window.focus(&search.focus_handle(cx), cx);
        let mut this = Self {
            thread,
            model_option,
            search,
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            _subscriptions: subscriptions,
        };
        let current = this.current_model(cx);
        this.selected_index = this
            .entries(cx)
            .iter()
            .position(|entry| Some(&entry.value) == current.as_ref())
            .unwrap_or_default();
        this
    }

    fn model_select<'a>(&self, cx: &'a App) -> Option<&'a acp::SessionConfigSelect> {
        self.thread
            .read(cx)
            .config_options()
            .iter()
            .find(|option| option.id == self.model_option)
            .and_then(|option| match &option.kind {
                acp::SessionConfigKind::Select(select) => Some(select),
                _ => None,
            })
    }

    fn current_model(&self, cx: &App) -> Option<acp::SessionConfigValueId> {
        self.model_select(cx)
            .map(|select| select.current_value.clone())
    }

    /// The models the search matches, in the agent's order.
    fn entries(&self, cx: &App) -> Vec<ModelEntry> {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let Some(select) = self.model_select(cx) else {
            return Vec::new();
        };
        select_choices(&select.options)
            .into_iter()
            .filter_map(|(group, value, name)| {
                let positions = fuzzy_match(&query, &name)?;
                Some(ModelEntry {
                    group,
                    value,
                    name,
                    positions,
                })
            })
            .collect()
    }

    fn effort(&self, cx: &App) -> Option<Effort> {
        let thread = self.thread.read(cx);
        let (_, effort) = model_and_effort(thread.config_options());
        let effort = effort?;
        let acp::SessionConfigKind::Select(select) = &effort.kind else {
            return None;
        };
        Some(Effort {
            id: effort.id.clone(),
            name: effort.name.clone().into(),
            levels: select_choices(&select.options)
                .into_iter()
                .map(|(_, value, name)| (value, name))
                .collect(),
            current: select.current_value.clone(),
        })
    }

    /// The selected model's row, after the headers of its group and those before it.
    fn scroll_to_selection(&self, entries: &[ModelEntry]) {
        let mut group = None;
        let mut headers = 0;
        for entry in entries.iter().take(self.selected_index + 1) {
            if entry.group.is_some() && entry.group != group {
                group = entry.group.clone();
                headers += 1;
            }
        }
        self.scroll_handle
            .scroll_to_item(self.selected_index + headers);
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        let entries = self.entries(cx);
        if !entries.is_empty() {
            self.selected_index = (self.selected_index + 1) % entries.len();
            self.scroll_to_selection(&entries);
            cx.notify();
        }
    }

    fn select_previous(
        &mut self,
        _: &menu::SelectPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entries = self.entries(cx);
        if !entries.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(entries.len() - 1);
            self.scroll_to_selection(&entries);
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(entry) = self.entries(cx).get(self.selected_index) {
            self.choose(entry.value.clone(), cx);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn choose(&mut self, value: acp::SessionConfigValueId, cx: &mut Context<Self>) {
        let model_option = self.model_option.clone();
        self.thread.update(cx, |thread, cx| {
            thread.set_config_option(
                model_option,
                acp::SessionConfigOptionValue::value_id(value),
                cx,
            )
        });
        cx.emit(DismissEvent);
    }

    /// The menu stays open, so the level picked shows.
    fn set_effort(
        &mut self,
        id: acp::SessionConfigId,
        value: acp::SessionConfigValueId,
        cx: &mut Context<Self>,
    ) {
        self.thread.update(cx, |thread, cx| {
            thread.set_config_option(id, acp::SessionConfigOptionValue::value_id(value), cx)
        });
    }

    fn render_entry(
        &self,
        index: usize,
        entry: ModelEntry,
        current: Option<&acp::SessionConfigValueId>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_current = current == Some(&entry.value);
        let selector = format!("model-{}", entry.value.0);
        let value = entry.value.clone();
        let item = ListItem::new(("model-picker-entry", index))
            .inset(true)
            .spacing(ListItemSpacing::Sparse)
            .toggle_state(index == self.selected_index)
            .child(
                div()
                    .min_w_0()
                    .overflow_hidden()
                    .child(HighlightedLabel::new(entry.name, entry.positions).truncate()),
            )
            .end_slot(div().w(IconSize::Small.rems()).when(is_current, |this| {
                this.child(
                    Icon::new(IconName::Check)
                        .size(IconSize::Small)
                        .color(Color::Accent),
                )
            }))
            .on_click(cx.listener(move |this, _, _, cx| this.choose(value.clone(), cx)));
        div()
            .debug_selector(move || selector)
            .child(item)
            .into_any_element()
    }

    fn render_effort(&self, effort: Effort, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let border_variant = colors.border_variant;
        let selected_background = colors.element_selected;
        let hover_background = colors.element_hover;
        let Effort {
            id,
            name,
            levels,
            current,
        } = effort;
        v_flex()
            .debug_selector(|| "model-picker-effort".into())
            .px_2()
            .py_1p5()
            .gap_1()
            .border_t_1()
            .border_color(border_variant)
            .child(Label::new(name).size(LabelSize::Small).color(Color::Muted))
            .child(
                h_flex()
                    .w_full()
                    .p_0p5()
                    .gap_0p5()
                    .rounded_md()
                    .border_1()
                    .border_color(border_variant)
                    .children(
                        levels
                            .into_iter()
                            .enumerate()
                            .map(|(index, (value, name))| {
                                let is_current = value == current;
                                let selector = format!("effort-{}", value.0);
                                let id = id.clone();
                                div()
                                    .id(("effort-level", index))
                                    .debug_selector(move || selector)
                                    .flex_1()
                                    .min_w_0()
                                    .py_0p5()
                                    .rounded_sm()
                                    .text_center()
                                    .text_ui_sm(cx)
                                    .map(|segment| {
                                        if is_current {
                                            segment.bg(selected_background).text_color(colors.text)
                                        } else {
                                            segment
                                                .text_color(colors.text_muted)
                                                .cursor_pointer()
                                                .hover(move |style| style.bg(hover_background))
                                        }
                                    })
                                    .child(div().truncate().child(name))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_effort(id.clone(), value.clone(), cx)
                                    }))
                            }),
                    ),
            )
            .into_any_element()
    }
}

impl Render for ModelPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let entries = self.entries(cx);
        let current = self.current_model(cx);
        let mut rows = Vec::with_capacity(entries.len());
        let mut group = None;
        for (index, entry) in entries.iter().cloned().enumerate() {
            if entry.group.is_some() && entry.group != group {
                group = entry.group.clone();
                rows.push(
                    div()
                        .px_2()
                        .pt_1()
                        .child(
                            Label::new(group.clone().unwrap_or_default())
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                        )
                        .into_any_element(),
                );
            }
            rows.push(self.render_entry(index, entry, current.as_ref(), cx));
        }
        let effort = self.effort(cx).map(|effort| self.render_effort(effort, cx));

        v_flex()
            .key_context(KEY_CONTEXT)
            .debug_selector(|| "model-picker".into())
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            .w(rems(18.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(
                h_flex()
                    .px_3()
                    .py_1p5()
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
                    .id("model-picker-scroll")
                    .child(
                        v_flex()
                            .id("model-picker-entries")
                            .max_h(rems(18.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(rows)
                            .when(entries.is_empty(), |list| {
                                list.child(
                                    div().px_2().py_1p5().child(
                                        Label::new("No matching models")
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    ),
                                )
                            }),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
            .children(effort)
    }
}
