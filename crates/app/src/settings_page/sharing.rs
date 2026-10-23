//! Settings' Shared (design/settings-2, topic 2): the machine picker of Thread titles, Skills,
//! MCP Servers and Storage starts with Shared, what every machine that doesn't set its own
//! takes. On Shared, a row some machine sets differently says "Set on Devbox 1", whose popover
//! lists each machine's value and opens the machines that set their own. On one machine, each
//! section starts with a "Use shared settings" switch: off, the machine keeps a copy to change;
//! back on, Shared's replace it.

use gpui::{AnyElement, App, Context, Window};
use ui::{ContextMenu, DropdownMenu, IconPosition, PopoverMenu, Switch, ToggleState, prelude::*};
use util::ResultExt as _;

use super::SettingsPage;
use super::render_row;
use super::skills::join_with_and;
use crate::machines::MachineId;
use crate::shared_settings::{SharedMachineSettings, SharedSection};

/// What a page that can share shows: Shared, or one machine's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MachineScope {
    Shared,
    Machine(MachineId),
}

/// A machine's line in a "Set on" popover.
#[derive(Clone)]
pub(super) struct SetOnEntry {
    pub machine: MachineId,
    pub label: SharedString,
    /// Its own value, or `None` while it follows Shared.
    pub value: Option<SharedString>,
    /// It sets its own, and it's not Shared's.
    pub differs: bool,
}

impl SettingsPage {
    pub(super) fn shared_settings<'a>(&self, cx: &'a App) -> &'a SharedMachineSettings {
        &self.app_settings.read(cx).settings().shared
    }

    /// Changes Shared, which the machines that follow it are then given.
    pub(super) fn update_shared(
        &self,
        change: impl FnOnce(&mut SharedMachineSettings),
        cx: &mut App,
    ) {
        self.app_settings.update(cx, |store, cx| {
            store.update(|settings| change(&mut settings.shared), cx)
        });
    }

    /// Thread titles' view: Shared or a machine's once there's more than this Mac, which alone
    /// changes its own (and Shared, while it follows it).
    pub(super) fn titles_view(&self, cx: &App) -> MachineScope {
        let machines = self.machines.read(cx);
        match self.titles_scope {
            _ if !machines.has_remotes() => MachineScope::Machine(MachineId::Local),
            MachineScope::Machine(machine) if machines.client(machine, cx).is_none() => {
                MachineScope::Shared
            }
            scope => scope,
        }
    }

    /// Storage's view, which opens on a machine, since its sizes are each machine's.
    pub(super) fn storage_view(&self, cx: &App) -> MachineScope {
        if !self.machines.read(cx).has_remotes() {
            MachineScope::Machine(MachineId::Local)
        } else if self.storage_shared {
            MachineScope::Shared
        } else {
            MachineScope::Machine(self.agents_machine)
        }
    }

    /// Skills' or MCP Servers' view. With only this Mac, Shared is still offered while it has
    /// any, since its items show on This Mac only to be changed there.
    pub(super) fn lists_view(&self, section: SharedSection, cx: &App) -> MachineScope {
        if !self.offers_shared_list(section, cx) {
            MachineScope::Machine(MachineId::Local)
        } else if self.lists_shared {
            MachineScope::Shared
        } else {
            MachineScope::Machine(self.agents_machine)
        }
    }

    pub(super) fn offers_shared_list(&self, section: SharedSection, cx: &App) -> bool {
        let shared = self.shared_settings(cx);
        self.machines.read(cx).has_remotes()
            || match section {
                SharedSection::Skills => !shared.skills.is_empty(),
                SharedSection::McpServers => !shared.mcp_servers.is_empty(),
                SharedSection::ThreadTitles | SharedSection::CleanupRules => false,
            }
    }

    /// Storage's picker: Shared's cleanup rules, or a machine's storage.
    pub(super) fn show_storage(&mut self, scope: MachineScope, cx: &mut Context<Self>) {
        self.storage_shared = scope == MachineScope::Shared;
        if let MachineScope::Machine(machine) = scope {
            self.set_agents_machine(machine, cx);
        }
        self.storage_page.error = None;
        cx.notify();
    }

    /// Skills' and MCP Servers' picker.
    pub(super) fn show_lists(&mut self, scope: MachineScope, cx: &mut Context<Self>) {
        self.lists_shared = scope == MachineScope::Shared;
        if let MachineScope::Machine(machine) = scope {
            self.set_agents_machine(machine, cx);
        }
        self.skill_error = None;
        self.mcp_server_error = None;
        cx.notify();
    }

    /// Whether the machine's section is its own. A machine not decided yet follows Shared.
    pub(super) fn sets_own(&self, machine: MachineId, section: SharedSection, cx: &App) -> bool {
        self.shared_settings(cx).sets_own(machine, section)
    }

    /// The machine picker with Shared at the top, over a line, as t3code's scope menu has All
    /// environments.
    pub(super) fn render_scope_picker(
        &self,
        id: &'static str,
        current: MachineScope,
        pick: fn(&mut SettingsPage, MachineScope, &mut Context<SettingsPage>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let machines: Vec<(MachineId, SharedString)> = self
            .machines
            .read(cx)
            .clients()
            .iter()
            .map(|client| (client.read(cx).machine(), client.read(cx).label().clone()))
            .collect();
        let (label, icon) = match current {
            MachineScope::Shared => (SharedString::from("Shared"), IconName::Layers),
            MachineScope::Machine(machine) => (
                self.machines.read(cx).label(machine, cx),
                self.machines.read(cx).machine_icon(machine, cx),
            ),
        };
        let this = cx.entity().downgrade();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            let choose = |scope: MachineScope| {
                let this = this.clone();
                move |_: &mut Window, cx: &mut App| {
                    this.update(cx, |this, cx| pick(this, scope, cx)).log_err();
                }
            };
            menu = menu.toggleable_entry(
                "Shared",
                current == MachineScope::Shared,
                IconPosition::End,
                None,
                choose(MachineScope::Shared),
            );
            menu = menu.separator();
            for (machine, label) in machines {
                let scope = MachineScope::Machine(machine);
                menu = menu.toggleable_entry(
                    label,
                    current == scope,
                    IconPosition::End,
                    None,
                    choose(scope),
                );
            }
            menu
        });
        let trigger = h_flex()
            .gap_1p5()
            .child(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
            .child(Label::new(label))
            .into_any_element();
        div()
            .debug_selector(move || id.to_string())
            .child(DropdownMenu::new_with_element(id, trigger, menu))
            .into_any_element()
    }

    /// The label of the form's machine, or Shared, where a picker would be.
    pub(super) fn render_scope_label(&self, scope: MachineScope, cx: &App) -> AnyElement {
        let (label, icon) = match scope {
            MachineScope::Shared => (SharedString::from("Shared"), IconName::Layers),
            MachineScope::Machine(machine) => (
                self.machines.read(cx).label(machine, cx),
                self.machines.read(cx).machine_icon(machine, cx),
            ),
        };
        h_flex()
            .gap_1p5()
            .child(Icon::new(icon).size(IconSize::Small).color(Color::Muted))
            .child(Label::new(label).color(Color::Muted))
            .into_any_element()
    }

    /// "Use shared settings", first in a machine's section: on, with what Shared has; off, the
    /// machine sets its own.
    pub(super) fn render_use_shared_row(
        &self,
        machine: MachineId,
        section: SharedSection,
        summary: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let follows = !self.sets_own(machine, section, cx);
        let description: SharedString = if follows {
            summary.into()
        } else {
            format!(
                "{} sets its own.",
                self.machines.read(cx).label(machine, cx)
            )
            .into()
        };
        let id = match section {
            SharedSection::ThreadTitles => "use-shared-titles",
            SharedSection::CleanupRules => "use-shared-cleanup",
            SharedSection::Skills => "use-shared-skills",
            SharedSection::McpServers => "use-shared-mcp-servers",
        };
        let page = cx.weak_entity();
        render_row(
            "Use shared settings",
            description,
            div()
                .debug_selector(move || id.to_string())
                .child(
                    Switch::new(id, follows.into()).on_click(move |state, _, cx| {
                        let follow = *state == ToggleState::Selected;
                        page.update(cx, |page, cx| {
                            page.update_shared(
                                |shared| shared.set_own(machine, section, !follow),
                                cx,
                            )
                        })
                        .log_err();
                    }),
                )
                .into_any_element(),
            cx,
        )
    }

    /// "Devbox 1 sets its own.", for a section on Shared.
    pub(super) fn own_note(&self, section: SharedSection, cx: &App) -> Option<String> {
        let shared = self.shared_settings(cx);
        let names: Vec<String> = self
            .machines
            .read(cx)
            .clients()
            .iter()
            .filter(|client| shared.sets_own(client.read(cx).machine(), section))
            .map(|client| client.read(cx).label().to_string())
            .collect();
        match names.len() {
            0 => None,
            1 => Some(format!("{} sets its own.", join_with_and(names))),
            _ => Some(format!("{} set their own.", join_with_and(names))),
        }
    }

    /// Each machine's line for a row on Shared: its value where it sets its own.
    pub(super) fn set_on_entries(
        &self,
        section: SharedSection,
        value: impl Fn(&crate::server_client::ServerClient) -> Option<(String, SharedString)>,
        shared_key: &str,
        cx: &App,
    ) -> Vec<SetOnEntry> {
        let shared = self.shared_settings(cx);
        self.machines
            .read(cx)
            .clients()
            .iter()
            .map(|client| {
                let client = client.read(cx);
                let own = shared.sets_own(client.machine(), section);
                let value = own.then(|| value(client)).flatten();
                SetOnEntry {
                    machine: client.machine(),
                    label: client.label().clone(),
                    differs: value.as_ref().is_some_and(|(key, _)| key != shared_key),
                    value: value.map(|(_, shown)| shown),
                }
            })
            .collect()
    }

    /// Zed's "Modified in" after a row's title: "Set on Devbox 1", or how many machines, with
    /// t3code's popover of where the value comes from. A machine's line opens its own.
    pub(super) fn render_set_on(
        &self,
        id: &str,
        entries: Vec<SetOnEntry>,
        open: fn(&mut SettingsPage, MachineId, &mut Context<SettingsPage>),
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let differing: Vec<&SetOnEntry> = entries.iter().filter(|entry| entry.differs).collect();
        let label = match differing.as_slice() {
            [] => return None,
            [entry] => format!("Set on {}", entry.label),
            many => format!("Set on {} machines", many.len()),
        };
        let page = cx.weak_entity();
        let selector = format!("set-on-{id}");
        let trigger_id = SharedString::from(format!("set-on-{id}"));
        Some(
            div()
                .debug_selector(move || selector)
                .child(
                    PopoverMenu::new(SharedString::from(format!("set-on-menu-{id}")))
                        .menu(move |window, cx| {
                            let entries = entries.clone();
                            let page = page.clone();
                            Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                                for entry in &entries {
                                    let label = entry.label.clone();
                                    let value =
                                        entry.value.clone().unwrap_or_else(|| "Shared".into());
                                    let selector = format!("set-on-machine-{label}");
                                    let render = move |_: &mut Window, _: &mut App| {
                                        let selector = selector.clone();
                                        h_flex()
                                            .debug_selector(move || selector)
                                            .w_full()
                                            .gap_6()
                                            .justify_between()
                                            .child(Label::new(label.clone()))
                                            .child(Label::new(value.clone()).color(Color::Muted))
                                            .into_any_element()
                                    };
                                    if entry.value.is_some() {
                                        let page = page.clone();
                                        let machine = entry.machine;
                                        menu = menu.custom_entry(render, move |_, cx| {
                                            page.update(cx, |page, cx| open(page, machine, cx))
                                                .log_err();
                                        });
                                    } else {
                                        menu = menu.custom_row(render);
                                    }
                                }
                                menu
                            }))
                        })
                        .trigger(
                            Button::new(trigger_id, label)
                                .style(ButtonStyle::Transparent)
                                .size(ButtonSize::None)
                                .label_size(LabelSize::Small)
                                .color(Color::Accent),
                        )
                        .anchor(gpui::Anchor::TopLeft)
                        .offset(gpui::point(px(0.), px(4.))),
                )
                .into_any_element(),
        )
    }
}

/// "1 shared skill", "3 shared servers".
pub(super) fn count_summary(count: usize, one: &str, many: &str) -> String {
    match count {
        0 => format!("No {many} yet."),
        1 => format!("1 {one}."),
        count => format!("{count} {many}."),
    }
}

#[cfg(test)]
pub(super) mod tests {
    use agentz_protocol::storage::Storage;
    use agentz_protocol::title_generation::{
        TitleGeneration, TitleGenerationState, TitleModel, TitleProvider, TitleProviderInfo,
    };
    use gpui::{TestAppContext, VisualTestContext};

    use super::super::Section;
    use super::super::tests::machines_with_copies;
    use super::*;
    use crate::app_settings::AppSettingsStore;

    pub(in super::super) const DEVBOX: MachineId = MachineId::Remote(1);

    pub(in super::super) fn click(selector: &'static str, cx: &mut VisualTestContext) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is shown"));
        cx.simulate_click(bounds.center(), gpui::Modifiers::none());
        cx.run_until_parked();
    }

    pub(in super::super) fn shared(cx: &mut VisualTestContext) -> SharedMachineSettings {
        cx.update(|_, cx| {
            AppSettingsStore::global(cx)
                .read(cx)
                .settings()
                .shared
                .clone()
        })
    }

    pub(in super::super) fn update_shared(
        change: impl FnOnce(&mut SharedMachineSettings),
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            AppSettingsStore::global(cx).update(cx, |store, cx| {
                store.update(|settings| change(&mut settings.shared), cx)
            })
        });
    }

    /// Thread titles open on Shared, whose rows change Shared, and a row Devbox 1 sets
    /// differently says so. Its popover opens Devbox 1, whose switch makes it use Shared's.
    #[gpui::test]
    fn thread_titles_open_on_shared_and_say_where_machines_differ(cx: &mut TestAppContext) {
        let (local, devbox) = machines_with_copies(cx);
        let providers = vec![
            TitleProviderInfo {
                provider: TitleProvider::Codex,
                installed: true,
                models: vec![TitleModel {
                    id: "gpt-mini".into(),
                    name: "GPT-Mini".into(),
                    efforts: Vec::new(),
                }],
            },
            TitleProviderInfo {
                provider: TitleProvider::Claude,
                installed: true,
                models: Vec::new(),
            },
        ];
        let shared_titles = TitleGeneration {
            enabled: true,
            ..TitleGeneration::default()
        };
        let devbox_titles = TitleGeneration {
            provider: TitleProvider::Claude,
            ..shared_titles.clone()
        };
        cx.update(|cx| {
            for (client, settings) in [
                (&local, shared_titles.clone()),
                (&devbox, devbox_titles.clone()),
            ] {
                client.update(cx, |client, cx| {
                    client.set_title_generation_for_test(
                        TitleGenerationState {
                            settings,
                            providers: providers.clone(),
                        },
                        cx,
                    )
                });
            }
        });
        update_shared(
            |shared| {
                shared.title_generation = Some(shared_titles.clone());
                shared.set_own(MachineId::Local, SharedSection::ThreadTitles, false);
                shared.set_own(DEVBOX, SharedSection::ThreadTitles, true);
            },
            cx,
        );
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::ThreadTitles, window, cx)
        });
        cx.run_until_parked();

        // Only the provider differs on Devbox 1.
        assert!(cx.debug_bounds("titles-machine").is_some());
        assert!(cx.debug_bounds("use-shared-titles").is_none());
        assert!(cx.debug_bounds("set-on-generate-titles").is_none());
        assert!(cx.debug_bounds("set-on-title-provider").is_some());
        click("title-provider", cx);
        click("MENU_ITEM-Claude", cx);
        assert_eq!(shared(cx).title_generation, Some(devbox_titles));
        assert!(cx.debug_bounds("set-on-title-provider").is_none());
        // Shared isn't a machine's own: This Mac takes it once its session has arrived.
        assert!(local.read_with(cx, |client, _| client.sent_for_test().is_empty()));

        click("title-provider", cx);
        click("MENU_ITEM-Codex", cx);
        click("set-on-title-provider", cx);
        click("set-on-machine-Devbox 1", cx);
        assert_eq!(
            page.read_with(cx, |page, cx| page.titles_view(cx)),
            MachineScope::Machine(DEVBOX)
        );
        assert!(cx.debug_bounds("title-provider").is_some());
        click("use-shared-titles", cx);
        assert!(!shared(cx).sets_own(DEVBOX, SharedSection::ThreadTitles));
        assert!(cx.debug_bounds("title-provider").is_none());
        click("use-shared-titles", cx);
        assert!(shared(cx).sets_own(DEVBOX, SharedSection::ThreadTitles));
        assert!(cx.debug_bounds("title-provider").is_some());
    }

    /// Storage opens on a machine, which uses Shared's cleanup rules unless it sets its own.
    /// Shared has only the rules, since sizes and Clean Up Now are each machine's.
    #[gpui::test]
    fn storage_on_shared_has_only_the_cleanup_rules(cx: &mut TestAppContext) {
        let (local, devbox) = machines_with_copies(cx);
        cx.update(|cx| {
            let mut storage = Storage {
                measured: true,
                ..Storage::default()
            };
            local.update(cx, |client, cx| {
                client.set_storage_for_test(storage.clone(), cx)
            });
            storage.cleanup.rules.merged = true;
            devbox.update(cx, |client, cx| client.set_storage_for_test(storage, cx));
        });
        update_shared(
            |shared| {
                shared.cleanup_rules = Some(Default::default());
                shared.set_own(MachineId::Local, SharedSection::CleanupRules, false);
                shared.set_own(DEVBOX, SharedSection::CleanupRules, true);
            },
            cx,
        );
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        cx.simulate_resize(gpui::size(px(1100.), px(2000.)));
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Storage, window, cx)
        });
        cx.run_until_parked();

        assert!(cx.debug_bounds("storage-summary").is_some());
        assert!(cx.debug_bounds("use-shared-cleanup").is_some());
        assert!(cx.debug_bounds("cleanup-deleted").is_none());
        assert!(cx.debug_bounds("cleanup-now").is_some());

        page.update(cx, |page, cx| page.show_storage(MachineScope::Shared, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("storage-summary").is_none());
        assert!(cx.debug_bounds("cleanup-now").is_none());
        assert!(cx.debug_bounds("set-on-cleanup-merged").is_some());
        assert!(cx.debug_bounds("set-on-cleanup-deleted").is_none());
        click("cleanup-deleted", cx);
        assert!(shared(cx).cleanup_rules.is_some_and(|rules| rules.deleted));
        assert!(!local.read_with(cx, |client, _| client.storage().cleanup.rules.deleted));
    }
}
