//! A thread's model and effort, as one menu (`design/composer-2`, topic 2 A, from Codex's
//! /model): a search over the agent's models, then Manage Models… and the effort's row, whose
//! levels open beside the menu (`design/model-visibility`, topic 13 A). The models hidden in
//! Settings › Agents › an agent › Models are left out, the favorites come first and the rest
//! follow in the user's order (topics 2, 8 and 10). The agent's other model settings (ACP's
//! `model_config` category, such as Claude Agent's Fast mode) follow, as the user asked,
//! rather than in the composer's footer. The composer's "Opus 5.5 · High" chip opens it.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::accounts::AccountId;
use agentz_protocol::agents::AgentId;
use agentz_protocol::models::{self, ModelPreferences};
use gpui::{
    AnyElement, App, Bounds, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, KeyContext, MouseDownEvent, Pixels, ScrollHandle, Subscription, Window, canvas,
};
use text_input::{TextInput, TextInputEvent};
use ui::{
    Chip, HighlightedLabel, ListItem, ListItemSpacing, Switch, Tooltip, WithScrollbar as _,
    prelude::*,
};

use crate::app_settings::AppSettingsStore;
use crate::project_switcher::fuzzy_match;
use crate::server_client::ServerClient;
use crate::thread_entity::AgentThread;

const KEY_CONTEXT: &str = "ModelPicker";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
        // Over the search's own → and ←, only while they open or close the effort's levels.
        KeyBinding::new(
            "right",
            menu::SelectChild,
            Some("ModelPicker && effort_row > TextInput"),
        ),
        KeyBinding::new(
            "left",
            menu::SelectParent,
            Some("ModelPicker && effort_levels > TextInput"),
        ),
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

/// The model preferences the account's pickers follow, `None` being the External account:
/// its own while it keeps them, else Shared's.
pub(crate) fn followed_preferences(
    client: &ServerClient,
    agent_id: &AgentId,
    account: Option<AccountId>,
    cx: &App,
) -> ModelPreferences {
    let own = client.account_settings(agent_id, account).models;
    models::followed(
        &client.accounts(agent_id),
        own.as_ref(),
        AppSettingsStore::global(cx)
            .read(cx)
            .settings()
            .model_preferences
            .get(agent_id),
    )
    .cloned()
    .unwrap_or_default()
}

/// A model the picker lists.
#[derive(Clone)]
pub(crate) struct PickerModel {
    /// The agent's group for it, if it groups its models.
    pub group: Option<SharedString>,
    pub value: acp::SessionConfigValueId,
    pub name: SharedString,
    pub is_favorite: bool,
    /// Hidden from the pickers, and listed only because it's `keep`.
    pub is_hidden: bool,
}

/// The models the pickers list of a model setting's choices: the shown ones, and `keep` (the
/// one in use) in its place even when it's hidden (`design/model-visibility`, topic 3 A).
/// Favorites come first, then the agent's groups as it orders them, each in the user's order.
pub(crate) fn picker_models(
    options: &acp::SessionConfigSelectOptions,
    preferences: &ModelPreferences,
    keep: Option<&acp::SessionConfigValueId>,
) -> Vec<PickerModel> {
    let mut group_ranks: HashMap<Option<SharedString>, usize> = HashMap::new();
    let mut models: Vec<PickerModel> = select_choices(options)
        .into_iter()
        .filter_map(|(group, value, name)| {
            let ranks = group_ranks.len();
            group_ranks.entry(group.clone()).or_insert(ranks);
            let is_hidden = !preferences.is_shown(&value.0);
            (!is_hidden || keep == Some(&value)).then(|| PickerModel {
                is_favorite: preferences.is_favorite(&value.0),
                group,
                value,
                name,
                is_hidden,
            })
        })
        .collect();
    preferences.sort(&mut models, |model| model.value.0.as_ref());
    models.sort_by_key(|model| {
        if model.is_favorite {
            0
        } else {
            1 + group_ranks.get(&model.group).copied().unwrap_or_default()
        }
    });
    models
}

#[derive(Clone)]
struct ModelEntry {
    /// The heading it's under: Favorites, the agent's group, or All after the favorites.
    heading: Option<SharedString>,
    value: acp::SessionConfigValueId,
    name: SharedString,
    is_hidden: bool,
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
    /// Its choices, and the current one.
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

/// A row the keys move through: a model, then Manage Models… and the effort's row.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Model(usize),
    ManageModels,
    Effort,
}

/// The effort's levels beside the menu, while they're open.
struct OpenLevels {
    /// The level the keys are on, once → or Enter opened them; hovering opens them without.
    selected: Option<usize>,
}

pub struct ModelPicker {
    thread: Entity<AgentThread>,
    model_option: acp::SessionConfigId,
    preferences: ModelPreferences,
    manage_models: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    search: Entity<TextInput>,
    selected_index: usize,
    levels: Option<OpenLevels>,
    /// Where the levels were drawn, so a click on them doesn't close the menu.
    levels_bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
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
    /// `manage_models` opens the agent's Models tab in Settings, from the menu's Manage
    /// Models….
    pub fn new(
        thread: Entity<AgentThread>,
        model_option: acp::SessionConfigId,
        preferences: ModelPreferences,
        manage_models: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search models…", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.selected_index = 0;
                this.levels = None;
                cx.notify();
            }),
            // The agent answers a change with its settings, which may offer other levels.
            cx.observe(&thread, |_, _, cx| cx.notify()),
        ];
        window.focus(&search.focus_handle(cx), cx);
        let mut this = Self {
            thread,
            model_option,
            preferences,
            manage_models,
            search,
            selected_index: 0,
            levels: None,
            levels_bounds: Rc::default(),
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

    /// The models the search matches, as [`picker_models`] orders them, under Favorites
    /// then the agent's groups, or All, once there are favorites.
    fn entries(&self, cx: &App) -> Vec<ModelEntry> {
        let query = self.search.read(cx).text().trim().to_lowercase();
        let Some(select) = self.model_select(cx) else {
            return Vec::new();
        };
        let models: Vec<(PickerModel, Vec<usize>)> = picker_models(
            &select.options,
            &self.preferences,
            Some(&select.current_value),
        )
        .into_iter()
        .filter_map(|model| {
            let positions = fuzzy_match(&query, &model.name)?;
            Some((model, positions))
        })
        .collect();
        let has_favorites = models.iter().any(|(model, _)| model.is_favorite);
        models
            .into_iter()
            .map(|(model, positions)| ModelEntry {
                heading: if model.is_favorite {
                    Some("Favorites".into())
                } else {
                    model.group.or_else(|| has_favorites.then(|| "All".into()))
                },
                value: model.value,
                name: model.name,
                is_hidden: model.is_hidden,
                positions,
            })
            .collect()
    }

    /// The effort, when the agent has one.
    fn effort(&self, cx: &App) -> Option<Setting> {
        let (_, effort) = model_and_effort(self.thread.read(cx).config_options());
        effort.and_then(Setting::new)
    }

    /// The model's other settings, in the agent's order.
    fn settings(&self, cx: &App) -> Vec<Setting> {
        self.thread
            .read(cx)
            .config_options()
            .iter()
            .filter(|option| is_model_setting(option))
            .filter_map(Setting::new)
            .collect()
    }

    fn rows(&self, entries: &[ModelEntry], cx: &App) -> Vec<Row> {
        (0..entries.len())
            .map(Row::Model)
            .chain(self.manage_models.is_some().then_some(Row::ManageModels))
            .chain(self.effort(cx).is_some().then_some(Row::Effort))
            .collect()
    }

    fn selected_row(&self, entries: &[ModelEntry], cx: &App) -> Option<Row> {
        self.rows(entries, cx).get(self.selected_index).copied()
    }

    /// The effort's levels and the current one's place among them.
    fn levels(&self, cx: &App) -> Option<(Setting, Vec<acp::SessionConfigValueId>, usize)> {
        let effort = self.effort(cx)?;
        let SettingKind::Levels { levels, current } = &effort.kind else {
            return None;
        };
        let values: Vec<_> = levels.iter().map(|(value, _)| value.clone()).collect();
        let current = values
            .iter()
            .position(|value| value == current)
            .unwrap_or_default();
        Some((effort, values, current))
    }

    /// The selected model's row, after the headings of its section and those before it.
    fn scroll_to_selection(&self, entries: &[ModelEntry]) {
        if self.selected_index >= entries.len() {
            return;
        }
        let mut heading = None;
        let mut headings = 0;
        for entry in entries.iter().take(self.selected_index + 1) {
            if entry.heading.is_some() && entry.heading != heading {
                heading = entry.heading.clone();
                headings += 1;
            }
        }
        self.scroll_handle
            .scroll_to_item(self.selected_index + headings);
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(count) = self.keyboard_levels_count(cx) {
            if let Some(OpenLevels {
                selected: Some(selected),
            }) = &mut self.levels
            {
                *selected = (*selected + 1) % count;
            }
            cx.notify();
            return;
        }
        let entries = self.entries(cx);
        let count = self.rows(&entries, cx).len();
        if count > 0 {
            self.selected_index = (self.selected_index + 1) % count;
            self.levels = None;
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
        if let Some(count) = self.keyboard_levels_count(cx) {
            if let Some(OpenLevels {
                selected: Some(selected),
            }) = &mut self.levels
            {
                *selected = selected.checked_sub(1).unwrap_or(count - 1);
            }
            cx.notify();
            return;
        }
        let entries = self.entries(cx);
        let count = self.rows(&entries, cx).len();
        if count > 0 {
            self.selected_index = self.selected_index.checked_sub(1).unwrap_or(count - 1);
            self.levels = None;
            self.scroll_to_selection(&entries);
            cx.notify();
        }
    }

    /// How many levels the keys move through, while → or Enter opened them.
    fn keyboard_levels_count(&self, cx: &App) -> Option<usize> {
        self.levels.as_ref()?.selected?;
        let (_, values, _) = self.levels(cx)?;
        (!values.is_empty()).then_some(values.len())
    }

    fn confirm(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(OpenLevels {
            selected: Some(selected),
        }) = self.levels
        {
            if let Some((effort, values, _)) = self.levels(cx)
                && let Some(value) = values.get(selected)
            {
                self.choose_level(effort.id, value.clone(), cx);
            }
            return;
        }
        let entries = self.entries(cx);
        match self.selected_row(&entries, cx) {
            Some(Row::Model(index)) => {
                if let Some(entry) = entries.get(index) {
                    self.choose(entry.value.clone(), cx);
                }
            }
            Some(Row::ManageModels) => self.manage_models(window, cx),
            Some(Row::Effort) => self.open_levels_for_keys(cx),
            None => {}
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if self.levels.take().is_some() {
            cx.notify();
            return;
        }
        cx.emit(DismissEvent);
    }

    /// → on the effort's row.
    fn select_child(&mut self, _: &menu::SelectChild, _: &mut Window, cx: &mut Context<Self>) {
        self.open_levels_for_keys(cx);
    }

    /// ← in the effort's levels.
    fn select_parent(&mut self, _: &menu::SelectParent, _: &mut Window, cx: &mut Context<Self>) {
        self.levels = None;
        cx.notify();
    }

    fn open_levels_for_keys(&mut self, cx: &mut Context<Self>) {
        if let Some((_, _, current)) = self.levels(cx) {
            self.levels = Some(OpenLevels {
                selected: Some(current),
            });
            cx.notify();
        }
    }

    fn open_levels_on_hover(&mut self, cx: &mut Context<Self>) {
        if self.levels.is_none() {
            self.levels = Some(OpenLevels { selected: None });
            cx.notify();
        }
    }

    /// Hovering another row closes the levels, as a menu's submenu closes.
    fn close_levels_on_hover(&mut self, hovered: bool, cx: &mut Context<Self>) {
        if hovered && self.levels.take().is_some() {
            cx.notify();
        }
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

    /// Picking a level closes the levels and the menu.
    fn choose_level(
        &mut self,
        id: acp::SessionConfigId,
        value: acp::SessionConfigValueId,
        cx: &mut Context<Self>,
    ) {
        self.thread.update(cx, |thread, cx| {
            thread.set_config_option(id, acp::SessionConfigOptionValue::value_id(value), cx)
        });
        cx.emit(DismissEvent);
    }

    fn manage_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(manage_models) = self.manage_models.clone() {
            cx.emit(DismissEvent);
            manage_models(window, cx);
        }
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

    fn dismiss_on_mouse_down_out(
        &mut self,
        event: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let on_levels = self.levels.is_some()
            && self
                .levels_bounds
                .get()
                .is_some_and(|bounds| bounds.contains(&event.position));
        if !on_levels {
            cx.emit(DismissEvent);
        }
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
                h_flex()
                    .min_w_0()
                    .gap_2()
                    .overflow_hidden()
                    .child(
                        HighlightedLabel::new(entry.name, entry.positions)
                            .when(entry.is_hidden, |label| label.color(Color::Placeholder))
                            .truncate(),
                    )
                    .when(entry.is_hidden, |row| {
                        row.child(
                            div()
                                .debug_selector(|| "model-hidden-tag".into())
                                .child(Chip::new("Hidden").label_color(Color::Muted)),
                        )
                    }),
            )
            .end_slot(div().w(IconSize::Small.rems()).when(is_current, |this| {
                this.child(
                    Icon::new(IconName::Check)
                        .size(IconSize::Small)
                        .color(Color::Accent),
                )
            }))
            .on_hover(cx.listener(|this, hovered, _, cx| this.close_levels_on_hover(*hovered, cx)))
            .on_click(cx.listener(move |this, _, _, cx| this.choose(value.clone(), cx)));
        div()
            .debug_selector(move || selector)
            .child(item)
            .into_any_element()
    }

    /// Manage Models…, which opens the agent's Models tab, as the agent picker ends with
    /// Manage Agents….
    fn render_manage_models(&self, is_selected: bool, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .debug_selector(|| "model-picker-manage".into())
            .p_1()
            .border_t_1()
            .border_color(cx.theme().colors().border_variant)
            .child(
                ListItem::new("model-picker-manage")
                    .inset(true)
                    .spacing(ListItemSpacing::Sparse)
                    .toggle_state(is_selected)
                    .start_slot(
                        Icon::new(IconName::Settings)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(Label::new("Manage Models…"))
                    .on_hover(
                        cx.listener(|this, hovered, _, cx| {
                            this.close_levels_on_hover(*hovered, cx)
                        }),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.manage_models(window, cx))),
            )
            .into_any_element()
    }

    /// The effort's row: its name, the current level and a chevron ("Extra High ›"), which
    /// hovering or → opens.
    fn render_effort_row(
        &self,
        effort: &Setting,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let current = match &effort.kind {
            SettingKind::Levels { levels, current } => levels
                .iter()
                .find(|(value, _)| value == current)
                .map(|(_, name)| name.clone())
                .unwrap_or_else(|| current.0.to_string().into()),
            SettingKind::Switch { .. } => SharedString::default(),
        };
        let selector = format!("model-setting-{}", effort.id.0);
        v_flex()
            .debug_selector(move || selector)
            .p_1()
            .border_t_1()
            .border_color(cx.theme().colors().border_variant)
            .child(
                ListItem::new("model-picker-effort")
                    .inset(true)
                    .spacing(ListItemSpacing::Sparse)
                    .toggle_state(is_selected || self.levels.is_some())
                    .child(
                        div()
                            .min_w_0()
                            .child(Label::new(effort.name.clone()).truncate()),
                    )
                    .end_slot(
                        h_flex()
                            .gap_1()
                            .child(Label::new(current).color(Color::Muted))
                            .child(
                                Icon::new(IconName::ChevronRight)
                                    .size(IconSize::XSmall)
                                    .color(Color::Muted),
                            ),
                    )
                    .when_some(effort.description.clone(), |item, description| {
                        item.tooltip(Tooltip::text(description))
                    })
                    .on_hover(cx.listener(|this, hovered, _, cx| {
                        if *hovered {
                            this.open_levels_on_hover(cx);
                        }
                    }))
                    .on_click(cx.listener(|this, _, _, cx| this.open_levels_on_hover(cx))),
            )
            .into_any_element()
    }

    /// The effort's levels beside the menu, the current one checked.
    fn render_levels(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.levels.as_ref()?;
        let (effort, _, _) = self.levels(cx)?;
        let SettingKind::Levels { levels, current } = effort.kind else {
            return None;
        };
        let id = effort.id;
        let bounds = self.levels_bounds.clone();
        Some(
            v_flex()
                .debug_selector(|| "model-picker-levels".into())
                .relative()
                .min_w(rems(9.375))
                .p_1()
                .elevation_3(cx)
                .children(
                    levels
                        .into_iter()
                        .enumerate()
                        .map(|(index, (value, name))| {
                            let is_current = value == current;
                            let selector = format!("{}-{}", id.0, value.0);
                            let id = id.clone();
                            let item = ListItem::new(("model-picker-level", index))
                                .inset(true)
                                .spacing(ListItemSpacing::Sparse)
                                .toggle_state(open.selected == Some(index))
                                .child(div().min_w_0().child(Label::new(name).truncate()))
                                .end_slot(div().w(IconSize::Small.rems()).when(
                                    is_current,
                                    |this| {
                                        this.child(
                                            Icon::new(IconName::Check)
                                                .size(IconSize::Small)
                                                .color(Color::Accent),
                                        )
                                    },
                                ))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.choose_level(id.clone(), value.clone(), cx)
                                }));
                            div().debug_selector(move || selector).child(item)
                        }),
                )
                .child(
                    canvas(move |area, _, _| bounds.set(Some(area)), |_, _, _, _| {})
                        .absolute()
                        .size_full(),
                )
                .into_any_element(),
        )
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
                            .on_hover(cx.listener(|this, hovered, _, cx| {
                                this.close_levels_on_hover(*hovered, cx)
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
                            .on_hover(cx.listener(|this, hovered, _, cx| {
                                this.close_levels_on_hover(*hovered, cx)
                            }))
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
        let selected_row = self.selected_row(&entries, cx);
        let mut rows = Vec::with_capacity(entries.len());
        let mut heading = None;
        for (index, entry) in entries.iter().cloned().enumerate() {
            if entry.heading.is_some() && entry.heading != heading {
                heading = entry.heading.clone();
                rows.push(
                    div()
                        .px_2()
                        .pt_1()
                        .child(
                            Label::new(heading.clone().unwrap_or_default())
                                .size(LabelSize::XSmall)
                                .color(Color::Muted),
                        )
                        .into_any_element(),
                );
            }
            rows.push(self.render_entry(index, entry, current.as_ref(), cx));
        }
        let manage_models = self
            .manage_models
            .is_some()
            .then(|| self.render_manage_models(selected_row == Some(Row::ManageModels), cx));
        let effort = self
            .effort(cx)
            .map(|effort| self.render_effort_row(&effort, selected_row == Some(Row::Effort), cx));
        let settings: Vec<AnyElement> = self
            .settings(cx)
            .into_iter()
            .map(|setting| self.render_setting(setting, cx))
            .collect();
        let levels = self.render_levels(cx);

        let mut key_context = KeyContext::new_with_defaults();
        key_context.add(KEY_CONTEXT);
        match &self.levels {
            Some(OpenLevels { selected: Some(_) }) => key_context.add("effort_levels"),
            _ if selected_row == Some(Row::Effort) => key_context.add("effort_row"),
            _ => {}
        }

        div()
            .key_context(key_context)
            .relative()
            .on_mouse_down_out(cx.listener(Self::dismiss_on_mouse_down_out))
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::select_child))
            .on_action(cx.listener(Self::select_parent))
            .child(
                v_flex()
                    .debug_selector(|| "model-picker".into())
                    .w(rems(18.))
                    .elevation_3(cx)
                    .overflow_hidden()
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
                                    .max_h(rems(16.))
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
                    .children(manage_models)
                    .children(effort)
                    .children(settings),
            )
            // Beside the menu, to its left where there's room by the composer, along its
            // bottom.
            .when_some(levels, |this, levels| {
                this.child(
                    div()
                        .absolute()
                        .right_full()
                        .bottom_0()
                        .mr_1()
                        .child(levels),
                )
            })
    }
}
