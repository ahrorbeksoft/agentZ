//! Settings › Machines' cloud providers (design/cloud): a group per provider account, with its
//! account, usage and every machine it has (topics 1 C, 6 B, 10 B, 13 A), then the providers
//! that aren't set up, with how to set them up and Check Again (topic 2 A). agentZ signs no one
//! in: the user installs each CLI and signs in with it (topic 3's note).

use std::rc::Rc;
use std::time::Duration;

use agentz_protocol::Request;
use agentz_protocol::cloud::{CloudMachine, Lifecycle, Provider, ProviderState, ProviderStatus};
use gpui::{AnyElement, Context, FontWeight, SharedString, Window};
use ui::{
    ContextMenu, ContextMenuEntry, IconPosition, PopoverMenu, Switch, ToggleState, Tooltip,
    prelude::*,
};
use util::ResultExt as _;

use super::{
    SettingsPage, SettingsPageEvent, connection_line, on_page, render_rows,
    render_section_with_actions,
};
use crate::app_settings::{CloudLink, MachineProfile};
use crate::cloud_machines::{
    MainAction, balance_left, change_label, charges, lifecycle_label, machine_summary, main_action,
    other_changes, provider_icon, state_color, state_label, usage_summary,
};
use crate::confirm_dialog::{ConfirmCheckbox, ConfirmRequest};
use crate::controls::{copy_to_clipboard, icon_tile, status_badge};
use crate::machines::{MachineId, Machines};

/// How long a copy button says it copied.
const COPIED_FOR: Duration = Duration::from_secs(2);
/// A machine's and a provider's mark, as a machine's icon is drawn.
const MARK_SIZE: Pixels = px(28.);

impl SettingsPage {
    /// Each provider that's signed in or has saved machines as a group, then More Providers.
    pub(super) fn render_cloud_providers(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let cloud = Machines::local(cx).read(cx).cloud().clone();
        let profiles = self.app_settings.read(cx).settings().machines.clone();
        let mut sections = Vec::new();
        let mut not_set_up = Vec::new();
        for provider in Provider::ALL {
            let state = cloud.provider(provider);
            let saved: Vec<&MachineProfile> = profiles
                .iter()
                .filter(|profile| {
                    profile
                        .cloud
                        .as_ref()
                        .is_some_and(|link| link.provider == provider)
                })
                .collect();
            if state.is_some_and(|state| state.status.is_signed_in()) || !saved.is_empty() {
                sections.push(self.render_provider_group(provider, state, &saved, cx));
            } else if let Some(state) = state {
                not_set_up.push(state);
            }
        }
        if !not_set_up.is_empty() || cloud.checking {
            sections.push(self.render_more_providers(&not_set_up, cloud.checking, cx));
        }
        sections
    }

    /// The account's header over its machines: every one it has, and the saved ones it doesn't
    /// list anymore.
    fn render_provider_group(
        &self,
        provider: Provider,
        state: Option<&ProviderState>,
        saved: &[&MachineProfile],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let listed = state.map_or(&[][..], |state| state.machines.as_slice());
        let saved_as = |machine: &str| {
            saved.iter().copied().find(|profile| {
                profile
                    .cloud
                    .as_ref()
                    .is_some_and(|link| link.machine == machine)
            })
        };
        let mut rows: Vec<AnyElement> = listed
            .iter()
            .map(|machine| {
                self.render_cloud_row(provider, state, Some(machine), saved_as(&machine.id), cx)
            })
            .collect();
        for profile in saved {
            let is_listed = profile
                .cloud
                .as_ref()
                .is_some_and(|link| listed.iter().any(|machine| machine.id == link.machine));
            if !is_listed {
                rows.push(self.render_cloud_row(provider, state, None, Some(profile), cx));
            }
        }
        if rows.is_empty() {
            rows.push(
                h_flex()
                    .px_4()
                    .py_3()
                    .child(
                        Label::new("No machines in this account yet.")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .into_any_element(),
            );
        }
        let error = self.cloud_errors.get(&provider).cloned().or_else(|| {
            state
                .and_then(|state| state.error.as_ref())
                .map(|error| format!("Couldn't list its machines: {error}").into())
        });
        v_flex()
            .gap_2()
            .child(self.render_provider_header(provider, state, cx))
            .children(
                error.map(|error| Label::new(error).size(LabelSize::Small).color(Color::Error)),
            )
            .child(render_rows(rows, cx))
            .into_any_element()
    }

    /// The provider's mark and name, who its CLI is signed in as and the account's usage, its
    /// page (Add Credit, or Dashboard) and a menu.
    fn render_provider_header(
        &self,
        provider: Provider,
        state: Option<&ProviderState>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let description = provider.description();
        let slug = description.program;
        let (line, signed_out) = match state.map(|state| &state.status) {
            Some(ProviderStatus::SignedIn { account, .. }) => {
                let usage = state
                    .and_then(|state| state.usage.as_ref())
                    .map(usage_summary);
                let line = [Some(account.clone()), usage]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" · ");
                (Some(line), false)
            }
            Some(ProviderStatus::SignedOut { .. }) => (
                Some(format!(
                    "Signed out. Run `{}` in a terminal.",
                    description.sign_in
                )),
                true,
            ),
            Some(ProviderStatus::NotInstalled) => (
                Some(format!("The {} CLI isn't installed.", description.program)),
                false,
            ),
            None => (None, false),
        };
        let (link_label, url) = match description.add_credit {
            Some(url) => ("Add Credit", url),
            None => ("Dashboard", description.dashboard),
        };
        let account = signed_in_account(state);
        let asks = account.as_ref().is_none_or(|account| {
            !self
                .app_settings
                .read(cx)
                .settings()
                .charges_without_asking(provider, account)
        });
        let page = cx.weak_entity();
        let menu = PopoverMenu::new(SharedString::from(format!("cloud-provider-menu-{slug}")))
            .menu(move |window, cx| {
                let page = page.clone();
                let account = account.clone();
                Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                    if description.add_credit.is_some() {
                        menu = menu.item(
                            ContextMenuEntry::new("Open Dashboard")
                                .icon(IconName::ArrowUpRight)
                                .icon_color(Color::Muted)
                                .handler(move |_, cx| cx.open_url(description.dashboard)),
                        );
                    }
                    menu = menu.item(
                        ContextMenuEntry::new("Learn More")
                            .icon(IconName::ArrowUpRight)
                            .icon_color(Color::Muted)
                            .handler(move |_, cx| cx.open_url(description.learn_more)),
                    );
                    if let Some(account) = account.filter(|_| description.charge.is_some()) {
                        menu = menu.separator().toggleable_entry(
                            "Ask Before Charging",
                            asks,
                            IconPosition::End,
                            None,
                            on_page(&page, move |page, _, cx| {
                                page.set_asks_before_charging(provider, &account, !asks, cx)
                            }),
                        );
                    }
                    menu
                }))
            })
            .trigger_with_tooltip(
                IconButton::new(
                    SharedString::from(format!("cloud-provider-menu-trigger-{slug}")),
                    IconName::Ellipsis,
                )
                .icon_size(IconSize::Small)
                .icon_color(Color::Muted),
                Tooltip::text("More"),
            )
            .anchor(gpui::Anchor::TopRight)
            .offset(gpui::point(px(0.), px(4.)));
        let copy_sign_in = signed_out.then(|| {
            IconButton::new(
                SharedString::from(format!("cloud-copy-sign-in-{slug}")),
                if self.copied_command == Some(description.sign_in) {
                    IconName::Check
                } else {
                    IconName::Copy
                },
            )
            .icon_size(IconSize::XSmall)
            .icon_color(Color::Muted)
            .tooltip(Tooltip::text("Copy Command"))
            .on_click(cx.listener(move |this, _, _, cx| this.copy_command(description.sign_in, cx)))
        });
        h_flex()
            .debug_selector(|| format!("cloud-provider-{slug}"))
            .min_h(px(20.))
            .gap_2()
            .child(
                Icon::new(provider_icon(provider))
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Label::new(description.name)
                    .size(LabelSize::Small)
                    .weight(FontWeight::MEDIUM),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .children(line.map(|line| {
                        Label::new(line)
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate()
                    }))
                    .children(copy_sign_in),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .child(
                        Button::new(
                            SharedString::from(format!("cloud-provider-link-{slug}")),
                            link_label,
                        )
                        .style(ButtonStyle::Subtle)
                        .label_size(LabelSize::Small)
                        .color(Color::Muted)
                        .end_icon(
                            Icon::new(IconName::ArrowUpRight)
                                .size(IconSize::XSmall)
                                .color(Color::Muted),
                        )
                        .on_click(move |_, _, cx| cx.open_url(url)),
                    )
                    .child(menu),
            )
            .into_any_element()
    }

    /// t3code's `EnvironmentRow` for a provider's machine (topic 6 B): its provider's mark, its
    /// name over what it is and how its connection is doing, then a badge with its provider's
    /// state, the one change it most likely needs, a menu with the rest, and its switch. A
    /// machine not saved in agentZ is switched off; switching it on saves it (topic 10 B).
    fn render_cloud_row(
        &self,
        provider: Provider,
        state: Option<&ProviderState>,
        machine: Option<&CloudMachine>,
        profile: Option<&MachineProfile>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let description = provider.description();
        let Some(machine_id) = machine.map(|machine| machine.id.clone()).or_else(|| {
            profile
                .and_then(|profile| profile.cloud.as_ref())
                .map(|link| link.machine.clone())
        }) else {
            return gpui::Empty.into_any_element();
        };
        let id_suffix = format!("{}-{machine_id}", description.program);
        let element_id = |action: &str| SharedString::from(format!("cloud-{action}-{id_suffix}"));
        let machine_name = machine.map_or(machine_id.clone(), |machine| machine.name.clone());
        let label: SharedString = profile
            .map(MachineProfile::display_label)
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| machine_name.clone())
            .into();
        let client = profile.and_then(|profile| {
            self.machines
                .read(cx)
                .client(MachineId::Remote(profile.id), cx)
        });
        let connection = client.as_ref().map(|client| connection_line(client, cx));
        let changing = state
            .and_then(|state| {
                state
                    .changing
                    .iter()
                    .find(|changing| changing.machine == machine_id)
            })
            .map(|changing| changing.change);
        let can_change =
            changing.is_none() && state.is_some_and(|state| state.status.is_signed_in());

        let summary = match machine {
            Some(machine) => Some(machine_summary(machine)).filter(|summary| !summary.is_empty()),
            None => state
                .filter(|state| state.status.is_signed_in() && state.error.is_none())
                .map(|_| format!("{} doesn't list it anymore", description.name)),
        };
        let is_error = connection.as_ref().is_some_and(|line| line.is_error);
        let subtitle = [
            summary,
            connection.as_ref().and_then(|line| line.status.clone()),
            connection
                .as_ref()
                .and_then(|line| line.server_version.clone()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let hint = connection.and_then(|line| line.hint);
        let machine_error = machine.and_then(|machine| machine.error.clone());

        let badge = match (changing, machine) {
            (Some(change), _) => Some(status_badge(change_label(change), Color::Accent.color(cx))),
            (None, Some(machine)) => Some(status_badge(
                state_label(&machine.state).to_string(),
                state_color(&machine.state).color(cx),
            )),
            (None, None) => None,
        };
        let main_button = machine
            .filter(|_| can_change)
            .and_then(|machine| main_action(provider, &machine.state))
            .map(|action| match action {
                MainAction::Change(change) => {
                    let machine_id = machine_id.clone();
                    let label = label.clone();
                    Button::new(element_id("main"), lifecycle_label(change))
                        .style(ButtonStyle::Outlined)
                        .label_size(LabelSize::Small)
                        .when_some(charges(provider, change), |button, charge| {
                            button.tooltip(Tooltip::text(format!("It costs {charge}.")))
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.change_cloud_machine(
                                provider,
                                machine_id.clone(),
                                label.clone(),
                                change,
                                window,
                                cx,
                            )
                        }))
                }
                MainAction::AddCredit(url) => Button::new(element_id("add-credit"), "Add Credit")
                    .style(ButtonStyle::Outlined)
                    .label_size(LabelSize::Small)
                    .end_icon(
                        Icon::new(IconName::ArrowUpRight)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .on_click(move |_, _, cx| cx.open_url(url)),
            });
        let update_button = client
            .filter(|client| client.read(cx).is_online() && client.read(cx).is_outdated())
            .map(|client| {
                IconButton::new(element_id("update"), IconName::CircleArrowUp)
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Update Server"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.update_server(client.clone(), window, cx)
                    }))
            });

        let menu = {
            let page = cx.weak_entity();
            let changes = machine
                .filter(|_| can_change)
                .map(|machine| other_changes(provider, &machine.state))
                .unwrap_or_default();
            let ssh_command = match profile
                .map(|profile| profile.target.as_str())
                .filter(|target| !target.is_empty())
            {
                Some(target) => format!("ssh {target}"),
                None => format!("{} ssh {machine_name}", description.program),
            };
            let removable = profile.map(|profile| (profile.id, label.clone()));
            let machine_id = machine_id.clone();
            let label = label.clone();
            PopoverMenu::new(element_id("menu"))
                .menu(move |window, cx| {
                    let page = page.clone();
                    let changes = changes.clone();
                    let ssh_command = ssh_command.clone();
                    let removable = removable.clone();
                    let machine_id = machine_id.clone();
                    let label = label.clone();
                    Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                        let has_changes = !changes.is_empty();
                        for change in changes {
                            let machine_id = machine_id.clone();
                            let label = label.clone();
                            menu = menu.item(
                                ContextMenuEntry::new(lifecycle_label(change))
                                    .icon(lifecycle_icon(change))
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, move |page, window, cx| {
                                        page.change_cloud_machine(
                                            provider,
                                            machine_id.clone(),
                                            label.clone(),
                                            change,
                                            window,
                                            cx,
                                        )
                                    })),
                            );
                        }
                        if has_changes {
                            menu = menu.separator();
                        }
                        menu = menu.item(
                            ContextMenuEntry::new("Copy SSH Command")
                                .icon(IconName::Copy)
                                .icon_color(Color::Muted)
                                .handler(move |_, cx| copy_to_clipboard(&ssh_command, cx)),
                        );
                        if let Some((id, name)) = removable {
                            menu = menu.separator().item(
                                ContextMenuEntry::new("Remove from agentZ")
                                    .icon(IconName::Close)
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, move |page, window, cx| {
                                        page.confirm_remove_machine(id, name.clone(), window, cx)
                                    })),
                            );
                        }
                        menu
                    }))
                })
                .trigger_with_tooltip(
                    IconButton::new(element_id("menu-trigger"), IconName::Ellipsis)
                        .icon_size(IconSize::Small)
                        .icon_color(Color::Muted),
                    Tooltip::text("More"),
                )
                .anchor(gpui::Anchor::TopRight)
                .offset(gpui::point(px(0.), px(4.)))
        };

        let is_enabled = profile.is_some_and(|profile| profile.enabled);
        let switch = match profile {
            Some(profile) => {
                let id = profile.id;
                Switch::new(element_id("switch"), is_enabled.into()).on_click(cx.listener(
                    move |this, state: &ToggleState, _, cx| {
                        let enabled = *state == ToggleState::Selected;
                        this.update_machine(id, |profile| profile.enabled = enabled, cx)
                    },
                ))
            }
            None => {
                let link = CloudLink {
                    provider,
                    machine: machine_id,
                };
                let name = machine_name;
                Switch::new(element_id("switch"), ToggleState::Unselected).on_click(cx.listener(
                    move |this, state: &ToggleState, _, cx| {
                        if *state != ToggleState::Selected {
                            return;
                        }
                        let link = link.clone();
                        let name = name.clone();
                        this.app_settings.update(cx, |store, cx| {
                            store.update(
                                |settings| {
                                    settings.add_cloud_machine(name, link);
                                },
                                cx,
                            )
                        });
                    },
                ))
            }
        };
        let switch = div()
            .id(element_id("switch-tooltip"))
            .debug_selector(|| format!("cloud-switch-{id_suffix}"))
            .tooltip(Tooltip::text(if is_enabled {
                "Switch Off"
            } else {
                "Switch On"
            }))
            .child(switch);

        h_flex()
            .debug_selector(|| format!("cloud-row-{id_suffix}"))
            .px_4()
            .py_2p5()
            .gap_3()
            .when(!is_enabled, |row| row.opacity(0.6))
            .child(icon_tile(
                Icon::new(provider_icon(provider)).color(Color::Muted),
                MARK_SIZE,
                cx,
            ))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(Label::new(label).truncate())
                    .when(!subtitle.is_empty(), |column| {
                        column.child(
                            Label::new(subtitle)
                                .size(LabelSize::Small)
                                .color(if is_error { Color::Error } else { Color::Muted })
                                .truncate(),
                        )
                    })
                    .children(
                        machine_error.map(|error| {
                            Label::new(error).size(LabelSize::Small).color(Color::Error)
                        }),
                    )
                    .children(
                        hint.map(|hint| {
                            Label::new(hint).size(LabelSize::Small).color(Color::Muted)
                        }),
                    ),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .children(badge.map(|badge| div().mr_1().child(badge)))
                    .children(main_button.map(|button| {
                        div()
                            .debug_selector(|| format!("cloud-main-{id_suffix}"))
                            .child(button)
                    }))
                    .children(update_button)
                    .child(menu)
                    .child(switch),
            )
            .into_any_element()
    }

    /// Providers whose CLI isn't installed or isn't signed in, with how to set each up (topic
    /// 2 A), and Check Again for one set up meanwhile.
    fn render_more_providers(
        &self,
        states: &[&ProviderState],
        checking: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut rows: Vec<AnyElement> = states
            .iter()
            .map(|state| self.render_provider_setup_row(state, cx))
            .collect();
        if rows.is_empty() {
            rows.push(
                h_flex()
                    .px_4()
                    .py_3()
                    .child(
                        Label::new("Looking for the providers’ CLIs…")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .into_any_element(),
            );
        }
        let check_again = Button::new(
            "cloud-check-again",
            if checking {
                "Checking…"
            } else {
                "Check Again"
            },
        )
        .style(ButtonStyle::Subtle)
        .label_size(LabelSize::Small)
        .color(Color::Muted)
        .start_icon(
            Icon::new(IconName::RotateCw)
                .size(IconSize::XSmall)
                .color(Color::Muted),
        )
        .disabled(checking)
        .tooltip(Tooltip::text("Look for the providers’ CLIs again"))
        .on_click(cx.listener(|this, _, _, cx| this.check_cloud(cx)));
        let check_again = div()
            .debug_selector(|| "cloud-check-again".into())
            .child(check_again);
        render_section_with_actions("More Providers", rows, check_again.into_any_element(), cx)
    }

    fn render_provider_setup_row(
        &self,
        state: &ProviderState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let provider = state.provider;
        let description = provider.description();
        let slug = description.program;
        let (tag, tag_color, line, copy_label, command) = match &state.status {
            ProviderStatus::SignedOut { .. } => (
                "Signed out",
                Color::Warning,
                format!(
                    "Its CLI is installed. Run `{}` in a terminal to sign in.",
                    description.sign_in
                ),
                "Copy Command",
                description.sign_in,
            ),
            _ => (
                "Not installed",
                Color::Muted,
                format!("{} {}.", description.pitch, description.price),
                "Copy Install Command",
                description.install,
            ),
        };
        let colors = cx.theme().colors();
        let copied = self.copied_command == Some(command);
        let code = description.code.as_ref().map(|code| {
            let code_text = code.code;
            div()
                .id(SharedString::from(format!("cloud-code-{slug}")))
                .px(px(7.))
                .rounded(px(5.))
                .border_1()
                .border_dashed()
                .border_color(Color::Success.color(cx))
                .cursor_pointer()
                .tooltip(Tooltip::text("Copy Code"))
                .on_click(cx.listener(move |this, _, _, cx| this.copy_command(code_text, cx)))
                .child(
                    Label::new(code.text)
                        .size(LabelSize::XSmall)
                        .color(Color::Success),
                )
        });
        h_flex()
            .debug_selector(|| format!("cloud-setup-{slug}"))
            .px_4()
            .py_3()
            .gap_3()
            .items_start()
            .child(icon_tile(
                Icon::new(provider_icon(provider)).color(Color::Muted),
                MARK_SIZE,
                cx,
            ))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Label::new(description.name))
                            .child(
                                div()
                                    .px(px(6.))
                                    .rounded(px(4.))
                                    .bg(colors.element_hover)
                                    .child(
                                        Label::new(tag).size(LabelSize::XSmall).color(tag_color),
                                    ),
                            )
                            .children(code),
                    )
                    .child(Label::new(line).size(LabelSize::Small).color(Color::Muted)),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap_1()
                    .child(
                        Button::new(
                            SharedString::from(format!("cloud-copy-{slug}")),
                            if copied { "Copied" } else { copy_label },
                        )
                        .style(ButtonStyle::Outlined)
                        .label_size(LabelSize::Small)
                        .start_icon(
                            Icon::new(if copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                        )
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.copy_command(command, cx)),
                        ),
                    )
                    .child(
                        Button::new(
                            SharedString::from(format!("cloud-learn-more-{slug}")),
                            "Learn More",
                        )
                        .style(ButtonStyle::Subtle)
                        .label_size(LabelSize::Small)
                        .color(Color::Accent)
                        .end_icon(
                            Icon::new(IconName::ArrowUpRight)
                                .size(IconSize::XSmall)
                                .color(Color::Accent),
                        )
                        .on_click(move |_, _, cx| cx.open_url(description.learn_more)),
                    ),
            )
            .into_any_element()
    }

    /// Looks for the providers' CLIs again now, on this Mac's server.
    pub(super) fn check_cloud(&mut self, cx: &mut Context<Self>) {
        Machines::local(cx).read(cx).send(Request::CheckCloud, cx);
    }

    fn copy_command(&mut self, command: &'static str, cx: &mut Context<Self>) {
        copy_to_clipboard(command, cx);
        self.copied_command = Some(command);
        self.copied_reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            this.update(cx, |this, cx| {
                if this.copied_command == Some(command) {
                    this.copied_command = None;
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Changes the machine through its provider's CLI, asking first when that starts a charge,
    /// until "Don't ask again" for its account (topic 5 C).
    fn change_cloud_machine(
        &mut self,
        provider: Provider,
        machine: String,
        name: SharedString,
        change: Lifecycle,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = Machines::local(cx)
            .read(cx)
            .cloud()
            .provider(provider)
            .cloned();
        let account = signed_in_account(state.as_ref());
        let asks = account.as_ref().is_none_or(|account| {
            !self
                .app_settings
                .read(cx)
                .settings()
                .charges_without_asking(provider, account)
        });
        let Some(charge) = charges(provider, change).filter(|_| asks) else {
            self.send_cloud_change(provider, machine, change, cx);
            return;
        };
        let balance = state
            .as_ref()
            .and_then(|state| state.usage.as_ref())
            .and_then(balance_left)
            .map(|left| format!(" ({left})"))
            .unwrap_or_default();
        let verb = lifecycle_label(change);
        let page = cx.weak_entity();
        let app_settings = self.app_settings.clone();
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest {
            icon: IconName::Coins,
            title: format!("{verb} {name}?").into(),
            message: format!("It costs {charge}{balance}. Stopping it stops the charge.").into(),
            confirm_label: verb.into(),
            on_confirm: Rc::new(move |_, cx| {
                page.update(cx, |page, cx| {
                    page.send_cloud_change(provider, machine.clone(), change, cx)
                })
                .log_err();
            }),
            checkbox: account.map(|account| ConfirmCheckbox {
                label: "Don’t ask again for this account".into(),
                on_confirm_ticked: Rc::new(move |cx| {
                    app_settings.update(cx, |store, cx| {
                        store.update(
                            |settings| settings.set_asks_before_charging(provider, &account, false),
                            cx,
                        )
                    })
                }),
            }),
            is_charge: true,
        }));
    }

    fn send_cloud_change(
        &mut self,
        provider: Provider,
        machine: String,
        change: Lifecycle,
        cx: &mut Context<Self>,
    ) {
        self.cloud_errors.remove(&provider);
        let request = Machines::local(cx)
            .read(cx)
            .request(Request::ChangeCloudMachine {
                provider,
                machine,
                change,
            });
        cx.spawn(async move |this, cx| {
            if let Err(error) = request.await {
                this.update(cx, |this, cx| {
                    this.cloud_errors
                        .insert(provider, format!("{error:#}").into());
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
    }

    fn set_asks_before_charging(
        &mut self,
        provider: Provider,
        account: &str,
        asks: bool,
        cx: &mut Context<Self>,
    ) {
        self.app_settings.update(cx, |store, cx| {
            store.update(
                |settings| settings.set_asks_before_charging(provider, account, asks),
                cx,
            )
        });
    }
}

fn signed_in_account(state: Option<&ProviderState>) -> Option<String> {
    match &state?.status {
        ProviderStatus::SignedIn { account, .. } => Some(account.clone()),
        _ => None,
    }
}

fn lifecycle_icon(change: Lifecycle) -> IconName {
    match change {
        Lifecycle::Start | Lifecycle::Resume => IconName::PlayFilled,
        Lifecycle::Stop => IconName::Stop,
        Lifecycle::Restart => IconName::RotateCw,
        Lifecycle::Pause => IconName::DebugPause,
        Lifecycle::Delete => IconName::Trash,
    }
}
