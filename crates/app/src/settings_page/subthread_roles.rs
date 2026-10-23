//! Settings › Subthreads' Roles (`design/settings-2` topic 4 D): a row per role, its picks as
//! chips in the order they were added, dragged to reorder, and + to add one. A chip's menu has
//! the agent, which of its accounts (All, or some), the model, the effort and Remove. A switch
//! over the roles runs each task on the account with the most usage to spare.

use std::rc::Rc;

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::accounts::{AccountId, AgentAccounts};
use agentz_protocol::agents::{AgentId, InstallState};
use agentz_protocol::models::ModelPreferences;
use agentz_protocol::subthread_roles::{RolePick, SubthreadRoles, TaskRole};
use gpui::{AnyElement, App, Context, Entity, Window};
use ui::{
    ButtonLike, ContextMenu, ContextMenuEntry, IconPosition, PopoverMenu, Switch, prelude::*,
};

use super::{
    SettingsPage, account_entries, account_entry, account_selector, render_account_entry,
    render_row, render_section_with_note, select_choices,
};
use crate::agent_icons::agent_icon;
use crate::app_settings::AppSettingsStore;
use crate::machines::Machines;
use crate::model_picker::followed_preferences;

const ROLE_EMPTY: &str = "None: the agent that delegates chooses.";

fn role_name(role: TaskRole) -> &'static str {
    match role {
        TaskRole::Researcher => "Researcher",
        TaskRole::Reviewer => "Reviewer",
        TaskRole::Worker => "Worker",
        TaskRole::Other => "Other",
    }
}

/// An agent a pick may name: one installed on some machine.
#[derive(Clone)]
struct PickAgent {
    id: AgentId,
    name: SharedString,
}

/// What a pick's agent offers: This Mac's accounts, and the models and efforts of the account
/// it would use, or another machine's where This Mac hasn't heard them.
struct PickChoices {
    accounts: AgentAccounts,
    /// Each choice's name and value.
    models: Vec<(SharedString, String)>,
    /// The choices the account's pickers follow, for the Model menu.
    model_preferences: ModelPreferences,
    efforts: Vec<(SharedString, String)>,
}

impl PickChoices {
    fn of(pick: &RolePick, machines: &Entity<Machines>, cx: &App) -> Self {
        let local = Machines::local(cx);
        let local = local.read(cx);
        let accounts = local.accounts(&pick.agent);
        let account = pick
            .accounts
            .as_ref()
            .and_then(|chosen| chosen.first().copied())
            .unwrap_or_else(|| accounts.new_thread_account());
        let mut options = local
            .account_settings(&pick.agent, account)
            .known_config_options;
        if options.is_empty() {
            options = machines
                .read(cx)
                .clients()
                .iter()
                .map(|client| {
                    client
                        .read(cx)
                        .agent_settings(&pick.agent.0)
                        .known_config_options
                })
                .find(|options| !options.is_empty())
                .unwrap_or_default();
        }
        let model_preferences = followed_preferences(local, &pick.agent, account, cx);
        Self {
            accounts,
            model_preferences,
            models: choices_of(&options, acp::SessionConfigOptionCategory::Model),
            efforts: choices_of(&options, acp::SessionConfigOptionCategory::ThoughtLevel),
        }
    }
}

impl PickChoices {
    /// The Model menu's models: those the pickers show, in the user's order, and the pick's
    /// own even when it's hidden (`design/model-visibility`, topic 15 B).
    fn shown_models(&self, current: Option<&str>) -> Vec<(SharedString, String)> {
        let mut models: Vec<(SharedString, String)> = self
            .models
            .iter()
            .filter(|(_, value)| {
                self.model_preferences.is_shown(value) || Some(value.as_str()) == current
            })
            .cloned()
            .collect();
        self.model_preferences
            .sort(&mut models, |(_, value)| value.as_str());
        models
    }
}

/// The first selector of the category: each choice's name and value.
fn choices_of(
    options: &[acp::SessionConfigOption],
    category: acp::SessionConfigOptionCategory,
) -> Vec<(SharedString, String)> {
    options
        .iter()
        .find_map(|option| {
            if option.category.as_ref() != Some(&category) {
                return None;
            }
            let acp::SessionConfigKind::Select(select) = &option.kind else {
                return None;
            };
            Some(
                select_choices(select)
                    .into_iter()
                    .map(|(name, value)| (name, value.0.to_string()))
                    .collect(),
            )
        })
        .unwrap_or_default()
}

/// A choice's name, or its value as the pick keeps it when the agent hasn't offered it.
fn choice_name(choices: &[(SharedString, String)], value: &str) -> SharedString {
    choices
        .iter()
        .find(|(_, choice)| choice == value)
        .map_or_else(|| value.to_string().into(), |(name, _)| name.clone())
}

/// "Claude Agent (Work) · Sonnet 5.5 · High": the agent, the accounts when some are chosen, then
/// the model and effort, "Default" standing for one left to the agent.
fn chip_label(pick: &RolePick, agent_name: &str, choices: &PickChoices) -> String {
    let mut label = agent_name.to_string();
    if let Some(chosen) = &pick.accounts {
        match chosen.as_slice() {
            [account] => {
                let name = account_entry(&choices.accounts, *account).name;
                label.push_str(&format!(" ({name})"));
            }
            chosen => label.push_str(&format!(" ({} accounts)", chosen.len())),
        }
    }
    let model = pick
        .model
        .as_deref()
        .map(|value| choice_name(&choices.models, value));
    let effort = pick
        .effort
        .as_deref()
        .map(|value| choice_name(&choices.efforts, value));
    if model.is_some() || effort.is_some() {
        label.push_str(" · ");
        label.push_str(model.as_deref().unwrap_or("Default"));
    }
    if let Some(effort) = effort {
        label.push_str(" · ");
        label.push_str(&effort);
    }
    label
}

fn update_roles(
    app_settings: &Entity<AppSettingsStore>,
    change: impl FnOnce(&mut SubthreadRoles),
    cx: &mut App,
) {
    app_settings.update(cx, |store, cx| {
        store.update(|settings| change(&mut settings.subthread_roles), cx)
    });
}

fn update_pick(
    app_settings: &Entity<AppSettingsStore>,
    role: TaskRole,
    index: usize,
    change: impl FnOnce(&mut RolePick),
    cx: &mut App,
) {
    update_roles(
        app_settings,
        |roles| {
            if let Some(pick) = roles.picks_mut(role).get_mut(index) {
                change(pick);
            }
        },
        cx,
    );
}

/// Turns one of the agent's accounts on or off for the pick, keeping at least one. Once every
/// listed account is on, it's All again, which takes accounts added later too.
fn toggle_account(pick: &mut RolePick, account: Option<AccountId>, listed: &[Option<AccountId>]) {
    let mut chosen = pick.accounts.clone().unwrap_or_else(|| listed.to_vec());
    if chosen.contains(&account) {
        if chosen.len() == 1 {
            return;
        }
        chosen.retain(|other| *other != account);
    } else {
        chosen.push(account);
    }
    let chosen: Vec<Option<AccountId>> = listed
        .iter()
        .copied()
        .filter(|account| chosen.contains(account))
        .collect();
    pick.accounts = (chosen.len() < listed.len()).then_some(chosen);
}

fn agent_icon_element(agent: &AgentId, cx: &App) -> Icon {
    match agent_icon(agent, cx) {
        Some(markup) => Icon::from_svg_markup(markup),
        None => Icon::new(IconName::Sparkle),
    }
}

/// An agent in a menu: its icon and name, and a check when it's the pick's.
fn render_agent_entry(agent: &PickAgent, is_current: bool, cx: &App) -> AnyElement {
    let selector = format!("role-agent-{}", agent.id.0);
    h_flex()
        .debug_selector(move || selector)
        .w_full()
        .gap_1p5()
        .child(
            agent_icon_element(&agent.id, cx)
                .size(IconSize::Small)
                .color(Color::Muted),
        )
        .child(Label::new(agent.name.clone()))
        .child(div().flex_1().min_w(px(16.)))
        .when(is_current, |row| {
            row.child(
                Icon::new(IconName::Check)
                    .size(IconSize::Small)
                    .color(Color::Accent),
            )
        })
        .into_any_element()
}

/// A chip on its way to another place in its role's list.
#[derive(Clone)]
struct DraggedPick {
    role: TaskRole,
    index: usize,
    label: SharedString,
}

impl Render for DraggedPick {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .child(Label::new(self.label.clone()).size(LabelSize::Small))
    }
}

impl SettingsPage {
    /// The roles, after the switch that picks by usage.
    pub(super) fn render_roles_section(&self, cx: &mut Context<Self>) -> AnyElement {
        let roles = self
            .app_settings
            .read(cx)
            .settings()
            .subthread_roles
            .clone();
        let agents = Rc::new(self.pick_agents(cx));
        let mut rows = vec![self.render_by_usage_row(roles.by_usage, cx)];
        rows.extend(
            TaskRole::ALL
                .into_iter()
                .map(|role| self.render_role_row(role, roles.picks(role), &agents, cx)),
        );
        let mut note = "Drag a pick to change the order.".to_string();
        if self.machines.read(cx).has_remotes() {
            note.push_str(
                " The accounts chosen are This Mac's; on other machines, a pick uses all of its \
                 agent's accounts there.",
            );
        }
        render_section_with_note("Roles", rows, note, cx)
    }

    fn render_by_usage_row(&self, by_usage: bool, cx: &mut Context<Self>) -> AnyElement {
        let app_settings = self.app_settings.clone();
        render_row(
            "Choose by usage",
            "Runs each task on whichever of its role's agents and accounts has the most usage \
             to spare. Off, they're tried in the order they were added, and the first with \
             usage left runs it.",
            div()
                .debug_selector(|| "roles-by-usage".to_string())
                .child(Switch::new("roles-by-usage", by_usage.into()).on_click(
                    move |state, _, cx| {
                        let by_usage = *state == ToggleState::Selected;
                        update_roles(&app_settings, |roles| roles.by_usage = by_usage, cx);
                    },
                ))
                .into_any_element(),
            cx,
        )
    }

    /// The agents installed on any machine, by name.
    fn pick_agents(&self, cx: &App) -> Vec<PickAgent> {
        let mut agents: Vec<PickAgent> = Vec::new();
        for client in self.machines.read(cx).clients() {
            for agent in client.read(cx).registry().read(cx).agents() {
                if matches!(agent.install_state, InstallState::Installed { .. })
                    && !agents.iter().any(|other| other.id == *agent.id())
                {
                    agents.push(PickAgent {
                        id: agent.id().clone(),
                        name: agent.name().clone(),
                    });
                }
            }
        }
        agents.sort_by(|a, b| a.name.cmp(&b.name));
        agents
    }

    /// The role's name, then its picks as chips and the + chip.
    fn render_role_row(
        &self,
        role: TaskRole,
        picks: &[RolePick],
        agents: &Rc<Vec<PickAgent>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        h_flex()
            .debug_selector(move || format!("role-{}", role.id()))
            .px_4()
            .py(px(10.))
            .gap_3()
            .items_start()
            .child(
                div()
                    .flex_none()
                    .w(px(86.))
                    .pt(px(3.))
                    .child(Label::new(role_name(role))),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .flex_wrap()
                    .gap_1p5()
                    .when(picks.is_empty(), |chips| {
                        chips.child(
                            div().pt(px(3.)).child(
                                Label::new(ROLE_EMPTY)
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            ),
                        )
                    })
                    .children(
                        picks.iter().enumerate().map(|(index, pick)| {
                            self.render_pick_chip(role, index, pick, agents, cx)
                        }),
                    )
                    .child(self.render_add_pick(role, agents)),
            )
            .into_any_element()
    }

    /// "1 Codex · GPT-5.5 · High", opening the pick's menu, and dragged to another place.
    fn render_pick_chip(
        &self,
        role: TaskRole,
        index: usize,
        pick: &RolePick,
        agents: &Rc<Vec<PickAgent>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let choices = PickChoices::of(pick, &self.machines, cx);
        let agent_name = agents
            .iter()
            .find(|agent| agent.id == pick.agent)
            .map_or_else(|| pick.agent.0.to_string(), |agent| agent.name.to_string());
        let label: SharedString = chip_label(pick, &agent_name, &choices).into();
        let selector = format!("role-pick-{}-{index}", role.id());
        let trigger = ButtonLike::new(SharedString::from(selector.clone()))
            .style(ButtonStyle::Outlined)
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Label::new((index + 1).to_string())
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    )
                    .child(
                        agent_icon_element(&pick.agent, cx)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(Label::new(label.clone()).size(LabelSize::Small)),
            );
        let (app_settings, machines, agents) = (
            self.app_settings.clone(),
            self.machines.clone(),
            agents.clone(),
        );
        let menu = PopoverMenu::new(SharedString::from(format!("{selector}-menu")))
            .menu(move |window, cx| {
                Some(pick_menu(
                    app_settings.clone(),
                    machines.clone(),
                    agents.clone(),
                    role,
                    index,
                    window,
                    cx,
                ))
            })
            .trigger(trigger)
            .anchor(gpui::Anchor::TopLeft)
            .offset(gpui::point(px(0.), px(4.)));
        let drop_background = cx.theme().colors().drop_target_background;
        div()
            .id(SharedString::from(format!("{selector}-drag")))
            .debug_selector(move || selector)
            .rounded_md()
            .on_drag(DraggedPick { role, index, label }, |dragged, _, _, cx| {
                cx.new(|_| dragged.clone())
            })
            .drag_over::<DraggedPick>(move |style, dragged, _, _| {
                if dragged.role == role {
                    style.bg(drop_background)
                } else {
                    style
                }
            })
            .on_drop(cx.listener(move |this, dragged: &DraggedPick, _, cx| {
                this.move_pick(dragged, role, index, cx)
            }))
            .child(menu)
            .into_any_element()
    }

    /// The + chip: a menu of the agents installed on any machine, adding one as the role's
    /// last pick.
    fn render_add_pick(&self, role: TaskRole, agents: &Rc<Vec<PickAgent>>) -> AnyElement {
        let selector = format!("role-add-{}", role.id());
        let (app_settings, agents) = (self.app_settings.clone(), agents.clone());
        div()
            .debug_selector({
                let selector = selector.clone();
                move || selector
            })
            .child(
                PopoverMenu::new(SharedString::from(format!("{selector}-menu")))
                    .menu(move |window, cx| {
                        let (app_settings, agents) = (app_settings.clone(), agents.clone());
                        Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                            if agents.is_empty() {
                                return menu.label("No agent is installed.");
                            }
                            menu = menu.header("Installed on a machine");
                            for agent in agents.iter() {
                                let (render_agent, app_settings) =
                                    (agent.clone(), app_settings.clone());
                                let agent_id = agent.id.clone();
                                menu = menu.custom_entry(
                                    move |_, cx| render_agent_entry(&render_agent, false, cx),
                                    move |_, cx| {
                                        let agent = agent_id.clone();
                                        update_roles(
                                            &app_settings,
                                            |roles| {
                                                roles.picks_mut(role).push(RolePick {
                                                    agent,
                                                    accounts: None,
                                                    model: None,
                                                    effort: None,
                                                })
                                            },
                                            cx,
                                        );
                                    },
                                );
                            }
                            menu
                        }))
                    })
                    .trigger_with_tooltip(
                        ButtonLike::new(SharedString::from(selector))
                            .style(ButtonStyle::Outlined)
                            .child(
                                Icon::new(IconName::Plus)
                                    .size(IconSize::Small)
                                    .color(Color::Muted),
                            ),
                        ui::Tooltip::text("Add a Pick"),
                    )
                    .anchor(gpui::Anchor::TopLeft)
                    .offset(gpui::point(px(0.), px(4.))),
            )
            .into_any_element()
    }

    fn move_pick(&mut self, dragged: &DraggedPick, role: TaskRole, to: usize, cx: &mut App) {
        if dragged.role != role || dragged.index == to {
            return;
        }
        update_roles(
            &self.app_settings,
            |roles| {
                let picks = roles.picks_mut(role);
                if dragged.index < picks.len() {
                    let pick = picks.remove(dragged.index);
                    picks.insert(to.min(picks.len()), pick);
                }
            },
            cx,
        );
    }
}

/// A chip's menu: the agent, its accounts, the model and effort, and Remove. Rebuilt after each
/// click from the settings, so the accounts' checks stay open to change.
fn pick_menu(
    app_settings: Entity<AppSettingsStore>,
    machines: Entity<Machines>,
    agents: Rc<Vec<PickAgent>>,
    role: TaskRole,
    index: usize,
    window: &mut Window,
    cx: &mut App,
) -> Entity<ContextMenu> {
    ContextMenu::build_persistent(window, cx, move |mut menu, _, cx| {
        let Some(pick) = app_settings
            .read(cx)
            .settings()
            .subthread_roles
            .picks(role)
            .get(index)
            .cloned()
        else {
            return menu;
        };
        let choices = PickChoices::of(&pick, &machines, cx);
        let agent_name: SharedString = agents
            .iter()
            .find(|agent| agent.id == pick.agent)
            .map_or_else(
                || pick.agent.0.to_string().into(),
                |agent| agent.name.clone(),
            );

        menu = menu.header("Agent").submenu(agent_name, {
            let (app_settings, agents, current) =
                (app_settings.clone(), agents.clone(), pick.agent.clone());
            move |mut submenu, _, _| {
                for agent in agents.iter() {
                    let (render_agent, app_settings) = (agent.clone(), app_settings.clone());
                    let is_current = agent.id == current;
                    let agent_id = agent.id.clone();
                    submenu = submenu.custom_entry(
                        move |_, cx| render_agent_entry(&render_agent, is_current, cx),
                        move |_, cx| {
                            let agent = agent_id.clone();
                            // Accounts, models and efforts are each agent's own.
                            update_pick(
                                &app_settings,
                                role,
                                index,
                                |pick| {
                                    if pick.agent != agent {
                                        *pick = RolePick {
                                            agent,
                                            accounts: None,
                                            model: None,
                                            effort: None,
                                        };
                                    }
                                },
                                cx,
                            );
                        },
                    );
                }
                submenu
            }
        });

        let listed = choices.accounts.listed();
        if listed.len() > 1 {
            menu = menu.header("Accounts").toggleable_entry(
                "All",
                pick.accounts.is_none(),
                IconPosition::End,
                None,
                {
                    let app_settings = app_settings.clone();
                    move |_, cx| {
                        update_pick(&app_settings, role, index, |pick| pick.accounts = None, cx)
                    }
                },
            );
            for entry in account_entries(&choices.accounts) {
                let account = entry.account;
                let is_on = pick
                    .accounts
                    .as_ref()
                    .is_none_or(|chosen| chosen.contains(&account));
                let (app_settings, listed) = (app_settings.clone(), listed.clone());
                let selector = format!("role-pick-account-{}", account_selector(account));
                menu = menu.custom_entry(
                    move |_, cx| {
                        let selector = selector.clone();
                        div()
                            .w_full()
                            .debug_selector(move || selector)
                            .child(render_account_entry(&entry, is_on, cx))
                            .into_any_element()
                    },
                    move |_, cx| {
                        update_pick(
                            &app_settings,
                            role,
                            index,
                            |pick| toggle_account(pick, account, &listed),
                            cx,
                        )
                    },
                );
            }
        }

        menu = menu.header("Model").submenu(
            pick.model.as_deref().map_or_else(
                || "Default".into(),
                |value| choice_name(&choices.models, value),
            ),
            choice_submenu(
                app_settings.clone(),
                role,
                index,
                choices.shown_models(pick.model.as_deref()),
                pick.model.clone(),
                |pick, model| pick.model = model,
            ),
        );
        if !choices.efforts.is_empty() || pick.effort.is_some() {
            menu = menu.header("Effort").submenu(
                pick.effort.as_deref().map_or_else(
                    || "Default".into(),
                    |value| choice_name(&choices.efforts, value),
                ),
                choice_submenu(
                    app_settings.clone(),
                    role,
                    index,
                    choices.efforts,
                    pick.effort,
                    |pick, effort| pick.effort = effort,
                ),
            );
        }

        let app_settings = app_settings.clone();
        menu.separator().item(
            ContextMenuEntry::new("Remove")
                .icon(IconName::Trash)
                .icon_color(Color::Muted)
                .handler(move |window, cx| {
                    update_roles(
                        &app_settings,
                        |roles| {
                            let picks = roles.picks_mut(role);
                            if index < picks.len() {
                                picks.remove(index);
                            }
                        },
                        cx,
                    );
                    // The menu stays open after a click; this one's pick is gone.
                    window.dispatch_action(Box::new(menu::Cancel), cx);
                }),
        )
    })
}

/// The model's or effort's choices: Default (the account's or model's own), then the agent's.
fn choice_submenu(
    app_settings: Entity<AppSettingsStore>,
    role: TaskRole,
    index: usize,
    choices: Vec<(SharedString, String)>,
    current: Option<String>,
    set: fn(&mut RolePick, Option<String>),
) -> impl Fn(ContextMenu, &mut Window, &mut Context<ContextMenu>) -> ContextMenu + 'static {
    move |mut submenu, _, _| {
        let entries = std::iter::once((SharedString::from("Default"), None)).chain(
            choices
                .iter()
                .map(|(name, value)| (name.clone(), Some(value.clone()))),
        );
        for (name, value) in entries {
            let app_settings = app_settings.clone();
            let is_current = value == current;
            submenu = submenu.toggleable_entry(
                name,
                is_current,
                IconPosition::End,
                None,
                move |_, cx| {
                    let value = value.clone();
                    update_pick(&app_settings, role, index, |pick| set(pick, value), cx)
                },
            );
        }
        submenu
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use agentz_protocol::Request;
    use agentz_protocol::accounts::AccountStatus;
    use agentz_protocol::agents::{AgentSettings, RegistrySnapshot};
    use gpui::TestAppContext;

    use super::super::Section;
    use super::super::tests::{account, listing, machines_with_copies};
    use super::*;
    use crate::server_client::ServerClient;

    fn roles(page: &Entity<SettingsPage>, cx: &mut gpui::VisualTestContext) -> SubthreadRoles {
        page.read_with(cx, |page, cx| {
            page.app_settings
                .read(cx)
                .settings()
                .subthread_roles
                .clone()
        })
    }

    fn last_sent(
        client: &Entity<ServerClient>,
        cx: &mut gpui::VisualTestContext,
    ) -> Option<SubthreadRoles> {
        client.read_with(cx, |client, _| {
            client
                .sent_for_test()
                .into_iter()
                .rev()
                .find_map(|request| match request {
                    Request::SetSubthreadRoles(roles) => Some(roles),
                    _ => None,
                })
        })
    }

    fn agents_of(picks: &[RolePick]) -> Vec<&str> {
        picks.iter().map(|pick| pick.agent.0.as_ref()).collect()
    }

    /// `design/settings-2` topic 4 D: + adds a pick as a chip, a chip's menu narrows it to some
    /// of This Mac's accounts (other machines are sent All), a chip dragged onto another takes
    /// its place, and the switch turns off choosing by usage.
    #[gpui::test]
    fn roles_take_their_picks_as_chips(cx: &mut TestAppContext) {
        let (local, devbox) = machines_with_copies(cx);
        let installed = || InstallState::Installed {
            version: "1.0.0".into(),
            update_available: false,
        };
        cx.update(|cx| {
            for client in [&local, &devbox] {
                client.update(cx, |client, cx| client.set_online_for_test(cx));
            }
            let registry = local.read(cx).registry().clone();
            registry.update(cx, |registry, cx| {
                registry.set_snapshot(
                    RegistrySnapshot {
                        agents: vec![
                            listing("claude", "Claude Agent", installed()),
                            listing("codex", "Codex", installed()),
                        ],
                        is_fetching: false,
                        fetch_error: None,
                    },
                    cx,
                )
            });
            local.update(cx, |client, cx| {
                client.set_accounts_for_test(
                    BTreeMap::from([(
                        AgentId::new("claude"),
                        AgentAccounts {
                            accounts: vec![account(
                                1,
                                Some("Work"),
                                Some(AccountStatus::default()),
                            )],
                            external_logged_in: Some(true),
                            last_id: 1,
                            ..AgentAccounts::default()
                        },
                    )]),
                    cx,
                );
                client.set_agent_settings_for_test(
                    BTreeMap::from([(AgentId::new("claude"), AgentSettings::default())]),
                    cx,
                );
            });
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        cx.simulate_resize(gpui::size(px(1100.), px(2000.)));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Subthreads, window, cx)
        });
        cx.run_until_parked();
        fn click(name: &'static str, cx: &mut gpui::VisualTestContext) {
            let bounds = cx
                .debug_bounds(name)
                .unwrap_or_else(|| panic!("{name} is shown"));
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        }

        for agent in ["role-agent-claude", "role-agent-codex"] {
            click("role-add-worker", cx);
            click(agent, cx);
        }
        assert_eq!(agents_of(&roles(&page, cx).worker), ["claude", "codex"]);
        assert!(roles(&page, cx).researcher.is_empty());

        click("role-pick-worker-0", cx);
        click("role-pick-account-external", cx);
        assert_eq!(
            roles(&page, cx).worker[0].accounts,
            Some(vec![Some(AccountId(1))])
        );
        // The menu stays open on its checks.
        assert!(cx.debug_bounds("role-pick-account-1").is_some());
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert_eq!(
            last_sent(&local, cx).map(|roles| roles.worker[0].accounts.clone()),
            Some(Some(vec![Some(AccountId(1))]))
        );
        assert_eq!(
            last_sent(&devbox, cx).map(|roles| roles.worker[0].accounts.clone()),
            Some(None),
            "accounts are This Mac's"
        );

        let first = cx
            .debug_bounds("role-pick-worker-0")
            .expect("the first chip");
        let second = cx
            .debug_bounds("role-pick-worker-1")
            .expect("the second chip");
        cx.simulate_mouse_down(
            second.center(),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_move(
            first.center(),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.simulate_mouse_up(
            first.center(),
            gpui::MouseButton::Left,
            gpui::Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(agents_of(&roles(&page, cx).worker), ["codex", "claude"]);

        assert!(roles(&page, cx).by_usage);
        click("roles-by-usage", cx);
        assert!(!roles(&page, cx).by_usage);
        assert_eq!(
            last_sent(&devbox, cx).map(|roles| roles.by_usage),
            Some(false)
        );
    }

    #[test]
    fn a_chip_names_the_accounts_model_and_effort() {
        let choices = PickChoices {
            accounts: AgentAccounts {
                accounts: vec![account(1, Some("Work"), None)],
                last_id: 1,
                ..AgentAccounts::default()
            },
            model_preferences: ModelPreferences::default(),
            models: vec![("Sonnet 5.5".into(), "sonnet".into())],
            efforts: vec![("High".into(), "high".into())],
        };
        let pick = |accounts, model: Option<&str>, effort: Option<&str>| RolePick {
            agent: AgentId::new("claude"),
            accounts,
            model: model.map(str::to_string),
            effort: effort.map(str::to_string),
        };
        assert_eq!(
            chip_label(&pick(None, None, None), "Claude Agent", &choices),
            "Claude Agent"
        );
        assert_eq!(
            chip_label(
                &pick(Some(vec![Some(AccountId(1))]), Some("sonnet"), Some("high")),
                "Claude Agent",
                &choices
            ),
            "Claude Agent (Work) · Sonnet 5.5 · High"
        );
        assert_eq!(
            chip_label(&pick(None, None, Some("max")), "Claude Agent", &choices),
            "Claude Agent · Default · max"
        );
    }

    #[test]
    fn an_account_turned_off_leaves_the_others_and_all_comes_back() {
        let listed = [None, Some(AccountId(1)), Some(AccountId(2))];
        let mut pick = RolePick {
            agent: AgentId::new("claude"),
            accounts: None,
            model: None,
            effort: None,
        };
        toggle_account(&mut pick, Some(AccountId(1)), &listed);
        assert_eq!(pick.accounts, Some(vec![None, Some(AccountId(2))]));
        toggle_account(&mut pick, None, &listed);
        assert_eq!(pick.accounts, Some(vec![Some(AccountId(2))]));
        // The last one stays.
        toggle_account(&mut pick, Some(AccountId(2)), &listed);
        assert_eq!(pick.accounts, Some(vec![Some(AccountId(2))]));
        toggle_account(&mut pick, None, &listed);
        toggle_account(&mut pick, Some(AccountId(1)), &listed);
        assert_eq!(pick.accounts, None, "every account is All again");
    }
}
