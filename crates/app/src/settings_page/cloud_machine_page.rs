//! A cloud machine's page in Settings › Machines (design/cloud topic 7 A), opened from its row
//! as an agent's page is: back to Machines, its mark, name and state, its main change and a
//! menu, then a tab for each of its provider's extras. Proton's are Overview and SSH Keys.
//! What it shows is read from the provider as it opens and after each listing; what it does
//! goes through the provider's CLI on this Mac's server.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use agentz_protocol::cloud::{
    CloudMachine, MachineAction, MachineDetails, Provider, ProviderState,
};
use agentz_protocol::{Request, Response};
use gpui::{AnyElement, Context, FontWeight, PathPromptOptions, SharedString, Task, Window};
use ui::{
    ContextMenu, ContextMenuEntry, DropdownMenu, DropdownStyle, IconPosition, PopoverMenu, Tooltip,
    prelude::*,
};
use util::ResultExt as _;

use super::cloud_providers::lifecycle_icon;
use super::new_cloud_machine::minutes_label;
use super::{SettingsPage, SettingsPageEvent, on_page, render_row, render_rows, render_section};
use crate::cloud_machines::{
    MainAction, balance_left, change_label, dollars, duration_label, lifecycle_label, main_action,
    other_changes, provider_icon, size_summary, state_color, state_label, time_label,
};
use crate::confirm_dialog::ConfirmRequest;
use crate::controls::{ActionButton, icon_tile, status_badge};
use crate::machines::Machines;

/// "When idle"'s choices, in minutes; zero never stops.
const IDLE_STOPS: [u64; 4] = [30, 60, 240, 0];

/// The keys `ssh` offers by itself. A Proton computer's `ssh-config` names none, so these are
/// what agentZ reaches it with.
const DEFAULT_KEYS: [&str; 7] = [
    "id_rsa",
    "id_ecdsa",
    "id_ecdsa_sk",
    "id_ed25519",
    "id_ed25519_sk",
    "id_xmss",
    "id_dsa",
];

/// The machine whose page is open, and what its provider said of it.
pub(super) struct MachinePage {
    provider: Provider,
    machine: String,
    tab: MachineTab,
    /// Once read; why it couldn't be read otherwise.
    details: Option<Result<MachineDetails, SharedString>>,
    /// The provider's listing the details were read after: a newer one reads them again.
    listed_at: Option<SystemTime>,
    /// Why the last thing done to it failed.
    error: Option<SharedString>,
    _reading: Option<Task<()>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MachineTab {
    Overview,
    SshKeys,
}

impl MachineTab {
    /// Overview, then a tab for each extra the provider has.
    fn of(provider: Provider) -> Vec<MachineTab> {
        let capabilities = &provider.description().capabilities;
        [
            (true, MachineTab::Overview),
            (capabilities.ssh_keys, MachineTab::SshKeys),
        ]
        .into_iter()
        .filter_map(|(has, tab)| has.then_some(tab))
        .collect()
    }

    fn label(self) -> &'static str {
        match self {
            MachineTab::Overview => "Overview",
            MachineTab::SshKeys => "SSH Keys",
        }
    }
}

type MachineActionKey = (Provider, String, MachineAction);

impl SettingsPage {
    pub(super) fn open_machine_page(
        &mut self,
        provider: Provider,
        machine: String,
        tab: MachineTab,
        cx: &mut Context<Self>,
    ) {
        let listed_at = Machines::local(cx)
            .read(cx)
            .cloud()
            .provider(provider)
            .and_then(|state| state.listed_at);
        self.machine_page = Some(MachinePage {
            provider,
            machine,
            tab,
            details: None,
            listed_at,
            error: None,
            _reading: None,
        });
        self.read_machine_details(cx);
        self.scroll_content_to_top();
        cx.notify();
    }

    fn close_machine_page(&mut self, cx: &mut Context<Self>) {
        self.machine_page = None;
        self.scroll_content_to_top();
        cx.notify();
    }

    fn select_machine_tab(&mut self, tab: MachineTab, cx: &mut Context<Self>) {
        if let Some(page) = &mut self.machine_page {
            page.tab = tab;
        }
        self.scroll_content_to_top();
        cx.notify();
    }

    /// Reads the open page's machine from its provider. What was read before stays shown
    /// until the new read is in.
    fn read_machine_details(&mut self, cx: &mut Context<Self>) {
        let Some(page) = &self.machine_page else {
            return;
        };
        let (provider, machine) = (page.provider, page.machine.clone());
        let request = Machines::local(cx)
            .read(cx)
            .request(Request::CloudMachineDetails {
                provider,
                machine: machine.clone(),
            });
        let reading = cx.spawn(async move |this, cx| {
            let details = match request.await {
                Ok(Response::CloudMachineDetails(details)) => Ok(details),
                Ok(_) => Err(SharedString::from("its server answered something else")),
                Err(error) => Err(format!("{error:#}").into()),
            };
            this.update(cx, |this, cx| {
                let Some(page) = this.machine_page_for(provider, &machine) else {
                    return;
                };
                match (details, &page.details) {
                    // A machine read once keeps what it showed, with why it's out of date.
                    (Err(error), Some(Ok(_))) => {
                        page.error = Some(format!("Couldn't read it again: {error}").into())
                    }
                    (details, _) => page.details = Some(details),
                }
                cx.notify();
            })
            .ok();
        });
        if let Some(page) = &mut self.machine_page {
            page._reading = Some(reading);
        }
    }

    /// Follows the provider's listings: each new one reads the page's machine again, and a
    /// machine the provider doesn't list anymore closes its page.
    pub(super) fn sync_machine_page(&mut self, cx: &mut Context<Self>) {
        let Some(page) = &self.machine_page else {
            return;
        };
        let (listed_at, is_gone) = {
            let local = Machines::local(cx);
            let cloud = local.read(cx).cloud();
            let state = cloud.provider(page.provider);
            let listed_at = state.and_then(|state| state.listed_at);
            let is_gone = state.is_some_and(|state| {
                state.listed_at.is_some()
                    && state.error.is_none()
                    && !state
                        .machines
                        .iter()
                        .any(|machine| machine.id == page.machine)
            });
            (listed_at, is_gone)
        };
        if is_gone {
            self.close_machine_page(cx);
        } else if listed_at != page.listed_at {
            if let Some(page) = &mut self.machine_page {
                page.listed_at = listed_at;
            }
            self.read_machine_details(cx);
        }
    }

    fn machine_page_for(&mut self, provider: Provider, machine: &str) -> Option<&mut MachinePage> {
        self.machine_page
            .as_mut()
            .filter(|page| page.provider == provider && page.machine == machine)
    }

    /// Does `action` to the machine through its provider, once at a time.
    pub(super) fn send_machine_action(
        &mut self,
        provider: Provider,
        machine: String,
        action: MachineAction,
        cx: &mut Context<Self>,
    ) {
        let key: MachineActionKey = (provider, machine.clone(), action.clone());
        if self.cloud_actions.contains(&key) {
            return;
        }
        self.cloud_actions.push(key.clone());
        self.cloud_errors.remove(&provider);
        if let Some(page) = self.machine_page_for(provider, &machine) {
            page.error = None;
        }
        let request = Machines::local(cx)
            .read(cx)
            .request(Request::CloudMachineAction {
                provider,
                machine,
                action,
            });
        cx.spawn(async move |this, cx| {
            let response = request.await;
            this.update(cx, |this, cx| this.finish_machine_action(key, response, cx))
                .ok();
        })
        .detach();
        cx.notify();
    }

    /// What fails shows on the page; anything else changed the machine, so it's read again.
    fn finish_machine_action(
        &mut self,
        key: MachineActionKey,
        response: anyhow::Result<Response>,
        cx: &mut Context<Self>,
    ) {
        self.cloud_actions.retain(|doing| *doing != key);
        let (provider, machine, action) = &key;
        match (response, action) {
            (Err(error), _) => {
                let failure = action_failure(action);
                self.set_machine_error(*provider, machine, format!("{failure}: {error:#}"), cx);
                return;
            }
            // This Mac's server keeps it, and sends it with the listing.
            (Ok(_), MachineAction::StopWhenIdle(_)) => {}
            (Ok(_), MachineAction::ReplaceSshKeys(_)) => {
                if self.machine_page_for(*provider, machine).is_some() {
                    self.read_machine_details(cx);
                }
            }
        }
        cx.notify();
    }

    /// On the machine's page when it's open, otherwise over its provider's group.
    fn set_machine_error(
        &mut self,
        provider: Provider,
        machine: &str,
        error: String,
        cx: &mut Context<Self>,
    ) {
        let label = self.machine_label(provider, machine, cx);
        match self.machine_page_for(provider, machine) {
            Some(page) => page.error = Some(error.into()),
            None => {
                self.cloud_errors
                    .insert(provider, format!("{label}: {error}").into());
            }
        }
        cx.notify();
    }

    fn is_doing(
        &self,
        provider: Provider,
        machine: &str,
        matches: impl Fn(&MachineAction) -> bool,
    ) -> bool {
        self.cloud_actions
            .iter()
            .any(|(doing, on, action)| *doing == provider && on == machine && matches(action))
    }

    /// What agentZ calls the machine: its saved name, or its provider's.
    fn machine_label(&self, provider: Provider, machine: &str, cx: &App) -> SharedString {
        self.app_settings
            .read(cx)
            .settings()
            .cloud_machine(provider, machine)
            .map(|profile| profile.display_label())
            .filter(|label| !label.is_empty())
            .or_else(|| {
                Machines::local(cx)
                    .read(cx)
                    .cloud()
                    .machine(provider, machine)
                    .map(|listed| listed.name.clone())
            })
            .unwrap_or_else(|| machine.to_string())
            .into()
    }

    /// Replace Keys…: the public keys to let in, picked on this Mac.
    fn pick_ssh_keys(&mut self, provider: Provider, machine: String, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Use Keys".into()),
        });
        cx.spawn(async move |this, cx| {
            let keys = match paths.await {
                Ok(Ok(Some(keys))) => keys,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    log::error!("failed to pick SSH keys: {error:#}");
                    return;
                }
            };
            this.update(cx, |this, cx| {
                this.confirm_replace_ssh_keys(provider, machine, keys, cx)
            })
            .ok();
        })
        .detach();
    }

    /// Asks before replacing the machine's keys, and warns when agentZ's own wouldn't be
    /// among them.
    fn confirm_replace_ssh_keys(
        &mut self,
        provider: Provider,
        machine: String,
        keys: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if keys.is_empty() {
            return;
        }
        if let Some(key) = keys
            .iter()
            .find(|key| key.extension().is_none_or(|extension| extension != "pub"))
        {
            let error = format!(
                "{} isn't a public key. Pick the files that end in .pub.",
                key.display()
            );
            self.set_machine_error(provider, &machine, error, cx);
            return;
        }
        let label = self.machine_label(provider, &machine, cx);
        let names = keys
            .iter()
            .filter_map(|key| key.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(", ");
        let ssh_folder = dirs::home_dir().map(|home| home.join(".ssh"));
        let keeps_own = keys
            .iter()
            .any(|key| is_own_key(key, ssh_folder.as_deref()));
        let mut message = format!("From now on, only these keys reach it: {names}.");
        if !keeps_own {
            message.push_str(
                " None of them is one of this Mac's own keys in ~/.ssh, which agentZ reaches it \
                 with, so agentZ won't reach it anymore.",
            );
        }
        let page = cx.weak_entity();
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest {
            icon: IconName::Lock,
            title: format!("Replace the SSH keys of {label}?").into(),
            message: message.into(),
            confirm_label: "Replace Keys".into(),
            on_confirm: Rc::new(move |_, cx| {
                page.update(cx, |page, cx| {
                    page.send_machine_action(
                        provider,
                        machine.clone(),
                        MachineAction::ReplaceSshKeys(keys.clone()),
                        cx,
                    )
                })
                .log_err();
            }),
            checkbox: None,
            is_charge: false,
        }));
    }

    /// The way back to Machines, as an agent's page has to Agents.
    pub(super) fn render_machine_page_header(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        h_flex()
            .h(px(28.))
            .child(
                h_flex()
                    .id("machines-back")
                    .debug_selector(|| "machines-back".into())
                    .ml_neg_1p5()
                    .px_1p5()
                    .py_0p5()
                    .gap_1p5()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|style| style.bg(colors.ghost_element_hover))
                    .on_click(cx.listener(|this, _, _, cx| this.close_machine_page(cx)))
                    .child(
                        Icon::new(IconName::ArrowLeft)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new("Machines")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
            )
            .into_any_element()
    }

    /// The machine's heading and tabs, what couldn't be read or done, then the open tab.
    pub(super) fn render_machine_page(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let Some(page) = &self.machine_page else {
            return Vec::new();
        };
        let provider = page.provider;
        let name = provider.description().name;
        let cloud = Machines::local(cx).read(cx).cloud().clone();
        let state = cloud.provider(provider);
        let Some(machine) = cloud.machine(provider, &page.machine) else {
            return vec![render_rows(
                vec![render_message(
                    format!("{name} doesn't list it anymore."),
                    Color::Muted,
                )],
                cx,
            )];
        };
        let is_changing = state.is_some_and(|state| {
            state
                .changing
                .iter()
                .any(|changing| changing.machine == machine.id)
        });
        let can_act = !is_changing && state.is_some_and(|state| state.status.is_signed_in());
        let mut sections = vec![
            v_flex()
                .gap(px(22.))
                .child(self.render_machine_heading(page, state, machine, can_act, cx))
                .child(self.render_machine_tabs(page, cx))
                .into_any_element(),
        ];
        if let Some(error) = page.error.clone() {
            sections.push(
                div()
                    .debug_selector(|| "machine-page-errors".into())
                    .child(Label::new(error).size(LabelSize::Small).color(Color::Error))
                    .into_any_element(),
            );
        }
        let details = match &page.details {
            None => {
                sections.push(render_rows(
                    vec![render_message(
                        format!("Reading it from {name}…"),
                        Color::Muted,
                    )],
                    cx,
                ));
                return sections;
            }
            Some(Err(error)) => {
                let retry = Button::new("machine-page-retry", "Try Again")
                    .style(ButtonStyle::Subtle)
                    .label_size(LabelSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| this.read_machine_details(cx)));
                sections.push(render_rows(
                    vec![
                        h_flex()
                            .pr_2()
                            .justify_between()
                            .child(render_message(
                                format!("Couldn't read it from {name}: {error}"),
                                Color::Error,
                            ))
                            .child(retry)
                            .into_any_element(),
                    ],
                    cx,
                ));
                return sections;
            }
            Some(Ok(details)) => details,
        };
        sections.extend(match page.tab {
            MachineTab::Overview => {
                self.render_overview(provider, state, machine, details, can_act, window, cx)
            }
            MachineTab::SshKeys => self.render_ssh_keys(provider, machine, can_act, cx),
        });
        sections
    }

    /// Its mark, its name beside its state, "Proton · 3f1c9a52… · ssh.proton.computer:22021",
    /// the change it most likely needs, and a menu with the other changes and Remove….
    fn render_machine_heading(
        &self,
        page: &MachinePage,
        state: Option<&ProviderState>,
        machine: &CloudMachine,
        can_act: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let provider = page.provider;
        let description = provider.description();
        let label = self.machine_label(provider, &machine.id, cx);
        let changing = state
            .and_then(|state| {
                state
                    .changing
                    .iter()
                    .find(|changing| changing.machine == machine.id)
            })
            .map(|changing| changing.change);
        let badge = match changing {
            Some(change) => status_badge(change_label(change), Color::Accent.color(cx)),
            None => status_badge(
                state_label(&machine.state).to_string(),
                state_color(&machine.state).color(cx),
            ),
        };
        let line = [
            Some(description.name.to_string()),
            (machine.id != machine.name).then(|| machine.id.clone()),
            machine
                .ssh_address
                .as_ref()
                .map(|address| format!("{}:{}", address.host, address.port)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");

        let main = main_action(provider, &machine.state)
            .filter(|_| can_act)
            .map(|action| match action {
                MainAction::Change(change) => {
                    let machine_id = machine.id.clone();
                    let label = label.clone();
                    ActionButton::new("machine-page-main", lifecycle_label(change))
                        .start_icon(Icon::new(lifecycle_icon(change)).size(IconSize::XSmall))
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
                MainAction::AddCredit(url) => ActionButton::new("machine-page-main", "Add Credit")
                    .end_icon(Icon::new(IconName::ArrowUpRight).size(IconSize::XSmall))
                    .on_click(move |_, _, cx| cx.open_url(url)),
            })
            .map(|button| {
                div()
                    .debug_selector(|| "machine-page-main".into())
                    .child(button)
            });
        let changes = if can_act {
            other_changes(provider, &machine.state)
        } else {
            Vec::new()
        };
        let removable = self
            .app_settings
            .read(cx)
            .settings()
            .cloud_machine(provider, &machine.id)
            .map(|profile| {
                // Deleting it needs the provider to list it, by the name it has there.
                let deletable = can_act.then(|| (machine.id.clone(), machine.name.clone()));
                (profile.id, label.clone(), deletable)
            });
        let menu = (!changes.is_empty() || removable.is_some()).then(|| {
            let page = cx.weak_entity();
            let machine_id = machine.id.clone();
            let label = label.clone();
            PopoverMenu::new("machine-page-menu")
                .menu(move |window, cx| {
                    let page = page.clone();
                    let changes = changes.clone();
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
                        if let Some((id, name, deletable)) = removable {
                            if has_changes {
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
                    IconButton::new("machine-page-menu-trigger", IconName::Ellipsis)
                        .style(ButtonStyle::Outlined)
                        .size(ButtonSize::Medium)
                        .icon_size(IconSize::Small)
                        .icon_color(Color::Muted),
                    Tooltip::text("More"),
                )
                .anchor(gpui::Anchor::TopRight)
                .offset(gpui::point(px(0.), px(4.)))
        });

        h_flex()
            .debug_selector(|| "machine-page-heading".into())
            .gap_4()
            .child(icon_tile(
                Icon::new(provider_icon(provider)).color(Color::Default),
                px(52.),
                cx,
            ))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        h_flex()
                            .min_w_0()
                            .gap_2()
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .text_size(rems_from_px(18_f32))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(label),
                            )
                            .child(badge),
                    )
                    .child(
                        Label::new(line)
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    ),
            )
            .child(h_flex().flex_none().gap_2().children(main).children(menu))
            .into_any_element()
    }

    /// Overview and the provider's tabs, underlined when chosen, as an agent's page's.
    fn render_machine_tabs(&self, page: &MachinePage, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        h_flex()
            .gap_5()
            .border_b_1()
            .border_color(colors.border_variant)
            .children(MachineTab::of(page.provider).into_iter().map(|tab| {
                let is_current = tab == page.tab;
                let selector = format!(
                    "machine-tab-{}",
                    tab.label().to_lowercase().replace(' ', "-")
                );
                div()
                    .id(SharedString::from(selector.clone()))
                    .debug_selector(move || selector)
                    .pb(px(10.))
                    // Over the strip's border, so the underline replaces it.
                    .mb(px(-1.))
                    .border_b_2()
                    .border_color(if is_current {
                        colors.text_accent
                    } else {
                        gpui::transparent_black()
                    })
                    .text_size(rems_from_px(13_f32))
                    .text_color(if is_current {
                        colors.text
                    } else {
                        colors.text_muted
                    })
                    .when(!is_current, |this| {
                        this.cursor_pointer()
                            .hover(|style| style.text_color(colors.text))
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.select_machine_tab(tab, cx)),
                            )
                    })
                    .child(tab.label())
            }))
            .into_any_element()
    }

    /// What it is and does: its size, how long it's run and what that cost, its last activity,
    /// then Lifecycle: when it's stopped for being idle.
    #[allow(clippy::too_many_arguments)]
    fn render_overview(
        &self,
        provider: Provider,
        state: Option<&ProviderState>,
        machine: &CloudMachine,
        details: &MachineDetails,
        can_act: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let description = provider.description();
        let now = SystemTime::now();
        let size = size_summary(&machine.size);
        let mut rows = vec![render_value_row(
            "Size",
            render_value_text(if size.is_empty() {
                "Not given".to_string()
            } else {
                size
            }),
            None,
        )];
        if let Some(started_at) = details.started_at.filter(|_| machine.state.is_running()) {
            let elapsed = now.duration_since(started_at).unwrap_or_default().as_secs();
            let cost = description.hourly_micro_usd.map(|hourly| {
                let micro_usd = u128::from(hourly) * u128::from(elapsed) / 3600;
                format!(
                    "{} so far",
                    dollars(i64::try_from(micro_usd).unwrap_or(i64::MAX))
                )
            });
            let value = [
                Some(format!("Since {}", time_label(started_at, now))),
                Some(duration_label(elapsed)),
                cost,
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
            rows.push(render_value_row("Running", render_value_text(value), None));
        }
        if let Some(charge) = description.charge {
            let balance = state
                .and_then(|state| state.usage.as_ref())
                .and_then(balance_left);
            let value = [Some(charge.to_string()), balance]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" · ");
            rows.push(render_value_row("Cost", render_value_text(value), None));
        }
        if let Some(at) = details.last_activity {
            rows.push(render_value_row(
                "Last activity",
                render_value_text(time_label(at, now)),
                None,
            ));
        }
        let mut sections = vec![render_rows(rows, cx)];

        let mut lifecycle = Vec::new();
        if description.capabilities.idle_stop {
            lifecycle.push(self.render_stop_when_idle_row(
                provider,
                &machine.id,
                state,
                can_act,
                window,
                cx,
            ));
        }
        if !lifecycle.is_empty() {
            sections.push(render_section("Lifecycle", lifecycle, cx));
        }
        sections
    }

    /// "When idle" for a machine this Mac's server stops (topic 12 B): how long it may run with
    /// no agent working on it and nothing of it open before it's stopped.
    fn render_stop_when_idle_row(
        &self,
        provider: Provider,
        machine: &str,
        state: Option<&ProviderState>,
        can_act: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let minutes = state
            .and_then(|state| state.stop_when_idle.get(machine))
            .map_or(0, |after| after.as_secs() / 60);
        let (value, short) = match minutes {
            0 => ("Never stops".to_string(), "Never".to_string()),
            minutes => (
                format!("Stops after {} idle", minutes_label(minutes)),
                minutes_label(minutes),
            ),
        };
        let setting = self.is_doing(provider, machine, |action| {
            matches!(action, MachineAction::StopWhenIdle(_))
        });
        let page = cx.weak_entity();
        let machine_id = machine.to_string();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for choice in IDLE_STOPS {
                let machine = machine_id.clone();
                let label = match choice {
                    0 => "Never stop".to_string(),
                    choice => format!("Stop after {}", minutes_label(choice)),
                };
                menu = menu.toggleable_entry(
                    label,
                    choice == minutes,
                    IconPosition::End,
                    None,
                    on_page(&page, move |page, _, cx| {
                        let after = (choice > 0).then(|| Duration::from_secs(choice * 60));
                        page.send_machine_action(
                            provider,
                            machine.clone(),
                            MachineAction::StopWhenIdle(after),
                            cx,
                        )
                    }),
                );
            }
            menu
        });
        let dropdown = div()
            .debug_selector(|| "machine-stop-when-idle".into())
            .child(
                DropdownMenu::new("machine-stop-when-idle", short, menu)
                    .style(DropdownStyle::Outlined)
                    .disabled(!can_act || setting),
            );
        render_value_row(
            "When idle",
            v_flex()
                .gap_0p5()
                .child(render_value_text(value))
                .child(
                    Label::new("Idle is no agent working on it and nothing of it open in agentZ.")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .into_any_element(),
            Some(dropdown.into_any_element()),
        )
    }

    /// Proton's SSH Keys: Replace Keys…, from public key files on this Mac.
    fn render_ssh_keys(
        &self,
        provider: Provider,
        machine: &CloudMachine,
        can_act: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let replacing = self.is_doing(provider, &machine.id, |action| {
            matches!(action, MachineAction::ReplaceSshKeys(_))
        });
        let machine_id = machine.id.clone();
        let replace = div()
            .debug_selector(|| "machine-ssh-keys-replace".into())
            .child(
                ActionButton::new(
                    "machine-ssh-keys-replace",
                    if replacing {
                        "Replacing…"
                    } else {
                        "Replace Keys…"
                    },
                )
                .disabled(!can_act || replacing)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.pick_ssh_keys(provider, machine_id.clone(), cx)
                })),
            )
            .into_any_element();
        vec![render_rows(
            vec![render_row(
                "SSH Keys",
                format!(
                    "{} lets in only the public keys it's given. agentZ reaches it with this \
                     Mac's own keys in ~/.ssh, so keep one of them.",
                    provider.description().name
                ),
                replace,
                cx,
            )],
            cx,
        )]
    }
}

/// A line inside a group of rows.
fn render_message(message: impl Into<SharedString>, color: Color) -> AnyElement {
    h_flex()
        .px_4()
        .py_3()
        .child(Label::new(message).size(LabelSize::Small).color(color))
        .into_any_element()
}

/// A name on the left, what it is beside it, and a control at the right.
fn render_value_row(
    label: impl Into<SharedString>,
    value: AnyElement,
    control: Option<AnyElement>,
) -> AnyElement {
    h_flex()
        .px_4()
        .py(px(11.))
        .gap_3()
        .child(
            div()
                .w(px(150.))
                .flex_none()
                .child(Label::new(label).size(LabelSize::Small).color(Color::Muted)),
        )
        .child(div().flex_1().min_w_0().child(value))
        .children(control.map(|control| div().flex_none().child(control)))
        .into_any_element()
}

fn render_value_text(text: impl Into<SharedString>) -> AnyElement {
    Label::new(text).size(LabelSize::Small).into_any_element()
}

/// What a failed action couldn't do, before why.
fn action_failure(action: &MachineAction) -> String {
    match action {
        MachineAction::ReplaceSshKeys(_) => "Couldn't replace its SSH keys".into(),
        MachineAction::StopWhenIdle(_) => "Couldn't change when it stops".into(),
    }
}

/// One of the keys `ssh` offers by itself, in `ssh_folder`.
fn is_own_key(key: &Path, ssh_folder: Option<&Path>) -> bool {
    ssh_folder.is_some_and(|folder| key.parent() == Some(folder))
        && key
            .file_stem()
            .and_then(|stem| stem.to_str())
            .is_some_and(|stem| DEFAULT_KEYS.contains(&stem))
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use agentz_protocol::cloud::{CloudSnapshot, CloudState, ProviderStatus};
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::{Entity, TestAppContext, VisualTestContext};

    use super::super::Section;
    use super::super::tests::{cloud_machine, provider_state};
    use super::*;
    use crate::machines::MachineId;
    use crate::server_client::ServerClient;

    /// The test server's client, what it was asked, and whether what it's asked fails.
    struct Served {
        client: Entity<ServerClient>,
        requests: Rc<RefCell<Vec<Request>>>,
        fails: Rc<Cell<bool>>,
    }

    /// Proton's account, signed in, with `machines`.
    fn listing(listed_at: Option<SystemTime>, machines: Vec<CloudMachine>) -> CloudSnapshot {
        let mut proton = provider_state(
            Provider::Proton,
            ProviderStatus::SignedIn {
                version: "1.0.0".into(),
                account: "me@example.com".into(),
            },
        );
        proton.machines = machines;
        proton.listed_at = listed_at;
        CloudSnapshot {
            providers: vec![proton],
            ..Default::default()
        }
    }

    fn devbox() -> CloudMachine {
        cloud_machine("c-1", "devbox-3", CloudState::Running)
    }

    /// This Mac's server, listing `devbox-3` (`c-1`) running. Its reads and actions are done
    /// unless `fails` is set, when they fail as an unreachable server's do.
    fn serve(cx: &mut TestAppContext) -> Served {
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let fails = Rc::new(Cell::new(false));
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            super::super::init(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            client.update(cx, |client, cx| {
                let requests = requests.clone();
                let fails = fails.clone();
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    match request {
                        _ if fails.get() => None,
                        Request::CloudMachineDetails { .. } => {
                            Some(Response::CloudMachineDetails(MachineDetails::default()))
                        }
                        Request::CloudMachineAction { .. } => Some(Response::Ok),
                        _ => None,
                    }
                });
                client.set_cloud_for_test(listing(None, vec![devbox()]), cx);
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        Served {
            client,
            requests,
            fails,
        }
    }

    fn actions(requests: &RefCell<Vec<Request>>) -> Vec<MachineAction> {
        requests
            .borrow()
            .iter()
            .filter_map(|request| match request {
                Request::CloudMachineAction { action, .. } => Some(action.clone()),
                _ => None,
            })
            .collect()
    }

    fn reads(requests: &RefCell<Vec<Request>>, machine: &str) -> usize {
        requests
            .borrow()
            .iter()
            .filter(|request| {
                matches!(request, Request::CloudMachineDetails { machine: read, .. } if read == machine)
            })
            .count()
    }

    fn click(selector: &'static str, cx: &mut VisualTestContext) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is shown"));
        cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        cx.run_until_parked();
    }

    fn page_error(page: &Entity<SettingsPage>, cx: &mut VisualTestContext) -> Option<String> {
        page.read_with(cx, |page, _| {
            page.machine_page
                .as_ref()
                .and_then(|machine_page| machine_page.error.as_ref())
                .map(|error| error.to_string())
        })
    }

    /// The confirmations the page asks for.
    fn confirms(
        page: &Entity<SettingsPage>,
        cx: &mut VisualTestContext,
    ) -> Rc<RefCell<Vec<ConfirmRequest>>> {
        let confirms: Rc<RefCell<Vec<ConfirmRequest>>> = Rc::default();
        cx.update(|_, cx| {
            let confirms = confirms.clone();
            cx.subscribe(page, move |_, event: &SettingsPageEvent, _| {
                if let SettingsPageEvent::Confirm(request) = event {
                    confirms.borrow_mut().push(request.clone());
                }
            })
            .detach();
        });
        confirms
    }

    /// A machine's mark and name open its page, with Overview and SSH Keys, and Machines goes
    /// back. Each new listing reads it again, keeping what it showed when that fails, and a
    /// listing without it closes its page.
    #[gpui::test]
    fn a_cloud_machines_page_opens_from_its_row(cx: &mut TestAppContext) {
        let Served {
            client,
            requests,
            fails,
        } = serve(cx);
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Machines, window, cx)
        });
        cx.run_until_parked();

        click("cloud-open-proton-c-1", cx);
        assert!(cx.debug_bounds("machine-page-heading").is_some());
        assert_eq!(reads(&requests, "c-1"), 1);
        for shown in [
            "machine-tab-overview",
            "machine-tab-ssh-keys",
            "machine-page-main",
            "machine-stop-when-idle",
        ] {
            assert!(cx.debug_bounds(shown).is_some(), "{shown} is shown");
        }
        click("machine-tab-ssh-keys", cx);
        assert!(cx.debug_bounds("machine-ssh-keys-replace").is_some());
        assert!(cx.debug_bounds("machine-stop-when-idle").is_none());
        click("machine-tab-overview", cx);

        fails.set(true);
        client.update(cx, |client, cx| {
            client.set_cloud_for_test(listing(Some(SystemTime::now()), vec![devbox()]), cx)
        });
        cx.run_until_parked();
        assert_eq!(reads(&requests, "c-1"), 2);
        assert!(
            cx.debug_bounds("machine-stop-when-idle").is_some(),
            "what it read before stays"
        );
        let error = page_error(&page, cx).unwrap_or_default();
        assert!(error.starts_with("Couldn't read it again: "), "{error}");
        assert!(cx.debug_bounds("machine-page-errors").is_some());
        fails.set(false);

        click("machines-back", cx);
        assert!(cx.debug_bounds("machine-page-heading").is_none());
        assert!(cx.debug_bounds("cloud-row-proton-c-1").is_some());

        click("cloud-open-proton-c-1", cx);
        client.update(cx, |client, cx| {
            client.set_cloud_for_test(listing(Some(SystemTime::now()), Vec::new()), cx)
        });
        cx.run_until_parked();
        assert!(page.read_with(cx, |page, _| page.machine_page.is_none()));
    }

    /// When idle tells this Mac's server when to stop the machine, and what fails shows on its
    /// page.
    #[gpui::test]
    fn when_idle_sets_when_its_stopped(cx: &mut TestAppContext) {
        let Served {
            requests, fails, ..
        } = serve(cx);
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Machines, window, cx);
            page.open_machine_page(Provider::Proton, "c-1".into(), MachineTab::Overview, cx)
        });
        cx.run_until_parked();
        let stop_after = |after: Option<Duration>, cx: &mut VisualTestContext| {
            page.update(cx, |page, cx| {
                page.send_machine_action(
                    Provider::Proton,
                    "c-1".into(),
                    MachineAction::StopWhenIdle(after),
                    cx,
                )
            });
            cx.run_until_parked();
        };

        let hour = Some(Duration::from_secs(3600));
        stop_after(hour, cx);
        assert_eq!(actions(&requests), [MachineAction::StopWhenIdle(hour)]);
        assert_eq!(page_error(&page, cx), None);

        fails.set(true);
        stop_after(None, cx);
        assert_eq!(
            page_error(&page, cx).as_deref(),
            Some("Couldn't change when it stops: not connected to This Mac")
        );
        assert!(cx.debug_bounds("machine-page-errors").is_some());
    }

    /// Replace Keys… takes public keys only, and warns when none is one of this Mac's own,
    /// which agentZ reaches the computer with.
    #[gpui::test]
    fn replacing_ssh_keys_takes_public_keys_and_warns_without_this_macs(cx: &mut TestAppContext) {
        let Served { requests, .. } = serve(cx);
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        let confirms = confirms(&page, cx);
        page.update(cx, |page, cx| {
            page.open_machine_page(Provider::Proton, "c-1".into(), MachineTab::SshKeys, cx)
        });
        cx.run_until_parked();
        let replace = |keys: Vec<PathBuf>, cx: &mut VisualTestContext| {
            page.update(cx, |page, cx| {
                page.confirm_replace_ssh_keys(Provider::Proton, "c-1".into(), keys, cx)
            });
            cx.run_until_parked();
        };

        replace(vec![PathBuf::from("/tmp/keys/deploy")], cx);
        assert!(confirms.borrow().is_empty());
        let error = page_error(&page, cx).unwrap_or_default();
        assert!(error.contains("isn't a public key"), "{error}");

        let deploy = PathBuf::from("/tmp/keys/deploy.pub");
        replace(vec![deploy.clone()], cx);
        let confirm = confirms.borrow_mut().pop().expect("it asks first");
        assert_eq!(confirm.title.as_ref(), "Replace the SSH keys of devbox-3?");
        assert!(
            confirm.message.contains("agentZ won't reach it anymore"),
            "{}",
            confirm.message
        );
        cx.update(|window, cx| (confirm.on_confirm)(window, cx));
        cx.run_until_parked();
        assert_eq!(
            actions(&requests),
            [MachineAction::ReplaceSshKeys(vec![deploy])]
        );

        if let Some(home) = dirs::home_dir() {
            replace(vec![home.join(".ssh").join("id_ed25519.pub")], cx);
            let confirm = confirms.borrow_mut().pop().expect("it asks first");
            assert!(
                !confirm.message.contains("won't reach it"),
                "{}",
                confirm.message
            );
        }
    }

    #[test]
    fn own_keys_are_the_ones_ssh_offers_from_its_folder() {
        let folder = Path::new("/Users/me/.ssh");
        assert!(is_own_key(
            Path::new("/Users/me/.ssh/id_ed25519.pub"),
            Some(folder)
        ));
        assert!(is_own_key(
            Path::new("/Users/me/.ssh/id_rsa.pub"),
            Some(folder)
        ));
        assert!(!is_own_key(
            Path::new("/Users/me/.ssh/deploy.pub"),
            Some(folder)
        ));
        assert!(!is_own_key(
            Path::new("/Users/me/keys/id_ed25519.pub"),
            Some(folder)
        ));
        assert!(!is_own_key(
            Path::new("/Users/me/.ssh/id_ed25519.pub"),
            None
        ));
        assert_eq!(
            MachineTab::of(Provider::Proton),
            [MachineTab::Overview, MachineTab::SshKeys]
        );
    }
}
