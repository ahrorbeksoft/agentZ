//! Settings › Machines' cloud providers (design/cloud): a group per provider account, with its
//! account, usage and every machine it has (topics 1 C, 6 B, 10 B, 13 A), then the providers
//! that aren't set up, with how to set them up and Check Again (topic 2 A). agentZ signs no one
//! in: the user installs each CLI and signs in with it (topic 3's note).

use std::rc::Rc;
use std::time::Duration;

use agentz_protocol::cloud::{
    Change, CloudMachine, CreateMachine, Lifecycle, MachineAction, Provider, ProviderState,
    ProviderStatus,
};
use agentz_protocol::{Request, Response};
use anyhow::anyhow;
use gpui::{AnyElement, Context, DismissEvent, FontWeight, SharedString, WeakEntity, Window};
use ui::{
    ContextMenu, ContextMenuEntry, IconPosition, PopoverMenu, Switch, ToggleState, Tooltip,
    prelude::*,
};
use util::ResultExt as _;

use super::cloud_machine_dialog::DialogKind;
use super::cloud_machine_page::MachineTab;
use super::new_cloud_machine::NewCloudMachine;
use super::{
    SettingsPage, SettingsPageEvent, connection_line, on_page, render_rows,
    render_section_with_actions,
};
use crate::app_settings::{CloudLink, MachineProfile};
use crate::cloud_machines::{
    MainAction, change_label, charge_message, charges, lifecycle_label, machine_summary,
    main_action, noun_title, other_changes, provider_icon, state_color, state_label, usage_summary,
};
use crate::confirm_dialog::{ConfirmCheckbox, ConfirmRequest};
use crate::controls::{copy_to_clipboard, icon_tile, status_badge};
use crate::machines::{MachineId, Machines};

/// How long a copy button says it copied.
pub(super) const COPIED_FOR: Duration = Duration::from_secs(2);
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
        // Machines being made are named until they're listed.
        let creating = state.map_or(&[][..], |state| state.changing.as_slice());
        for changing in creating {
            let is_listed = listed
                .iter()
                .any(|machine| machine.name == changing.machine);
            if changing.change == Change::Create && !is_listed {
                rows.push(render_creating_row(provider, &changing.machine, cx));
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
        let new_machine = signed_in_account(state).map(|_| {
            div()
                .debug_selector(|| format!("cloud-new-machine-{slug}"))
                .child(
                    Button::new(
                        SharedString::from(format!("cloud-new-machine-{slug}")),
                        format!("New {}", noun_title(provider)),
                    )
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::Small)
                    .color(Color::Muted)
                    .start_icon(
                        Icon::new(IconName::Plus)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_new_cloud_machine(provider, window, cx)
                    })),
                )
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
                    .children(new_machine)
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
            let removable = profile.map(|profile| {
                // Deleting it needs the provider to list it, by the name it has there.
                let deletable = machine
                    .filter(|_| can_change)
                    .map(|machine| (machine.id.clone(), machine.name.clone()));
                (profile.id, label.clone(), deletable)
            });
            let machine_id = machine_id.clone();
            let label = label.clone();
            let capabilities = &description.capabilities;
            let is_listed = machine.is_some();
            // Whether it's running, once it can be acted on.
            let extras = machine
                .filter(|_| can_change)
                .map(|machine| machine.state.is_running());
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
                        if capabilities.desktop && extras == Some(true) {
                            let machine_id = machine_id.clone();
                            menu = menu.item(
                                ContextMenuEntry::new("Open Desktop")
                                    .icon(IconName::Screen)
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, move |page, _, cx| {
                                        page.send_machine_action(
                                            provider,
                                            machine_id.clone(),
                                            MachineAction::DesktopLink { view_only: false },
                                            cx,
                                        )
                                    })),
                            );
                        }
                        menu = menu.item(
                            ContextMenuEntry::new("Copy SSH Command")
                                .icon(IconName::Copy)
                                .icon_color(Color::Muted)
                                .handler(move |_, cx| copy_to_clipboard(&ssh_command, cx)),
                        );
                        let dialogs = [
                            (
                                capabilities.snapshots,
                                "Save Snapshot…",
                                IconName::Archive,
                                DialogKind::SaveSnapshot,
                            ),
                            (
                                capabilities.public_ports,
                                "Publish Port…",
                                IconName::Public,
                                DialogKind::PublishPort,
                            ),
                        ];
                        for (has, title, icon, kind) in dialogs {
                            if !has || extras.is_none() {
                                continue;
                            }
                            let machine_id = machine_id.clone();
                            menu = menu.item(
                                ContextMenuEntry::new(title)
                                    .icon(icon)
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, move |page, window, cx| {
                                        page.open_machine_dialog(
                                            provider,
                                            machine_id.clone(),
                                            kind,
                                            window,
                                            cx,
                                        )
                                    })),
                            );
                        }
                        let pages = [
                            (
                                capabilities.ssh_keys,
                                "SSH Keys…",
                                IconName::Lock,
                                MachineTab::SshKeys,
                            ),
                            (
                                true,
                                "Machine Settings…",
                                IconName::Settings,
                                MachineTab::Overview,
                            ),
                        ];
                        for (has, title, icon, tab) in pages {
                            if !has || !is_listed {
                                continue;
                            }
                            // Its settings go with Remove….
                            if tab == MachineTab::Overview {
                                menu = menu.separator();
                            }
                            let machine_id = machine_id.clone();
                            menu = menu.item(
                                ContextMenuEntry::new(title)
                                    .icon(icon)
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, move |page, _, cx| {
                                        page.open_machine_page(
                                            provider,
                                            machine_id.clone(),
                                            tab,
                                            cx,
                                        )
                                    })),
                            );
                        }
                        if let Some((id, name, deletable)) = removable {
                            if !is_listed {
                                menu = menu.separator();
                            }
                            menu = menu.item(
                                ContextMenuEntry::new("Remove…")
                                    .icon(IconName::Trash)
                                    .icon_color(Color::Muted)
                                    .handler(on_page(&page, move |page, _, cx| {
                                        page.confirm_remove_cloud_machine(
                                            provider,
                                            id,
                                            name.clone(),
                                            deletable.clone(),
                                            cx,
                                        )
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

        let open_page = machine.map(|_| {
            let machine_id = machine_id.clone();
            cx.listener(move |this, _, _, cx| {
                this.open_machine_page(provider, machine_id.clone(), MachineTab::Overview, cx)
            })
        });
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
                let name = machine_name;
                Switch::new(element_id("switch"), ToggleState::Unselected).on_click(cx.listener(
                    move |this, state: &ToggleState, _, cx| {
                        if *state == ToggleState::Selected {
                            this.save_cloud_machine(provider, &machine_id, &name, cx);
                        }
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
            .child(
                // Its mark and name open its page (topic 7 A); its buttons stay theirs.
                h_flex()
                    .id(element_id("open"))
                    .debug_selector(|| format!("cloud-open-{id_suffix}"))
                    .flex_1()
                    .min_w_0()
                    .gap_3()
                    .when_some(open_page, |area, open_page| {
                        area.cursor_pointer().on_click(open_page)
                    })
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
                            .children(machine_error.map(|error| {
                                Label::new(error).size(LabelSize::Small).color(Color::Error)
                            }))
                            .children(hint.map(|hint| {
                                Label::new(hint).size(LabelSize::Small).color(Color::Muted)
                            })),
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
    pub(super) fn change_cloud_machine(
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
        let verb = lifecycle_label(change);
        let page = cx.weak_entity();
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest {
            icon: IconName::Coins,
            title: format!("{verb} {name}?").into(),
            message: charge_message(
                charge,
                state.as_ref().and_then(|state| state.usage.as_ref()),
            )
            .into(),
            confirm_label: verb.into(),
            on_confirm: Rc::new(move |_, cx| {
                page.update(cx, |page, cx| {
                    page.send_cloud_change(provider, machine.clone(), change, cx)
                })
                .log_err();
            }),
            checkbox: account.map(|account| self.dont_ask_again(provider, account)),
            is_charge: true,
        }));
    }

    /// "Don't ask again for this account", under a question about a charge.
    fn dont_ask_again(&self, provider: Provider, account: String) -> ConfirmCheckbox {
        let app_settings = self.app_settings.clone();
        ConfirmCheckbox {
            label: "Don’t ask again for this account".into(),
            type_to_confirm: None,
            on_confirm_ticked: Rc::new(move |cx| {
                app_settings.update(cx, |store, cx| {
                    store.update(
                        |settings| settings.set_asks_before_charging(provider, &account, false),
                        cx,
                    )
                })
            }),
        }
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

    /// Add Machine's menu (topic 4 C): an SSH machine, a new machine from each signed-in
    /// provider, and the machines its accounts have that agentZ hasn't saved.
    pub(super) fn render_add_machine_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let cloud = Machines::local(cx).read(cx).cloud().clone();
        let settings = self.app_settings.read(cx).settings();
        let signed_in: Vec<Provider> = Provider::ALL
            .into_iter()
            .filter(|provider| {
                cloud
                    .provider(*provider)
                    .is_some_and(|state| state.status.is_signed_in())
            })
            .collect();
        let existing: Vec<(Provider, CloudMachine)> = signed_in
            .iter()
            .filter_map(|provider| Some((*provider, cloud.provider(*provider)?)))
            .flat_map(|(provider, state)| {
                state
                    .machines
                    .iter()
                    .filter(move |machine| settings.cloud_machine(provider, &machine.id).is_none())
                    .map(move |machine| (provider, machine.clone()))
            })
            .collect();
        let page = cx.weak_entity();
        let menu = PopoverMenu::new("machines-add-menu")
            .menu(move |window, cx| {
                let page = page.clone();
                let signed_in = signed_in.clone();
                let existing = existing.clone();
                Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                    menu = menu.item(
                        ContextMenuEntry::new("SSH Machine…")
                            .icon(IconName::Server)
                            .icon_color(Color::Muted)
                            .handler(on_page(&page, |_, _, cx| {
                                cx.emit(SettingsPageEvent::EditMachine(None))
                            })),
                    );
                    if !signed_in.is_empty() {
                        menu = menu.separator().header("New");
                    }
                    for provider in signed_in {
                        let description = provider.description();
                        menu = menu.item(
                            ContextMenuEntry::new(format!(
                                "{} {}…",
                                description.name,
                                noun_title(provider)
                            ))
                            .icon(provider_icon(provider))
                            .icon_color(Color::Muted)
                            .handler(on_page(
                                &page,
                                move |page, window, cx| {
                                    page.open_new_cloud_machine(provider, window, cx)
                                },
                            )),
                        );
                    }
                    if !existing.is_empty() {
                        menu = menu.separator().header("Add existing");
                    }
                    for (provider, machine) in existing {
                        menu = menu.item(
                            ContextMenuEntry::new(format!(
                                "{}: {}",
                                provider.description().name,
                                machine.name
                            ))
                            .icon(IconName::Link)
                            .icon_color(Color::Muted)
                            .handler(on_page(
                                &page,
                                move |page, _, cx| {
                                    page.save_cloud_machine(
                                        provider,
                                        &machine.id,
                                        &machine.name,
                                        cx,
                                    )
                                },
                            )),
                        );
                    }
                    menu
                }))
            })
            .trigger(
                Button::new("machines-add", "Add Machine")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::Small)
                    .color(Color::Muted)
                    .start_icon(
                        Icon::new(IconName::Plus)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    ),
            )
            .anchor(gpui::Anchor::TopRight)
            .offset(gpui::point(px(0.), px(4.)));
        div()
            .debug_selector(|| "machines-add".into())
            .child(menu)
            .into_any_element()
    }

    /// Saves a provider's machine, switched on, under the name it has there.
    fn save_cloud_machine(
        &mut self,
        provider: Provider,
        machine: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) {
        let link = CloudLink {
            provider,
            machine: machine.to_string(),
        };
        let name = name.to_string();
        self.app_settings.update(cx, |store, cx| {
            store.update(
                |settings| {
                    if settings.cloud_machine(provider, &link.machine).is_none() {
                        settings.add_cloud_machine(name, link);
                    }
                },
                cx,
            )
        });
    }

    /// The provider's New Machine dialog, in the shell's modal layer.
    pub(super) fn open_new_cloud_machine(
        &mut self,
        provider: Provider,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = cx.weak_entity();
        let dialog = cx.new(|cx| NewCloudMachine::new(provider, page, window, cx));
        cx.emit(SettingsPageEvent::NewCloudMachine(dialog));
    }

    /// Makes the machine the dialog describes, asking first when making it charges, until
    /// "Don't ask again" for its account (topic 5 C). Whether it's being made now; otherwise
    /// the dialog stays under the question, and closes once it's answered yes.
    pub(super) fn create_cloud_machine(
        &mut self,
        provider: Provider,
        machine: CreateMachine,
        dialog: WeakEntity<NewCloudMachine>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
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
        let Some(charge) = provider.description().charge.filter(|_| asks) else {
            self.send_cloud_create(provider, machine, cx);
            return true;
        };
        let page = cx.weak_entity();
        let title = format!("Create {}?", machine.name);
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest {
            icon: IconName::Coins,
            title: title.into(),
            message: charge_message(
                charge,
                state.as_ref().and_then(|state| state.usage.as_ref()),
            )
            .into(),
            confirm_label: "Create".into(),
            on_confirm: Rc::new(move |_, cx| {
                page.update(cx, |page, cx| {
                    page.send_cloud_create(provider, machine.clone(), cx)
                })
                .log_err();
                dialog.update(cx, |_, cx| cx.emit(DismissEvent)).log_err();
            }),
            checkbox: account.map(|account| self.dont_ask_again(provider, account)),
            is_charge: true,
        }));
        false
    }

    /// Makes the machine through its provider's CLI, and saves it once it's made, so agentZ
    /// connects to it. Its row says it's being made meanwhile.
    fn send_cloud_create(
        &mut self,
        provider: Provider,
        machine: CreateMachine,
        cx: &mut Context<Self>,
    ) {
        self.cloud_errors.remove(&provider);
        let name = machine.name.clone();
        let request = Machines::local(cx)
            .read(cx)
            .request(Request::CreateCloudMachine { provider, machine });
        let app_settings = self.app_settings.clone();
        cx.spawn(async move |this, cx| {
            let created = match request.await {
                Ok(Response::CloudMachineCreated(created)) => Ok(created),
                Ok(_) => Err(anyhow!("its server didn't say which machine it made")),
                Err(error) => Err(error),
            };
            match created {
                // Saved even if Settings closed meanwhile.
                Ok(created) => app_settings.update(cx, |store, cx| {
                    store.update(
                        |settings| {
                            if settings.cloud_machine(provider, &created.id).is_none() {
                                settings.add_cloud_machine(
                                    created.name.clone(),
                                    CloudLink {
                                        provider,
                                        machine: created.id.clone(),
                                    },
                                );
                            }
                        },
                        cx,
                    )
                }),
                Err(error) => {
                    this.update(cx, |this, cx| {
                        this.cloud_errors
                            .insert(provider, format!("Couldn't make {name}: {error:#}").into());
                        cx.notify();
                    })
                    .ok();
                }
            }
        })
        .detach();
        cx.notify();
    }

    /// One Remove… (topic 9 B): agentZ forgets the machine, and with the box ticked, its
    /// provider deletes it with its disk once its name is typed.
    pub(super) fn confirm_remove_cloud_machine(
        &mut self,
        provider: Provider,
        profile: u64,
        label: SharedString,
        deletable: Option<(String, String)>,
        cx: &mut Context<Self>,
    ) {
        let description = provider.description();
        let page = cx.weak_entity();
        let app_settings = self.app_settings.clone();
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest {
            icon: IconName::Trash,
            title: format!("Remove {label} from agentZ?").into(),
            message: format!(
                "agentZ stops connecting to it. It stays at {}, and you can add it back.",
                description.name
            )
            .into(),
            confirm_label: "Remove".into(),
            on_confirm: Rc::new(move |_, cx| {
                app_settings.update(cx, |store, cx| {
                    store.update(
                        |settings| settings.machines.retain(|saved| saved.id != profile),
                        cx,
                    )
                })
            }),
            checkbox: deletable.map(|(machine, name)| ConfirmCheckbox {
                label: format!("Also delete it at {}, with its disk", description.name).into(),
                type_to_confirm: Some(name.into()),
                on_confirm_ticked: Rc::new(move |cx| {
                    page.update(cx, |page, cx| {
                        page.send_cloud_change(provider, machine.clone(), Lifecycle::Delete, cx)
                    })
                    .log_err();
                }),
            }),
            is_charge: false,
        }));
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

pub(super) fn signed_in_account(state: Option<&ProviderState>) -> Option<String> {
    match &state?.status {
        ProviderStatus::SignedIn { account, .. } => Some(account.clone()),
        _ => None,
    }
}

/// A machine its provider is making, named as it was asked for, until it's listed.
fn render_creating_row(provider: Provider, name: &str, cx: &App) -> AnyElement {
    let slug = provider.description().program;
    let selector = format!("cloud-creating-{slug}-{name}");
    h_flex()
        .debug_selector(move || selector)
        .px_4()
        .py_2p5()
        .gap_3()
        .child(icon_tile(
            Icon::new(provider_icon(provider)).color(Color::Muted),
            MARK_SIZE,
            cx,
        ))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(Label::new(name.to_string()).truncate())
                .child(
                    Label::new(format!(
                        "{} is making it. agentZ connects once it's made.",
                        provider.description().name
                    ))
                    .size(LabelSize::Small)
                    .color(Color::Muted)
                    .truncate(),
                ),
        )
        .child(status_badge(
            change_label(Change::Create),
            Color::Accent.color(cx),
        ))
        .into_any_element()
}

pub(super) fn lifecycle_icon(change: Lifecycle) -> IconName {
    match change {
        Lifecycle::Start | Lifecycle::Resume => IconName::PlayFilled,
        Lifecycle::Stop => IconName::Stop,
        Lifecycle::Restart => IconName::RotateCw,
        Lifecycle::Pause => IconName::DebugPause,
        Lifecycle::Delete => IconName::Trash,
    }
}
