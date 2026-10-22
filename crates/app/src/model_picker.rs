//! A thread's model and effort, as one menu (`design/composer-2`, topic 2 A, from Codex's
//! /model): a search over the agent's models, then its effort's levels in a list under them,
//! which the user asked for in place of a row of segments that cut their names short. The
//! agent's other model settings (ACP's `model_config` category, such as Claude Agent's Fast
//! mode) follow, as the user asked, rather than in the composer's footer. The composer's
//! "Opus 5.5 · High" chip opens it.

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::AgentId;
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, ScrollHandle, Subscription, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{
    HighlightedLabel, ListItem, ListItemSpacing, Switch, Tooltip, WithScrollbar as _, prelude::*,
};

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

/// Whether a setting goes with the model, in its menu: ACP's `model_config` category.
pub(crate) fn is_model_setting(option: &acp::SessionConfigOption) -> bool {
    option.category == Some(acp::SessionConfigOptionCategory::ModelConfig)
        && matches!(
            option.kind,
            acp::SessionConfigKind::Select(_) | acp::SessionConfigKind::Boolean(_)
        )
}

/// Whether a setting is Fast mode, which Claude Agent and Codex name so, and which the model's
/// chip and menu mark with a bolt.
pub(crate) fn is_fast_mode(option: &acp::SessionConfigOption) -> bool {
    is_model_setting(option)
        && matches!(option.kind, acp::SessionConfigKind::Boolean(_))
        && option.name.to_lowercase().starts_with("fast")
}

/// Whether an agent's setting is left out of the composer and its defaults. Claude Agent's
/// Agent setting picks a persona for the main thread, and its adapter offers Claude Code's
/// own `web-fetch` agent there, which Claude Code then refuses ("Agent \"web-fetch\" not
/// found"); the user asked for it to be hidden.
pub(crate) fn is_hidden_option(
    agent_id: Option<&AgentId>,
    option: &acp::SessionConfigOption,
) -> bool {
    agent_id.is_some_and(|agent_id| agent_id.0 == "claude-acp") && &*option.id.0 == "agent"
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

/// A setting under the models: the effort, or another of the model's settings.
struct Setting {
    id: acp::SessionConfigId,
    name: SharedString,
    description: Option<SharedString>,
    kind: SettingKind,
}

enum SettingKind {
    /// Its choices, in a list, and the current one.
    Levels {
        levels: Vec<(acp::SessionConfigValueId, SharedString)>,
        current: acp::SessionConfigValueId,
    },
    /// On or off, with a switch.
    Switch { is_on: bool, is_fast: bool },
}

impl Setting {
    fn new(option: &acp::SessionConfigOption) -> Option<Self> {
        let kind = match &option.kind {
            acp::SessionConfigKind::Select(select) => SettingKind::Levels {
                levels: select_choices(&select.options)
                    .into_iter()
                    .map(|(_, value, name)| (value, name))
                    .collect(),
                current: select.current_value.clone(),
            },
            acp::SessionConfigKind::Boolean(boolean) => SettingKind::Switch {
                is_on: boolean.current_value,
                is_fast: is_fast_mode(option),
            },
            _ => return None,
        };
        Some(Self {
            id: option.id.clone(),
            name: option.name.clone().into(),
            description: option.description.clone().map(SharedString::from),
            kind,
        })
    }
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

    /// The effort, then the model's other settings, in the agent's order.
    fn settings(&self, cx: &App) -> Vec<Setting> {
        let options = self.thread.read(cx).config_options();
        let (_, effort) = model_and_effort(options);
        effort
            .into_iter()
            .chain(options.iter().filter(|option| is_model_setting(option)))
            .filter_map(Setting::new)
            .collect()
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

    /// The menu stays open, so the value picked shows.
    fn set_setting(
        &mut self,
        id: acp::SessionConfigId,
        value: acp::SessionConfigOptionValue,
        cx: &mut Context<Self>,
    ) {
        self.thread
            .update(cx, |thread, cx| thread.set_config_option(id, value, cx));
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

    fn render_setting(&self, setting: Setting, cx: &mut Context<Self>) -> AnyElement {
        let Setting {
            id,
            name,
            description,
            kind,
        } = setting;
        let section = v_flex()
            .debug_selector({
                let selector = format!("model-setting-{}", id.0);
                move || selector
            })
            .p_1()
            .border_t_1()
            .border_color(cx.theme().colors().border_variant);
        match kind {
            SettingKind::Levels { levels, current } => section
                .child(
                    div()
                        .px_2()
                        .py_0p5()
                        .child(Label::new(name).size(LabelSize::Small).color(Color::Muted)),
                )
                .children(
                    levels
                        .into_iter()
                        .enumerate()
                        .map(|(index, (value, name))| {
                            let is_current = value == current;
                            let selector = format!("{}-{}", id.0, value.0);
                            let id = id.clone();
                            let item = ListItem::new((
                                SharedString::from(format!("model-setting-{}", id.0)),
                                index,
                            ))
                            .inset(true)
                            .spacing(ListItemSpacing::Sparse)
                            .child(div().min_w_0().child(Label::new(name).truncate()))
                            .end_slot(div().w(IconSize::Small.rems()).when(is_current, |this| {
                                this.child(
                                    Icon::new(IconName::Check)
                                        .size(IconSize::Small)
                                        .color(Color::Accent),
                                )
                            }))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    this.set_setting(
                                        id.clone(),
                                        acp::SessionConfigOptionValue::value_id(value.clone()),
                                        cx,
                                    )
                                },
                            ));
                            div().debug_selector(move || selector).child(item)
                        }),
                )
                .into_any_element(),
            SettingKind::Switch { is_on, is_fast } => {
                let toggle = cx.listener({
                    let id = id.clone();
                    move |this, _, _, cx| {
                        this.set_setting(
                            id.clone(),
                            acp::SessionConfigOptionValue::boolean(!is_on),
                            cx,
                        )
                    }
                });
                section
                    .child(
                        ListItem::new(SharedString::from(format!("model-setting-{}", id.0)))
                            .inset(true)
                            .spacing(ListItemSpacing::Sparse)
                            // The bolt the chip shows while it's on.
                            .when(is_fast, |item| {
                                item.start_slot(
                                    Icon::new(IconName::BoltFilled)
                                        .size(IconSize::Small)
                                        .color(Color::Muted),
                                )
                            })
                            .child(div().min_w_0().child(Label::new(name).truncate()))
                            .end_slot(Switch::new(
                                SharedString::from(format!("model-setting-{}-switch", id.0)),
                                if is_on {
                                    ToggleState::Selected
                                } else {
                                    ToggleState::Unselected
                                },
                            ))
                            .when_some(description, |item, description| {
                                item.tooltip(Tooltip::text(description))
                            })
                            .on_click(toggle),
                    )
                    .into_any_element()
            }
        }
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
        let settings: Vec<AnyElement> = self
            .settings(cx)
            .into_iter()
            .map(|setting| self.render_setting(setting, cx))
            .collect();

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
            .children(settings)
    }
}
