//! Settings › Agents › an agent › Models (`design/model-visibility`, topics 1, 5, 7 and 8):
//! every model the agent offers, starred for the picker's Favorites and switched off to leave
//! it out of the pickers, in t3code's groups (Favorites, All, Hidden from picker), each dragged
//! by its grip within its group. Shared's choices are the app's, which every machine gets; an
//! account that sets its own keeps them on its machine.

use std::rc::Rc;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::accounts::{AccountId, AgentAccounts};
use agentz_protocol::agents::{AgentId, AgentSettings};
use agentz_protocol::models::ModelPreferences;
use agentz_protocol::thread::ConnectionStatus;
use gpui::{AnyElement, App, Context, Entity, Window};
use ui::{Switch, Tooltip, prelude::*};

use super::{
    AccountView, AgentPanel, ContentRow, SettingsPage, account_entry, render_row, render_section,
    skills, with_offers,
};
use crate::app_settings::AppSettingsStore;
use crate::model_picker::{model_and_effort, select_choices};
use crate::server_client::ServerClient;

const TITLE: &str = "Models";

/// From this many models up, a field filters them, as t3code's list has from nine.
const FILTER_FROM: usize = 9;

/// A model of the agent's, as its model setting names it.
#[derive(Clone)]
struct OfferedModel {
    id: String,
    name: SharedString,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModelGroup {
    Favorites,
    All,
    Hidden,
}

impl ModelGroup {
    fn of(model: &str, preferences: &ModelPreferences) -> Self {
        if preferences.is_favorite(model) {
            Self::Favorites
        } else if preferences.is_shown(model) {
            Self::All
        } else {
            Self::Hidden
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Favorites => "Favorites",
            Self::All => "All",
            Self::Hidden => "Hidden from picker",
        }
    }
}

/// A model's row on its way to another place in its group.
#[derive(Clone)]
struct DraggedModel {
    group: ModelGroup,
    id: String,
    name: SharedString,
}

impl Render for DraggedModel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .child(Label::new(self.name.clone()).size(LabelSize::Small))
    }
}

/// Where the tab's changes go: Shared's choices in the app's settings, or an account's own.
#[derive(Clone)]
struct ModelsTarget {
    app_settings: Entity<AppSettingsStore>,
    client: Entity<ServerClient>,
    agent_id: AgentId,
    view: AccountView,
}

impl ModelsTarget {
    fn change(&self, change: impl FnOnce(&mut ModelPreferences), cx: &mut App) {
        match self.view {
            AccountView::Shared => {
                let agent_id = self.agent_id.clone();
                self.app_settings.update(cx, |store, cx| {
                    store.update(
                        |settings| {
                            let preferences = settings
                                .model_preferences
                                .entry(agent_id.clone())
                                .or_default();
                            change(preferences);
                            if *preferences == ModelPreferences::default() {
                                settings.model_preferences.remove(&agent_id);
                            }
                        },
                        cx,
                    )
                });
            }
            AccountView::Account(account) => self.client.update(cx, |client, cx| {
                client.update_account_settings(
                    &self.agent_id,
                    account,
                    |settings| {
                        if let Some(models) = &mut settings.models {
                            change(models);
                        }
                    },
                    cx,
                )
            }),
        }
    }
}

/// The models the agent's model setting offers, in its order.
fn offered_models(settings: &AgentSettings) -> Vec<OfferedModel> {
    let (Some(model), _) = model_and_effort(&settings.known_config_options) else {
        return Vec::new();
    };
    let acp::SessionConfigKind::Select(select) = &model.kind else {
        return Vec::new();
    };
    select_choices(&select.options)
        .into_iter()
        .map(|(_, value, name)| OfferedModel {
            id: value.0.to_string(),
            name,
        })
        .collect()
}

fn count(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// "16 of 66 shown · 3 favorites": what Shared has, for an account that uses it.
fn shared_summary(models: &[OfferedModel], preferences: &ModelPreferences) -> String {
    let shown = models
        .iter()
        .filter(|model| preferences.is_shown(&model.id))
        .count();
    let favorites = models
        .iter()
        .filter(|model| preferences.is_favorite(&model.id))
        .count();
    let mut summary = format!("{shown} of {} shown", models.len());
    if favorites > 0 {
        summary.push_str(" · ");
        summary.push_str(&count(favorites, "favorite", "favorites"));
    }
    summary
}

/// "66 models · 3 favorites · 50 hidden", beside the list's heading, leaving out what's none.
fn list_counts(models: &[OfferedModel], preferences: &ModelPreferences) -> String {
    let favorites = models
        .iter()
        .filter(|model| preferences.is_favorite(&model.id))
        .count();
    let hidden = models
        .iter()
        .filter(|model| !preferences.is_shown(&model.id))
        .count();
    let mut counts = count(models.len(), "model", "models");
    if favorites > 0 {
        counts.push_str(" · ");
        counts.push_str(&count(favorites, "favorite", "favorites"));
    }
    if hidden > 0 {
        counts.push_str(&format!(" · {hidden} hidden"));
    }
    counts
}

/// Droid names the models it's retiring so.
fn is_deprecated(name: &str) -> bool {
    name.ends_with("[Deprecated]")
}

/// "Work sets its own.", beside Shared in the Account menu.
pub(super) fn models_own_note(
    client: &ServerClient,
    agent_id: &AgentId,
    accounts: &AgentAccounts,
) -> Option<String> {
    let names: Vec<String> = accounts
        .listed()
        .into_iter()
        .filter(|&account| client.account_settings(agent_id, account).models.is_some())
        .map(|account| account_entry(accounts, account).name.to_string())
        .collect();
    match names.len() {
        0 => None,
        1 => Some(format!("{} sets its own.", skills::join_with_and(names))),
        _ => Some(format!("{} set their own.", skills::join_with_and(names))),
    }
}

impl AgentPanel {
    /// What the Models tab shows: Shared while it's picked, or when the agent has one account,
    /// whose tab changes Shared's; else [`Self::settings_account`]'s.
    pub(super) fn models_view(&self, accounts: &AgentAccounts) -> AccountView {
        if self.picked_shared || accounts.listed().len() < 2 {
            AccountView::Shared
        } else {
            AccountView::Account(self.settings_account(accounts))
        }
    }
}

impl SettingsPage {
    /// The Models tab: on an account, "Use shared settings" first, and its own list once it
    /// sets its own; on Shared, Shared's list.
    pub(super) fn render_agent_models(&self, cx: &mut Context<Self>) -> Vec<ContentRow> {
        let Some(panel) = self.agent_panel() else {
            return Vec::new();
        };
        let agent_id = panel.agent_id.clone();
        let client = panel.client(cx);
        let accounts = client.read(cx).accounts(&agent_id);
        let view = panel.models_view(&accounts);
        let offering = match view {
            AccountView::Shared => AgentSettings::default(),
            AccountView::Account(account) => client.read(cx).account_settings(&agent_id, account),
        };
        let models = offered_models(&with_offers(
            offering,
            client.read(cx),
            &agent_id,
            &accounts,
        ));
        if models.is_empty() {
            let agent_name = panel.external.connection.read(cx).agent_name().clone();
            let account = match view {
                AccountView::Shared => accounts.new_thread_account(),
                AccountView::Account(account) => account,
            };
            let session = panel.session(account).unwrap_or(&panel.external);
            let connection = session.connection.read(cx);
            let message = match (connection.status(), connection.logged_in()) {
                (_, Some(false)) => format!("Log in to {agent_name} to see its models here."),
                (ConnectionStatus::Connecting, _) => format!("Loading {agent_name}'s models…"),
                _ => format!("{agent_name} doesn't offer a choice of models."),
            };
            let row = div()
                .px_4()
                .py_3()
                .child(Label::new(message).color(Color::Muted))
                .into_any_element();
            return vec![ContentRow::section(render_section(TITLE, vec![row], cx))];
        }
        let shared = self
            .app_settings
            .read(cx)
            .settings()
            .model_preferences
            .get(&agent_id)
            .cloned()
            .unwrap_or_default();
        let target = ModelsTarget {
            app_settings: self.app_settings.clone(),
            client: client.clone(),
            agent_id: agent_id.clone(),
            view,
        };
        let AccountView::Account(account) = view else {
            return self.render_models_list(&models, &shared, &target, cx);
        };
        let own = client.read(cx).account_settings(&agent_id, account).models;
        let use_shared = render_use_shared_models_row(
            &accounts,
            account,
            shared_summary(&models, &shared),
            &shared,
            own.is_none(),
            &target,
            cx,
        );
        let mut rows = vec![ContentRow::section(render_section(
            TITLE,
            vec![use_shared],
            cx,
        ))];
        if let Some(own) = own {
            rows.extend(self.render_models_list(&models, &own, &target, cx));
        }
        rows
    }

    /// The list's heading with its counts and Hide deprecated, the filter, then the rows in
    /// their groups, each a row of the page's list so a long list lays out only what's in view.
    fn render_models_list(
        &self,
        models: &[OfferedModel],
        preferences: &ModelPreferences,
        target: &ModelsTarget,
        cx: &mut Context<Self>,
    ) -> Vec<ContentRow> {
        let Some(panel) = self.agent_panel() else {
            return Vec::new();
        };
        let colors = cx.theme().colors().clone();
        let has_filter = models.len() >= FILTER_FROM;
        let query = if has_filter {
            panel.models_filter.read(cx).text().trim().to_lowercase()
        } else {
            String::new()
        };
        let is_filtering = !query.is_empty();
        let mut ordered = models.to_vec();
        preferences.sort(&mut ordered, |model| model.id.as_str());
        let order: Rc<Vec<String>> =
            Rc::new(ordered.iter().map(|model| model.id.clone()).collect());
        let matching: Vec<&OfferedModel> = ordered
            .iter()
            .filter(|model| {
                !is_filtering
                    || model.name.to_lowercase().contains(&query)
                    || model.id.to_lowercase().contains(&query)
            })
            .collect();

        let deprecated: Vec<String> = models
            .iter()
            .filter(|model| is_deprecated(&model.name))
            .map(|model| model.id.clone())
            .collect();
        let hide_deprecated = (!deprecated.is_empty()).then(|| {
            let all_hidden = deprecated.iter().all(|model| !preferences.is_shown(model));
            let target = target.clone();
            div()
                .debug_selector(|| "models-hide-deprecated".into())
                .child(
                    Button::new("models-hide-deprecated", "Hide deprecated")
                        .style(ButtonStyle::Subtle)
                        .label_size(LabelSize::Small)
                        .disabled(all_hidden)
                        .on_click(move |_, _, cx| {
                            target.change(
                                |preferences| {
                                    for model in &deprecated {
                                        preferences.set_shown(model, false);
                                    }
                                },
                                cx,
                            )
                        }),
                )
        });
        let counts = list_counts(models, preferences);
        let heading = h_flex()
            .min_h(px(24.))
            .justify_between()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .child(Label::new(TITLE).size(LabelSize::Small).color(Color::Muted))
                    .child(
                        div().debug_selector(|| "models-counts".into()).child(
                            Label::new(counts)
                                .size(LabelSize::XSmall)
                                .color(Color::Placeholder),
                        ),
                    ),
            )
            .children(hide_deprecated);
        let filter = has_filter.then(|| {
            h_flex()
                .debug_selector(|| "models-filter".into())
                .h(px(30.))
                .px_3()
                .gap_2()
                .rounded_md()
                .border_1()
                .border_color(colors.border)
                .bg(colors.editor_background)
                .child(
                    Icon::new(IconName::MagnifyingGlass)
                        .size(IconSize::Small)
                        .color(Color::Muted),
                )
                .child(div().flex_1().min_w_0().child(panel.models_filter.clone()))
        });
        let mut rows = vec![ContentRow::section(
            v_flex()
                .gap_2()
                .child(heading)
                .children(filter)
                .into_any_element(),
        )];

        // The card's pieces, each with the lines inside it; the card's own edges go on after.
        let mut pieces: Vec<AnyElement> = Vec::new();
        let has_favorites = matching
            .iter()
            .any(|model| ModelGroup::of(&model.id, preferences) == ModelGroup::Favorites);
        for group in [ModelGroup::Favorites, ModelGroup::All, ModelGroup::Hidden] {
            let in_group: Vec<&OfferedModel> = matching
                .iter()
                .copied()
                .filter(|model| ModelGroup::of(&model.id, preferences) == group)
                .collect();
            if in_group.is_empty() {
                continue;
            }
            let follows_group = !pieces.is_empty();
            // All is named only under Favorites, as t3code's list does.
            let label = (group != ModelGroup::All || has_favorites).then(|| group.label());
            if let Some(label) = label {
                let selector = format!("models-group-{}", label.to_lowercase().replace(' ', "-"));
                pieces.push(
                    div()
                        .debug_selector(move || selector)
                        .px_4()
                        .pt(px(10.))
                        .pb_1()
                        .when(follows_group, |piece| {
                            piece.border_t_1().border_color(colors.border_variant)
                        })
                        .child(
                            Label::new(label)
                                .size(LabelSize::XSmall)
                                .color(Color::Placeholder),
                        )
                        .into_any_element(),
                );
            }
            let count = in_group.len();
            for (index, model) in in_group.into_iter().enumerate() {
                let row = self.render_model_row(
                    model,
                    group,
                    preferences,
                    &order,
                    is_filtering,
                    target,
                    cx,
                );
                pieces.push(
                    div()
                        .when(index + 1 < count, |piece| {
                            piece.border_b_1().border_color(colors.border_variant)
                        })
                        .when(follows_group && label.is_none() && index == 0, |piece| {
                            piece.border_t_1().border_color(colors.border_variant)
                        })
                        .child(row)
                        .into_any_element(),
                );
            }
        }
        if pieces.is_empty() {
            pieces.push(
                div()
                    .debug_selector(|| "models-no-match".into())
                    .px_4()
                    .py_3()
                    .child(Label::new("No models match.").color(Color::Muted))
                    .into_any_element(),
            );
        }
        let last = pieces.len() - 1;
        rows.extend(pieces.into_iter().enumerate().map(|(index, piece)| {
            let card = div()
                .border_x_1()
                .border_color(colors.border)
                .bg(colors.panel_background)
                .when(index == 0, |card| card.border_t_1().rounded_t_lg())
                .when(index == last, |card| card.border_b_1().rounded_b_lg())
                .child(piece)
                .into_any_element();
            ContentRow::new(card, if index == 0 { rems(0.625) } else { rems(0.) })
        }));
        rows
    }

    /// A model's grip, star, name and switch.
    #[allow(clippy::too_many_arguments)]
    fn render_model_row(
        &self,
        model: &OfferedModel,
        group: ModelGroup,
        preferences: &ModelPreferences,
        order: &Rc<Vec<String>>,
        is_filtering: bool,
        target: &ModelsTarget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = model.id.clone();
        let is_shown = preferences.is_shown(&id);
        let is_favorite = preferences.is_favorite(&id);
        let drop_background = cx.theme().colors().drop_target_background;
        // While the list is filtered, rows between those shown would move unseen.
        let grip = div()
            .id(SharedString::from(format!("model-grip-{id}")))
            .debug_selector({
                let id = id.clone();
                move || format!("model-grip-{id}")
            })
            .flex()
            .items_center()
            .child(
                Icon::new(IconName::GripVertical)
                    .size(IconSize::Small)
                    .color(if is_filtering {
                        Color::Disabled
                    } else {
                        Color::Placeholder
                    }),
            )
            .when(!is_filtering, |grip| {
                grip.cursor_grab().on_drag(
                    DraggedModel {
                        group,
                        id: id.clone(),
                        name: model.name.clone(),
                    },
                    |dragged, _, _, cx| cx.new(|_| dragged.clone()),
                )
            });
        let star_selector = format!("model-star-{id}");
        let star = IconButton::new(
            SharedString::from(format!("model-star-{id}")),
            if is_favorite {
                IconName::StarFilled
            } else {
                IconName::Star
            },
        )
        .icon_size(IconSize::Small)
        .icon_color(if is_favorite {
            Color::Warning
        } else {
            Color::Muted
        })
        .tooltip(Tooltip::text(if is_favorite {
            "Remove from favorites"
        } else {
            "Add to favorites"
        }))
        .on_click({
            let (target, id) = (target.clone(), id.clone());
            move |_, _, cx| {
                target.change(
                    |preferences| preferences.set_favorite(&id, !is_favorite),
                    cx,
                )
            }
        });
        let switch = div()
            .id(SharedString::from(format!("model-shown-{id}-tooltip")))
            .debug_selector({
                let id = id.clone();
                move || format!("model-shown-{id}")
            })
            .tooltip(Tooltip::text(if is_shown {
                "Shown in picker"
            } else {
                "Hidden from picker"
            }))
            .child(
                Switch::new(
                    SharedString::from(format!("model-shown-{id}")),
                    is_shown.into(),
                )
                .on_click({
                    let (target, id) = (target.clone(), id.clone());
                    move |state, _, cx| {
                        let shown = *state == ToggleState::Selected;
                        target.change(|preferences| preferences.set_shown(&id, shown), cx)
                    }
                }),
            );
        let (drop_target, drop_order) = (target.clone(), order.clone());
        h_flex()
            .debug_selector({
                let id = id.clone();
                move || format!("model-row-{id}")
            })
            .h(px(38.))
            .pl(px(10.))
            .pr(px(14.))
            .gap_2()
            .drag_over::<DraggedModel>(move |style, dragged, _, _| {
                if dragged.group == group && !is_filtering {
                    style.bg(drop_background)
                } else {
                    style
                }
            })
            .on_drop(move |dragged: &DraggedModel, _, cx| {
                if dragged.group == group && dragged.id != id && !is_filtering {
                    drop_target.change(
                        |preferences| preferences.move_model(&drop_order, &dragged.id, &id),
                        cx,
                    )
                }
            })
            .child(grip)
            .child(div().debug_selector(move || star_selector).child(star))
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .when(!is_shown, |name| name.opacity(0.5))
                    .child(Label::new(model.name.clone()).truncate()),
            )
            .child(switch)
            .into_any_element()
    }
}

/// "Use shared settings", first on an account's Models tab: on, with what Shared has; off,
/// the account has a copy of Shared's to change, which goes once it's on again.
fn render_use_shared_models_row(
    accounts: &AgentAccounts,
    account: Option<AccountId>,
    summary: String,
    shared: &ModelPreferences,
    follows: bool,
    target: &ModelsTarget,
    cx: &App,
) -> AnyElement {
    let description: SharedString = if follows {
        summary.into()
    } else {
        format!("{} sets its own.", account_entry(accounts, account).name).into()
    };
    let (target, shared) = (target.clone(), shared.clone());
    render_row(
        "Use shared settings",
        description,
        div()
            .debug_selector(|| "use-shared-models".into())
            .child(Switch::new("use-shared-models", follows.into()).on_click(
                move |state, _, cx| {
                    let on = *state == ToggleState::Selected;
                    let shared = shared.clone();
                    target.client.update(cx, |client, cx| {
                        client.update_account_settings(
                            &target.agent_id,
                            account,
                            |settings| settings.models = (!on).then_some(shared),
                            cx,
                        )
                    });
                },
            ))
            .into_any_element(),
        cx,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn models(names: &[&str]) -> Vec<OfferedModel> {
        names
            .iter()
            .map(|name| OfferedModel {
                id: name.to_lowercase(),
                name: name.to_string().into(),
            })
            .collect()
    }

    #[test]
    fn counts_leave_out_what_is_none() {
        let models = models(&["Opus", "Sonnet", "Haiku"]);
        let mut preferences = ModelPreferences::default();
        assert_eq!(list_counts(&models, &preferences), "3 models");
        assert_eq!(shared_summary(&models, &preferences), "3 of 3 shown");
        preferences.set_favorite("opus", true);
        preferences.set_shown("haiku", false);
        preferences.set_shown("sonnet", false);
        assert_eq!(
            list_counts(&models, &preferences),
            "3 models · 1 favorite · 2 hidden"
        );
        assert_eq!(
            shared_summary(&models, &preferences),
            "1 of 3 shown · 1 favorite"
        );
    }
}
