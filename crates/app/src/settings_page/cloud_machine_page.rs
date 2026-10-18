//! A cloud machine's page in Settings › Machines (design/cloud topic 7 A), opened from its row
//! as an agent's page is: back to Machines, its mark, name and state, its main change, Open
//! Desktop and a menu, then a tab for each of its provider's extras. Instabox's are Overview,
//! Snapshots (topic 8 A), Ports, Desktop, Vault and SSH; Proton's are Overview and SSH Keys.
//! What it shows is read from the provider as it opens and after each listing; what it does
//! goes through the provider's CLI on this Mac's server.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use agentz_protocol::cloud::{
    AccountRules, CloudMachine, MachineAction, MachineDetails, MachineSnapshot, MetricSample,
    Provider, ProviderState, ProviderStatus, PublishedPort, TimeoutAt, VaultWrite,
};
use agentz_protocol::{Request, Response};
use gpui::{
    AnyElement, Context, FontWeight, Hsla, PathBuilder, PathPromptOptions, SharedString, Task,
    Window, canvas,
};
use ui::{
    ContextMenu, ContextMenuEntry, DropdownMenu, DropdownStyle, IconPosition, PopoverMenu, Tooltip,
    prelude::*,
};
use util::ResultExt as _;

use super::cloud_machine_dialog::{CloudMachineDialog, DialogKind};
use super::cloud_providers::{COPIED_FOR, lifecycle_icon};
use super::new_cloud_machine::{NewCloudMachine, Timeout, minutes_label};
use super::{
    SettingsPage, SettingsPageEvent, on_page, render_row, render_rows, render_section,
    render_section_with_actions,
};
use crate::cloud_machines::{
    MainAction, balance_left, change_label, dollars, duration_label, gigabytes, lifecycle_label,
    main_action, memory_label, other_changes, provider_icon, size_summary, state_color,
    state_label, time_label,
};
use crate::confirm_dialog::ConfirmRequest;
use crate::controls::{ActionButton, copy_to_clipboard, icon_tile, status_badge};
use crate::machines::Machines;

/// "When idle"'s choices, in minutes; zero never pauses or stops.
const IDLE_PAUSES: [u64; 4] = [30, 60, 240, 0];
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
    Snapshots,
    Ports,
    Desktop,
    Vault,
    Ssh,
    SshKeys,
}

impl MachineTab {
    /// Overview, then a tab for each extra the provider has.
    fn of(provider: Provider) -> Vec<MachineTab> {
        let capabilities = &provider.description().capabilities;
        [
            (true, MachineTab::Overview),
            (capabilities.snapshots, MachineTab::Snapshots),
            (capabilities.public_ports, MachineTab::Ports),
            (capabilities.desktop, MachineTab::Desktop),
            (capabilities.vault, MachineTab::Vault),
            (capabilities.ssh_tokens, MachineTab::Ssh),
            (capabilities.ssh_keys, MachineTab::SshKeys),
        ]
        .into_iter()
        .filter_map(|(has, tab)| has.then_some(tab))
        .collect()
    }

    fn label(self) -> &'static str {
        match self {
            MachineTab::Overview => "Overview",
            MachineTab::Snapshots => "Snapshots",
            MachineTab::Ports => "Ports",
            MachineTab::Desktop => "Desktop",
            MachineTab::Vault => "Vault",
            MachineTab::Ssh => "SSH",
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

    /// Hands on what an action gave: a desktop to open, or a command or link copied, which
    /// its button says for a moment. Anything else changed the machine, so it's read again.
    fn finish_machine_action(
        &mut self,
        key: MachineActionKey,
        response: anyhow::Result<Response>,
        cx: &mut Context<Self>,
    ) {
        self.cloud_actions.retain(|doing| *doing != key);
        let (provider, machine, action) = &key;
        let output = match response {
            Ok(Response::CloudOutput(output)) => Some(output),
            Ok(_) => None,
            Err(error) => {
                let failure = action_failure(action);
                self.set_machine_error(*provider, machine, format!("{failure}: {error:#}"), cx);
                return;
            }
        };
        match (action, output) {
            (MachineAction::DesktopLink { view_only: false }, Some(url)) => cx.open_url(&url),
            (
                MachineAction::DesktopLink { view_only: true } | MachineAction::SshCommand,
                Some(text),
            ) => {
                copy_to_clipboard(&text, cx);
                self.show_copied(key.clone(), cx);
            }
            // This Mac's server keeps it, and sends it with the listing.
            (MachineAction::StopWhenIdle(_), _) => {}
            (MachineAction::DesktopLink { .. } | MachineAction::SshCommand, None) => {
                let failure = action_failure(action);
                let name = provider.description().name;
                self.set_machine_error(
                    *provider,
                    machine,
                    format!("{failure}: {name} gave nothing back"),
                    cx,
                );
            }
            _ => {
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

    fn show_copied(&mut self, key: MachineActionKey, cx: &mut Context<Self>) {
        self.copied_action = Some(key.clone());
        self.copied_action_reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            this.update(cx, |this, cx| {
                if this.copied_action.as_ref() == Some(&key) {
                    this.copied_action = None;
                    cx.notify();
                }
            })
            .ok();
        }));
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

    fn is_copied(&self, provider: Provider, machine: &str, action: &MachineAction) -> bool {
        self.copied_action
            .as_ref()
            .is_some_and(|(copied, on, copied_action)| {
                *copied == provider && on == machine && copied_action == action
            })
    }

    /// Save Snapshot… or Publish Port…, in the shell's modal layer.
    pub(super) fn open_machine_dialog(
        &mut self,
        provider: Provider,
        machine: String,
        kind: DialogKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A snapshot is named after the machine as its provider has it.
        let name: SharedString = Machines::local(cx)
            .read(cx)
            .cloud()
            .machine(provider, &machine)
            .map_or_else(|| machine.clone(), |listed| listed.name.clone())
            .into();
        let page = cx.weak_entity();
        let dialog =
            cx.new(|cx| CloudMachineDialog::new(provider, machine, name, kind, page, window, cx));
        cx.emit(SettingsPageEvent::CloudMachineDialog(dialog));
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

    /// A snapshot's New Machine: the provider's dialog, starting from it.
    fn open_new_machine_from(
        &mut self,
        provider: Provider,
        snapshot: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = cx.weak_entity();
        let dialog =
            cx.new(|cx| NewCloudMachine::new(provider, page, window, cx).from_snapshot(snapshot));
        cx.emit(SettingsPageEvent::NewCloudMachine(dialog));
    }

    fn confirm_delete_snapshot(
        &mut self,
        provider: Provider,
        machine: String,
        snapshot: &MachineSnapshot,
        cx: &mut Context<Self>,
    ) {
        let page = cx.weak_entity();
        let id = snapshot.id.clone();
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest {
            icon: IconName::Trash,
            title: format!("Delete {}?", snapshot.name).into(),
            message: format!(
                "{} deletes it, and new machines can't start from it anymore.",
                provider.description().name
            )
            .into(),
            confirm_label: "Delete".into(),
            on_confirm: Rc::new(move |_, cx| {
                page.update(cx, |page, cx| {
                    page.send_machine_action(
                        provider,
                        machine.clone(),
                        MachineAction::DeleteSnapshot(id.clone()),
                        cx,
                    )
                })
                .log_err();
            }),
            checkbox: None,
            is_charge: false,
        }));
    }

    fn confirm_revoke_ssh_commands(
        &mut self,
        provider: Provider,
        machine: String,
        cx: &mut Context<Self>,
    ) {
        let label = self.machine_label(provider, &machine, cx);
        let page = cx.weak_entity();
        cx.emit(SettingsPageEvent::Confirm(ConfirmRequest {
            icon: IconName::Terminal,
            title: format!("Revoke every SSH command for {label}?").into(),
            message: "Every SSH command made for it stops working, and their sessions close."
                .into(),
            confirm_label: "Revoke All".into(),
            on_confirm: Rc::new(move |_, cx| {
                page.update(cx, |page, cx| {
                    page.send_machine_action(
                        provider,
                        machine.clone(),
                        MachineAction::RevokeSshCommands,
                        cx,
                    )
                })
                .log_err();
            }),
            checkbox: None,
            is_charge: false,
        }));
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
        let details = page
            .details
            .as_ref()
            .and_then(|details| details.as_ref().ok());
        let errors: Vec<SharedString> = page
            .error
            .iter()
            .cloned()
            .chain(
                details
                    .into_iter()
                    .flat_map(|details| details.errors.iter().cloned().map(SharedString::from)),
            )
            .collect();
        if !errors.is_empty() {
            sections.push(
                v_flex()
                    .debug_selector(|| "machine-page-errors".into())
                    .gap_1()
                    .children(
                        errors.into_iter().map(|error| {
                            Label::new(error).size(LabelSize::Small).color(Color::Error)
                        }),
                    )
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
            MachineTab::Snapshots => self.render_snapshots(provider, machine, details, can_act, cx),
            MachineTab::Ports => self.render_ports(provider, machine, details, can_act, cx),
            MachineTab::Desktop => self.render_desktop(provider, machine, can_act, cx),
            MachineTab::Vault => self.render_vault(provider, machine, details, can_act, cx),
            MachineTab::Ssh => self.render_ssh(provider, machine, can_act, cx),
            MachineTab::SshKeys => self.render_ssh_keys(provider, machine, can_act, cx),
        });
        sections
    }

    /// Its mark, its name beside its state, "Instabox · m_q4mz… · universal 2026-10-13", the
    /// change it most likely needs, Open Desktop, and a menu with the other changes and
    /// Remove….
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
        let template = page
            .details
            .as_ref()
            .and_then(|details| details.as_ref().ok())
            .and_then(|details| details.template.clone())
            .or_else(|| machine.template.clone());
        let line = [
            Some(description.name.to_string()),
            (machine.id != machine.name).then(|| machine.id.clone()),
            template,
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
        let desktop = description.capabilities.desktop.then(|| {
            let opening = self.is_doing(provider, &machine.id, |action| {
                *action == MachineAction::DesktopLink { view_only: false }
            });
            let machine_id = machine.id.clone();
            div()
                .debug_selector(|| "machine-page-desktop".into())
                .child(
                    ActionButton::new(
                        "machine-page-desktop",
                        if opening {
                            "Opening…"
                        } else {
                            "Open Desktop"
                        },
                    )
                    .start_icon(Icon::new(IconName::Screen).size(IconSize::XSmall))
                    .disabled(!can_act || !machine.state.is_running() || opening)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.send_machine_action(
                            provider,
                            machine_id.clone(),
                            MachineAction::DesktopLink { view_only: false },
                            cx,
                        )
                    })),
                )
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
            .child(
                h_flex()
                    .flex_none()
                    .gap_2()
                    .children(main)
                    .children(desktop)
                    .children(menu),
            )
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

    /// What it is and does: its size, CPU and memory, how long it's run and what that cost,
    /// then Lifecycle: its timeout, idle pause, Vault and labels.
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
        if let Some(sample) = details.metrics.last() {
            let cpu = details
                .metrics
                .iter()
                .map(|sample| (sample.cpu_percent / 100.) as f32)
                .collect();
            let memory = details.metrics.iter().map(memory_fraction).collect();
            rows.push(render_value_row(
                "CPU",
                render_value_text(format!("{:.0}% now", sample.cpu_percent)),
                Some(sparkline(cpu, cx.theme().colors().text_accent)),
            ));
            rows.push(render_value_row(
                "Memory",
                render_value_text(memory_label(
                    sample.memory_used_bytes,
                    sample.memory_total_bytes,
                )),
                Some(sparkline(memory, Color::Success.color(cx))),
            ));
        }
        if let Some(running) = &details.running {
            let value = [
                Some(duration_label(running.seconds)),
                running
                    .vcpu_seconds
                    .map(|seconds| format!("{} vCPU-hours", (seconds as f64 / 3600.).round())),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
            rows.push(render_value_row(
                "Running this month",
                render_value_text(value),
                None,
            ));
        }
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

        let rules = match state.map(|state| &state.status) {
            Some(ProviderStatus::SignedIn { rules, .. }) => rules.clone(),
            _ => AccountRules::default(),
        };
        let mut lifecycle = Vec::new();
        if description.capabilities.timeouts {
            lifecycle.push(self.render_timeout_row(
                provider,
                &machine.id,
                details,
                &rules,
                can_act,
                window,
                cx,
            ));
        }
        if description.capabilities.idle_pause {
            let held = state.and_then(|state| state.held_awake.get(&machine.id).copied());
            lifecycle.push(self.render_idle_pause_row(
                provider,
                &machine.id,
                details,
                held,
                &rules,
                can_act,
                window,
                cx,
            ));
        }
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
        if let Some(vault) = &details.vault {
            lifecycle.push(render_value_row(
                "Vault",
                render_value_text(if vault.enabled {
                    "Gets your Vault items when it boots"
                } else {
                    "Doesn't get your Vault items"
                }),
                None,
            ));
        }
        if !details.labels.is_empty() {
            lifecycle.push(render_value_row(
                "Labels",
                h_flex()
                    .flex_wrap()
                    .gap_1()
                    .children(
                        details
                            .labels
                            .iter()
                            .map(|(key, value)| render_tag(format!("{key}={value}"), cx)),
                    )
                    .into_any_element(),
                None,
            ));
        }
        if !lifecycle.is_empty() {
            sections.push(render_section("Lifecycle", lifecycle, cx));
        }
        sections
    }

    /// "Pause or delete after": when its timeout comes, and a menu of new ones within the
    /// account's limit, counted from now.
    #[allow(clippy::too_many_arguments)]
    fn render_timeout_row(
        &self,
        provider: Provider,
        machine: &str,
        details: &MachineDetails,
        rules: &AccountRules,
        can_act: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let now = SystemTime::now();
        let (value, short) = match details.timeout {
            Some(TimeoutAt { at, delete }) => {
                let at = time_label(at, now);
                let verb = if delete { "Is deleted" } else { "Pauses" };
                (format!("{verb} at {at}"), at)
            }
            None => ("Never".to_string(), "Never".to_string()),
        };
        let setting = self.is_doing(provider, machine, |action| {
            matches!(action, MachineAction::SetTimeout(_))
        });
        let allowed: Vec<Timeout> = Timeout::ALL
            .into_iter()
            .filter(|timeout| timeout.is_allowed(rules))
            .collect();
        let has_timeout = details.timeout.is_some();
        let page = cx.weak_entity();
        let machine_id = machine.to_string();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            let mut is_deleting = false;
            for timeout in allowed {
                // Pausing, then deleting.
                if matches!(timeout, Timeout::After { delete: true, .. }) && !is_deleting {
                    is_deleting = true;
                    menu = menu.separator();
                }
                let machine = machine_id.clone();
                menu = menu.toggleable_entry(
                    timeout.label(),
                    timeout == Timeout::Never && !has_timeout,
                    IconPosition::End,
                    None,
                    on_page(&page, move |page, _, cx| {
                        page.send_machine_action(
                            provider,
                            machine.clone(),
                            MachineAction::SetTimeout(timeout.timeout()),
                            cx,
                        )
                    }),
                );
            }
            menu
        });
        let dropdown = div().debug_selector(|| "machine-timeout".into()).child(
            DropdownMenu::new("machine-timeout", short, menu)
                .style(DropdownStyle::Outlined)
                .disabled(!can_act || setting),
        );
        render_value_row(
            "Pause or delete after",
            render_value_text(value),
            Some(dropdown.into_any_element()),
        )
    }

    /// "When idle": how long it idles before it pauses itself, unless the account fixes that.
    /// While agents work on it, this Mac's server has it off (topic 12 A), and `held` is the
    /// pause it sets back.
    #[allow(clippy::too_many_arguments)]
    fn render_idle_pause_row(
        &self,
        provider: Provider,
        machine: &str,
        details: &MachineDetails,
        held: Option<Duration>,
        rules: &AccountRules,
        can_act: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let description = provider.description();
        let now = SystemTime::now();
        let minutes = rules
            .fixed_idle_pause
            .or(held.filter(|pause| !pause.is_zero()))
            .or(details.idle_pause)
            .map_or(0, |after| after.as_secs() / 60);
        let (value, short) = match minutes {
            0 => ("Never pauses".to_string(), "Never".to_string()),
            minutes => (
                format!("Pauses after {} idle", minutes_label(minutes)),
                minutes_label(minutes),
            ),
        };
        let hints = [
            rules.fixed_idle_pause.map(|_| {
                // So agentZ can't hold it off while agents work (topic 12 A).
                format!(
                    "{} sets this for your account, so turns may stop when it pauses the {}.",
                    description.name, description.noun
                )
            }),
            held.map(|_| "Off while agents work on it, then set back.".to_string()),
            details.idle_paused_at.map(|at| {
                format!(
                    "It paused itself for being idle at {}.",
                    time_label(at, now)
                )
            }),
        ];
        let setting = self.is_doing(provider, machine, |action| {
            matches!(action, MachineAction::SetIdlePause(_))
        });
        let page = cx.weak_entity();
        let machine_id = machine.to_string();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for choice in IDLE_PAUSES {
                let machine = machine_id.clone();
                let label = match choice {
                    0 => "Never pause".to_string(),
                    choice => format!("Pause after {}", minutes_label(choice)),
                };
                menu = menu.toggleable_entry(
                    label,
                    choice == minutes,
                    IconPosition::End,
                    None,
                    on_page(&page, move |page, _, cx| {
                        page.send_machine_action(
                            provider,
                            machine.clone(),
                            MachineAction::SetIdlePause(Duration::from_secs(choice * 60)),
                            cx,
                        )
                    }),
                );
            }
            menu
        });
        let dropdown = div().debug_selector(|| "machine-idle-pause".into()).child(
            DropdownMenu::new("machine-idle-pause", short, menu)
                .style(DropdownStyle::Outlined)
                .disabled(!can_act || setting || rules.fixed_idle_pause.is_some()),
        );
        render_value_row(
            "When idle",
            v_flex()
                .gap_0p5()
                .child(render_value_text(value))
                .children(
                    hints
                        .into_iter()
                        .flatten()
                        .map(|hint| Label::new(hint).size(LabelSize::Small).color(Color::Muted)),
                )
                .into_any_element(),
            Some(dropdown.into_any_element()),
        )
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
            for choice in IDLE_PAUSES {
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

    /// The snapshots saved from it (topic 8 A), each with New Machine and Delete, and Save
    /// Snapshot….
    fn render_snapshots(
        &self,
        provider: Provider,
        machine: &CloudMachine,
        details: &MachineDetails,
        can_act: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let saving = self.is_doing(provider, &machine.id, |action| {
            matches!(action, MachineAction::SaveSnapshot { .. })
        });
        let machine_id = machine.id.clone();
        let save = div()
            .debug_selector(|| "machine-save-snapshot".into())
            .child(
                Button::new(
                    "machine-save-snapshot",
                    if saving {
                        "Saving…"
                    } else {
                        "Save Snapshot…"
                    },
                )
                .style(ButtonStyle::Subtle)
                .label_size(LabelSize::Small)
                .color(Color::Muted)
                .start_icon(
                    Icon::new(IconName::Plus)
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                )
                .disabled(!can_act || saving)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_machine_dialog(
                        provider,
                        machine_id.clone(),
                        DialogKind::SaveSnapshot,
                        window,
                        cx,
                    )
                })),
            )
            .into_any_element();
        let rows = if details.snapshots.is_empty() {
            vec![render_message(
                "No snapshots saved from it yet.",
                Color::Muted,
            )]
        } else {
            details
                .snapshots
                .iter()
                .map(|snapshot| {
                    self.render_snapshot_row(provider, &machine.id, snapshot, can_act, cx)
                })
                .collect()
        };
        let label = self.machine_label(provider, &machine.id, cx);
        vec![render_section_with_actions(
            format!("Saved from {label}"),
            rows,
            save,
            cx,
        )]
    }

    fn render_snapshot_row(
        &self,
        provider: Provider,
        machine: &str,
        snapshot: &MachineSnapshot,
        can_act: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let now = SystemTime::now();
        let id = snapshot.id.clone();
        let detail = [
            Some(
                if snapshot.software_only {
                    "software only"
                } else {
                    "software, home and workspace"
                }
                .to_string(),
            ),
            snapshot.size_bytes.map(gigabytes),
            snapshot.created_at.map(|at| time_label(at, now)),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        let deleting = self.is_doing(provider, machine, |action| {
            *action == MachineAction::DeleteSnapshot(id.clone())
        });
        let new_machine = {
            let id = id.clone();
            Button::new(
                SharedString::from(format!("machine-snapshot-new-{id}")),
                "New Machine",
            )
            .style(ButtonStyle::Outlined)
            .label_size(LabelSize::Small)
            .disabled(!can_act || !snapshot.is_ready)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_new_machine_from(provider, id.clone(), window, cx)
            }))
        };
        let delete = {
            let machine = machine.to_string();
            let snapshot = snapshot.clone();
            IconButton::new(
                SharedString::from(format!("machine-snapshot-delete-{id}")),
                IconName::Trash,
            )
            .icon_size(IconSize::Small)
            .icon_color(Color::Muted)
            .tooltip(Tooltip::text("Delete…"))
            .disabled(!can_act || deleting)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.confirm_delete_snapshot(provider, machine.clone(), &snapshot, cx)
            }))
        };
        let selector = format!("machine-snapshot-{id}");
        let new_selector = format!("machine-snapshot-new-{id}");
        let delete_selector = format!("machine-snapshot-delete-{id}");
        h_flex()
            .debug_selector(move || selector)
            .px_4()
            .py(px(11.))
            .gap_3()
            .child(
                Icon::new(IconName::Archive)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Label::new(snapshot.name.clone()).truncate())
                            .when(!snapshot.is_ready, |row| {
                                row.child(render_tag("Saving…", cx))
                            }),
                    )
                    .child(
                        Label::new(detail)
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .debug_selector(move || new_selector)
                    .child(new_machine),
            )
            .child(
                div()
                    .flex_none()
                    .debug_selector(move || delete_selector)
                    .child(delete),
            )
            .into_any_element()
    }

    /// Its published ports, each with its URL to copy or open and Unpublish, and Publish
    /// Port….
    fn render_ports(
        &self,
        provider: Provider,
        machine: &CloudMachine,
        details: &MachineDetails,
        can_act: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let publishing = self.is_doing(provider, &machine.id, |action| {
            matches!(action, MachineAction::PublishPort { .. })
        });
        let machine_id = machine.id.clone();
        let publish = div()
            .debug_selector(|| "machine-publish-port".into())
            .child(
                Button::new(
                    "machine-publish-port",
                    if publishing {
                        "Publishing…"
                    } else {
                        "Publish Port…"
                    },
                )
                .style(ButtonStyle::Subtle)
                .label_size(LabelSize::Small)
                .color(Color::Muted)
                .start_icon(
                    Icon::new(IconName::Plus)
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                )
                .disabled(!can_act || publishing)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_machine_dialog(
                        provider,
                        machine_id.clone(),
                        DialogKind::PublishPort,
                        window,
                        cx,
                    )
                })),
            )
            .into_any_element();
        let rows = if details.ports.is_empty() {
            vec![render_message("No ports published.", Color::Muted)]
        } else {
            details
                .ports
                .iter()
                .map(|port| self.render_port_row(provider, &machine.id, port, can_act, cx))
                .collect()
        };
        vec![
            v_flex()
                .gap_2()
                .child(render_section_with_actions("Published", rows, publish, cx))
                .child(
                    Label::new(
                        "Anyone with its URL reaches the service until it's unpublished. The \
                         URL answers only while the machine runs.",
                    )
                    .size(LabelSize::Small)
                    .color(Color::Muted),
                )
                .into_any_element(),
        ]
    }

    fn render_port_row(
        &self,
        provider: Provider,
        machine: &str,
        port: &PublishedPort,
        can_act: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let number = port.port;
        let title = match &port.name {
            Some(name) => format!("{number} · {name}"),
            None => number.to_string(),
        };
        let unpublishing = self.is_doing(provider, machine, |action| {
            *action == MachineAction::UnpublishPort(number)
        });
        let url_buttons = port.url.clone().map(|url| {
            let open = url.clone();
            h_flex()
                .gap_1()
                .child(
                    IconButton::new(
                        SharedString::from(format!("machine-port-copy-{number}")),
                        IconName::Copy,
                    )
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Copy URL"))
                    .on_click(move |_, _, cx| copy_to_clipboard(&url, cx)),
                )
                .child(
                    IconButton::new(
                        SharedString::from(format!("machine-port-open-{number}")),
                        IconName::ArrowUpRight,
                    )
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Open"))
                    .on_click(move |_, _, cx| cx.open_url(&open)),
                )
        });
        let machine = machine.to_string();
        let selector = format!("machine-port-{number}");
        let unpublish_selector = format!("machine-port-unpublish-{number}");
        h_flex()
            .debug_selector(move || selector)
            .px_4()
            .py(px(11.))
            .gap_3()
            .child(
                Icon::new(IconName::Public)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(Label::new(title).truncate())
                    .child(
                        Label::new(port.url.clone().unwrap_or_else(|| "No URL yet".into()))
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    ),
            )
            .children(url_buttons)
            .child(
                div().debug_selector(move || unpublish_selector).child(
                    IconButton::new(
                        SharedString::from(format!("machine-port-unpublish-{number}")),
                        IconName::Close,
                    )
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Unpublish"))
                    .disabled(!can_act || unpublishing)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.send_machine_action(
                            provider,
                            machine.clone(),
                            MachineAction::UnpublishPort(number),
                            cx,
                        )
                    })),
                ),
            )
            .into_any_element()
    }

    /// Open Desktop (a link to use it) and Copy View Link (to watch it, for an hour).
    fn render_desktop(
        &self,
        provider: Provider,
        machine: &CloudMachine,
        can_act: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let is_running = machine.state.is_running();
        let open = self.render_action_button(
            provider,
            &machine.id,
            MachineAction::DesktopLink { view_only: false },
            "machine-desktop-open",
            "Open Desktop",
            "Opening…",
            can_act && is_running,
            cx,
        );
        let view = self.render_action_button(
            provider,
            &machine.id,
            MachineAction::DesktopLink { view_only: true },
            "machine-desktop-view-link",
            "Copy View Link",
            "Making…",
            can_act && is_running,
            cx,
        );
        let rows = vec![
            render_row(
                "Desktop",
                format!(
                    "Use its desktop in your browser. {} installs and starts it first.",
                    provider.description().name
                ),
                open,
                cx,
            ),
            render_row(
                "View Link",
                "A link that shows its desktop, without control, for an hour.",
                view,
                cx,
            ),
        ];
        let mut sections = vec![render_rows(rows, cx)];
        if !is_running {
            sections.push(render_needs_running());
        }
        sections
    }

    /// What it gets from the account's Vault, and Push Now.
    fn render_vault(
        &self,
        provider: Provider,
        machine: &CloudMachine,
        details: &MachineDetails,
        can_act: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let name = provider.description().name;
        let vault = match &details.vault {
            None => {
                return vec![render_rows(
                    vec![render_message(
                        format!("{name} didn't say what it gets from your Vault."),
                        Color::Muted,
                    )],
                    cx,
                )];
            }
            Some(vault) if !vault.enabled => {
                return vec![render_rows(
                    vec![render_message(
                        format!("It doesn't get your Vault items. {name} turns that off for good."),
                        Color::Muted,
                    )],
                    cx,
                )];
            }
            Some(vault) => vault,
        };
        let is_running = machine.state.is_running();
        let push = self.render_action_button(
            provider,
            &machine.id,
            MachineAction::PushVault,
            "machine-vault-push",
            "Push Now",
            "Pushing…",
            can_act && is_running,
            cx,
        );
        let rows = if vault.items.is_empty() {
            vec![render_message(
                "Your Vault has nothing for it.",
                Color::Muted,
            )]
        } else {
            vault
                .items
                .iter()
                .map(|item| {
                    h_flex()
                        .px_4()
                        .py(px(11.))
                        .gap_3()
                        .child(
                            Icon::new(IconName::Lock)
                                .size(IconSize::Small)
                                .color(Color::Muted),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(Label::new(item.name.clone()).truncate()),
                        )
                        .child(
                            Label::new(match &item.path {
                                Some(path) => format!("~/{}", path.trim_start_matches('/')),
                                None => "Environment variable".into(),
                            })
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                        )
                        .into_any_element()
                })
                .collect()
        };
        let now = SystemTime::now();
        let (note, color) = match &vault.last_write {
            Some(VaultWrite {
                error: Some(error), ..
            }) => (format!("Its last write failed: {error}"), Color::Error),
            Some(VaultWrite { at: Some(at), .. }) => (
                format!(
                    "It gets them when it boots; they were last written at {}.",
                    time_label(*at, now)
                ),
                Color::Muted,
            ),
            _ => ("It gets them when it boots.".to_string(), Color::Muted),
        };
        let mut sections = vec![
            v_flex()
                .gap_2()
                .child(render_section_with_actions("What it gets", rows, push, cx))
                .child(Label::new(note).size(LabelSize::Small).color(color))
                .into_any_element(),
        ];
        if !is_running {
            sections.push(render_needs_running());
        }
        sections
    }

    /// Copy SSH Command (a 7-day token for any terminal) and Revoke All….
    fn render_ssh(
        &self,
        provider: Provider,
        machine: &CloudMachine,
        can_act: bool,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let copy = self.render_action_button(
            provider,
            &machine.id,
            MachineAction::SshCommand,
            "machine-ssh-command",
            "Copy SSH Command",
            "Making…",
            can_act,
            cx,
        );
        let revoking = self.is_doing(provider, &machine.id, |action| {
            *action == MachineAction::RevokeSshCommands
        });
        let machine_id = machine.id.clone();
        let revoke = div()
            .debug_selector(|| "machine-ssh-revoke".into())
            .child(
                ActionButton::new(
                    "machine-ssh-revoke",
                    if revoking {
                        "Revoking…"
                    } else {
                        "Revoke All…"
                    },
                )
                .disabled(!can_act || revoking)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.confirm_revoke_ssh_commands(provider, machine_id.clone(), cx)
                })),
            )
            .into_any_element();
        vec![render_rows(
            vec![
                render_row(
                    "SSH Command",
                    "A plain ssh command for any terminal, which works for 7 days.",
                    copy,
                    cx,
                ),
                render_row(
                    "Revoke All",
                    "Stops every SSH command made for it, and closes their sessions.",
                    revoke,
                    cx,
                ),
            ],
            cx,
        )]
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

    /// A button for an action, which says it's under way, and "Copied" once it copied what
    /// it gave.
    #[allow(clippy::too_many_arguments)]
    fn render_action_button(
        &self,
        provider: Provider,
        machine: &str,
        action: MachineAction,
        id: &'static str,
        label: &'static str,
        doing_label: &'static str,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let doing = self.is_doing(provider, machine, |doing| *doing == action);
        let copied = self.is_copied(provider, machine, &action);
        let label = if doing {
            doing_label
        } else if copied {
            "Copied"
        } else {
            label
        };
        let machine = machine.to_string();
        div()
            .debug_selector(move || id.into())
            .child(
                ActionButton::new(id, label)
                    .when(copied, |button| {
                        button.start_icon(Icon::new(IconName::Check).size(IconSize::XSmall))
                    })
                    .disabled(!enabled || doing)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.send_machine_action(provider, machine.clone(), action.clone(), cx)
                    })),
            )
            .into_any_element()
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

fn render_needs_running() -> AnyElement {
    Label::new("It needs to be running for these.")
        .size(LabelSize::Small)
        .color(Color::Muted)
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

fn render_tag(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .flex_none()
        .px(px(6.))
        .rounded(px(4.))
        .bg(cx.theme().colors().element_hover)
        .child(Label::new(text).size(LabelSize::XSmall))
        .into_any_element()
}

/// A line through samples from 0 to 1, oldest at the left.
fn sparkline(values: Vec<f32>, color: Hsla) -> AnyElement {
    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            if values.len() < 2 {
                return;
            }
            let last = (values.len() - 1) as f32;
            let mut path = PathBuilder::stroke(px(1.5));
            for (index, value) in values.iter().enumerate() {
                let point = gpui::point(
                    bounds.left() + bounds.size.width * (index as f32 / last),
                    bounds.bottom() - px(1.) - (bounds.size.height - px(2.)) * value.clamp(0., 1.),
                );
                if index == 0 {
                    path.move_to(point);
                } else {
                    path.line_to(point);
                }
            }
            if let Some(path) = path.build().log_err() {
                window.paint_path(path, color);
            }
        },
    )
    .w(px(120.))
    .h(px(28.))
    .flex_none()
    .into_any_element()
}

fn memory_fraction(sample: &MetricSample) -> f32 {
    if sample.memory_total_bytes == 0 {
        0.
    } else {
        (sample.memory_used_bytes as f64 / sample.memory_total_bytes as f64) as f32
    }
}

/// What a failed action couldn't do, before why.
fn action_failure(action: &MachineAction) -> String {
    match action {
        MachineAction::SaveSnapshot { name, .. } => format!("Couldn't save {name}"),
        MachineAction::DeleteSnapshot(_) => "Couldn't delete the snapshot".into(),
        MachineAction::PublishPort { port, .. } => format!("Couldn't publish port {port}"),
        MachineAction::UnpublishPort(port) => format!("Couldn't unpublish port {port}"),
        MachineAction::PushVault => "Couldn't push your Vault's items".into(),
        MachineAction::SetTimeout(_) => "Couldn't change its timeout".into(),
        MachineAction::SetIdlePause(_) => "Couldn't change when it pauses".into(),
        MachineAction::SshCommand => "Couldn't make an SSH command".into(),
        MachineAction::RevokeSshCommands => "Couldn't revoke its SSH commands".into(),
        MachineAction::DesktopLink { view_only: false } => "Couldn't open its desktop".into(),
        MachineAction::DesktopLink { view_only: true } => "Couldn't make a view link".into(),
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
pub(super) mod tests {
    use std::cell::{Cell, RefCell};

    use agentz_protocol::cloud::{CloudSnapshot, CloudState};
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::{Entity, TestAppContext, VisualTestContext};

    use super::super::Section;
    use super::super::tests::{cloud_machine, provider_state};
    use super::*;
    use crate::machines::MachineId;
    use crate::server_client::ServerClient;

    /// The test server's client, what it was asked, and whether its machines' reads fail.
    pub(in crate::settings_page) struct Served {
        pub client: Entity<ServerClient>,
        pub requests: Rc<RefCell<Vec<Request>>>,
        pub fails_reads: Rc<Cell<bool>>,
    }

    /// Instabox's account, signed in with `agents-1` (`m-1`) running, and Proton's with
    /// `devbox-3` (`c-1`) unless `proton` says otherwise.
    fn listing(listed_at: Option<SystemTime>, proton: Vec<CloudMachine>) -> CloudSnapshot {
        let signed_in = || ProviderStatus::SignedIn {
            version: "1.0.0".into(),
            account: "me@example.com".into(),
            rules: AccountRules::default(),
        };
        let mut instabox = provider_state(Provider::Instabox, signed_in());
        instabox.machines = vec![cloud_machine("m-1", "agents-1", CloudState::Running)];
        instabox.listed_at = listed_at;
        let mut proton_state = provider_state(Provider::Proton, signed_in());
        proton_state.machines = proton;
        proton_state.listed_at = listed_at;
        CloudSnapshot {
            providers: vec![instabox, proton_state],
            ..Default::default()
        }
    }

    fn devbox() -> CloudMachine {
        cloud_machine("c-1", "devbox-3", CloudState::Running)
    }

    fn instabox_details() -> MachineDetails {
        MachineDetails {
            template: Some("universal 2026-10-13".into()),
            snapshots: vec![MachineSnapshot {
                id: "s-1".into(),
                name: "dev-base".into(),
                is_ready: true,
                source_machine: Some("m-1".into()),
                created_at: None,
                software_only: false,
                size_bytes: Some(4 << 30),
            }],
            ports: vec![PublishedPort {
                port: 3000,
                name: Some("web".into()),
                url: Some("https://3000-m-1.instabox.dev".into()),
            }],
            ..Default::default()
        }
    }

    /// This Mac's server, with [`listing`]'s machines. `m-1` reads with a snapshot and a
    /// port; its SSH command and desktop links come back, changing when it pauses fails as
    /// an unreachable server does, and every other action is done.
    pub(in crate::settings_page) fn serve(cx: &mut TestAppContext) -> Served {
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let fails_reads = Rc::new(Cell::new(false));
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
                let fails_reads = fails_reads.clone();
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    match request {
                        Request::CloudMachineDetails { .. } if fails_reads.get() => None,
                        Request::CloudMachineDetails { machine, .. } => {
                            Some(Response::CloudMachineDetails(if machine == "m-1" {
                                instabox_details()
                            } else {
                                MachineDetails::default()
                            }))
                        }
                        Request::CloudMachineAction { action, .. } => match action {
                            MachineAction::SshCommand => {
                                Some(Response::CloudOutput("ssh m-1@ssh.instabox.dev".into()))
                            }
                            MachineAction::DesktopLink { .. } => Some(Response::CloudOutput(
                                "https://desktop.instabox.dev/m-1".into(),
                            )),
                            MachineAction::SetIdlePause(_) => None,
                            _ => Some(Response::Ok),
                        },
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
            fails_reads,
        }
    }

    pub(in crate::settings_page) fn actions(
        requests: &RefCell<Vec<Request>>,
    ) -> Vec<MachineAction> {
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

    struct Asked {
        confirms: Rc<RefCell<Vec<ConfirmRequest>>>,
        dialogs: Rc<Cell<usize>>,
    }

    fn asked(page: &Entity<SettingsPage>, cx: &mut VisualTestContext) -> Asked {
        let confirms: Rc<RefCell<Vec<ConfirmRequest>>> = Rc::default();
        let dialogs = Rc::new(Cell::new(0));
        cx.update(|_, cx| {
            let confirms = confirms.clone();
            let dialogs = dialogs.clone();
            cx.subscribe(page, move |_, event: &SettingsPageEvent, _| match event {
                SettingsPageEvent::Confirm(request) => confirms.borrow_mut().push(request.clone()),
                SettingsPageEvent::CloudMachineDialog(_) => dialogs.set(dialogs.get() + 1),
                _ => {}
            })
            .detach();
        });
        Asked { confirms, dialogs }
    }

    /// A machine's mark and name open its page, with a tab for each of its provider's
    /// extras, and Machines goes back. Each new listing reads it again, keeping what it
    /// showed when that fails, and a listing without it closes its page.
    #[gpui::test]
    fn a_cloud_machines_page_opens_from_its_row(cx: &mut TestAppContext) {
        let Served {
            client,
            requests,
            fails_reads,
        } = serve(cx);
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Machines, window, cx)
        });
        cx.run_until_parked();

        click("cloud-open-instabox-m-1", cx);
        assert!(cx.debug_bounds("machine-page-heading").is_some());
        assert_eq!(reads(&requests, "m-1"), 1);
        for tab in [
            "machine-tab-overview",
            "machine-tab-snapshots",
            "machine-tab-ports",
            "machine-tab-desktop",
            "machine-tab-vault",
            "machine-tab-ssh",
        ] {
            assert!(cx.debug_bounds(tab).is_some(), "{tab} is shown");
        }
        assert!(cx.debug_bounds("machine-tab-ssh-keys").is_none());
        for shown in [
            "machine-page-main",
            "machine-page-desktop",
            "machine-timeout",
            "machine-idle-pause",
        ] {
            assert!(cx.debug_bounds(shown).is_some(), "{shown} is shown");
        }
        // Instabox pauses its own; agentZ stops Proton's.
        assert!(cx.debug_bounds("machine-stop-when-idle").is_none());

        fails_reads.set(true);
        client.update(cx, |client, cx| {
            client.set_cloud_for_test(listing(Some(SystemTime::now()), vec![devbox()]), cx)
        });
        cx.run_until_parked();
        assert_eq!(reads(&requests, "m-1"), 2);
        assert!(
            cx.debug_bounds("machine-timeout").is_some(),
            "what it read before stays"
        );
        let error = page_error(&page, cx).unwrap_or_default();
        assert!(error.starts_with("Couldn't read it again: "), "{error}");
        assert!(cx.debug_bounds("machine-page-errors").is_some());
        fails_reads.set(false);

        click("machines-back", cx);
        assert!(cx.debug_bounds("machine-page-heading").is_none());
        assert!(cx.debug_bounds("cloud-row-instabox-m-1").is_some());

        // A Proton computer's page has SSH Keys, and no desktop.
        click("cloud-open-proton-c-1", cx);
        assert!(cx.debug_bounds("machine-tab-overview").is_some());
        assert!(cx.debug_bounds("machine-tab-ssh-keys").is_some());
        assert!(cx.debug_bounds("machine-stop-when-idle").is_some());
        for missing in [
            "machine-tab-snapshots",
            "machine-tab-ports",
            "machine-tab-desktop",
            "machine-tab-vault",
            "machine-tab-ssh",
            "machine-page-desktop",
            "machine-timeout",
        ] {
            assert!(cx.debug_bounds(missing).is_none(), "{missing} isn't shown");
        }

        client.update(cx, |client, cx| {
            client.set_cloud_for_test(listing(Some(SystemTime::now()), Vec::new()), cx)
        });
        cx.run_until_parked();
        assert!(page.read_with(cx, |page, _| page.machine_page.is_none()));
        assert!(cx.debug_bounds("cloud-row-instabox-m-1").is_some());
    }

    /// The tabs do the provider's extras: deleting a snapshot asks first and reads the
    /// machine again, Save Snapshot… opens its dialog, Unpublish unpublishes, the SSH
    /// command is copied and its button says so for a moment, Open Desktop opens its link,
    /// and what fails shows on the page.
    #[gpui::test]
    fn a_cloud_machines_page_does_its_providers_extras(cx: &mut TestAppContext) {
        let Served { requests, .. } = serve(cx);
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        let Asked { confirms, dialogs } = asked(&page, cx);
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Machines, window, cx);
            page.open_machine_page(Provider::Instabox, "m-1".into(), MachineTab::Snapshots, cx)
        });
        cx.run_until_parked();

        assert!(cx.debug_bounds("machine-snapshot-s-1").is_some());
        click("machine-snapshot-delete-s-1", cx);
        assert!(actions(&requests).is_empty());
        let confirm = confirms
            .borrow_mut()
            .pop()
            .expect("deleting a snapshot asks first");
        assert_eq!(confirm.title.as_ref(), "Delete dev-base?");
        cx.update(|window, cx| (confirm.on_confirm)(window, cx));
        cx.run_until_parked();
        assert_eq!(
            actions(&requests),
            [MachineAction::DeleteSnapshot("s-1".into())]
        );
        assert_eq!(reads(&requests, "m-1"), 2);

        click("machine-save-snapshot", cx);
        assert_eq!(dialogs.get(), 1);

        click("machine-tab-ports", cx);
        assert!(cx.debug_bounds("machine-port-3000").is_some());
        click("machine-port-unpublish-3000", cx);
        assert_eq!(
            actions(&requests).last(),
            Some(&MachineAction::UnpublishPort(3000))
        );

        click("machine-tab-ssh", cx);
        click("machine-ssh-command", cx);
        assert_eq!(
            cx.read_from_clipboard()
                .and_then(|item| item.text())
                .as_deref(),
            Some("ssh m-1@ssh.instabox.dev")
        );
        let copied = |cx: &mut VisualTestContext| {
            page.read_with(cx, |page, _| {
                page.is_copied(Provider::Instabox, "m-1", &MachineAction::SshCommand)
            })
        };
        assert!(copied(cx));
        cx.executor().advance_clock(COPIED_FOR);
        cx.run_until_parked();
        assert!(!copied(cx));

        click("machine-tab-desktop", cx);
        click("machine-desktop-open", cx);
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://desktop.instabox.dev/m-1")
        );

        page.update(cx, |page, cx| {
            page.send_machine_action(
                Provider::Instabox,
                "m-1".into(),
                MachineAction::SetIdlePause(Duration::from_secs(1800)),
                cx,
            )
        });
        cx.run_until_parked();
        assert_eq!(
            page_error(&page, cx).as_deref(),
            Some("Couldn't change when it pauses: not connected to This Mac")
        );
        assert!(cx.debug_bounds("machine-page-errors").is_some());
    }

    /// Replace Keys… takes public keys only, and warns when none is one of this Mac's own,
    /// which agentZ reaches the computer with.
    #[gpui::test]
    fn replacing_ssh_keys_takes_public_keys_and_warns_without_this_macs(cx: &mut TestAppContext) {
        let Served { requests, .. } = serve(cx);
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        let Asked { confirms, .. } = asked(&page, cx);
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
