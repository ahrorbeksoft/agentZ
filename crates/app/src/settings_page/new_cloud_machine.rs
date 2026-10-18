//! A provider's New Machine dialog (design/cloud topic 4 C), from Add Machine's menu or the
//! provider's group: the account, what the machine starts from (Instabox's templates or the
//! account's snapshots), its name, and Instabox's timeout and idle pause; Proton's says what
//! each of its computers is and costs. Create hands the machine to the settings page, which
//! asks first when making it charges (topic 5 C), makes it, and saves it once it's made.

use std::time::Duration;

use agentz_protocol::cloud::{
    AccountRules, CloudSources, CloudUsage, CreateMachine, MachineSource, MachineTimeout, Provider,
    ProviderStatus,
};
use agentz_protocol::{Request, Response};
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, SharedString,
    Subscription, Task, WeakEntity, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{
    ContextMenu, DropdownMenu, DropdownStyle, IconPosition, ToggleButtonGroup,
    ToggleButtonGroupStyle, ToggleButtonSimple, prelude::*,
};
use util::ResultExt as _;

use super::SettingsPage;
use super::cloud_providers::signed_in_account;
use crate::app_settings::AppSettingsStore;
use crate::cloud_machines::{charge_message, noun_title, provider_icon};
use crate::machines::Machines;

pub(super) const KEY_CONTEXT: &str = "NewCloudMachine";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartFrom {
    Template,
    Snapshot,
}

/// "Pause or delete it after".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Timeout {
    Never,
    After { minutes: u64, delete: bool },
}

impl Timeout {
    const ALL: [Timeout; 7] = [
        Timeout::Never,
        Timeout::After {
            minutes: 30,
            delete: false,
        },
        Timeout::After {
            minutes: 60,
            delete: false,
        },
        Timeout::After {
            minutes: 240,
            delete: false,
        },
        Timeout::After {
            minutes: 30,
            delete: true,
        },
        Timeout::After {
            minutes: 60,
            delete: true,
        },
        Timeout::After {
            minutes: 240,
            delete: true,
        },
    ];

    fn label(self) -> String {
        match self {
            Timeout::Never => "Never".into(),
            Timeout::After { minutes, delete } => format!(
                "{} after {}",
                if delete { "Delete" } else { "Pause" },
                minutes_label(minutes)
            ),
        }
    }

    fn timeout(self) -> Option<MachineTimeout> {
        match self {
            Timeout::Never => None,
            Timeout::After { minutes, delete } => Some(MachineTimeout {
                after: Duration::from_secs(minutes * 60),
                delete,
            }),
        }
    }

    fn is_allowed(self, rules: &AccountRules) -> bool {
        match (self.timeout(), rules.max_timeout) {
            (Some(timeout), Some(max)) => timeout.after <= max,
            _ => true,
        }
    }
}

/// "When idle": the provider's own idle pause, a longer one, or none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum IdlePause {
    /// Instabox's, after 30 minutes.
    Default,
    After {
        minutes: u64,
    },
    Never,
}

impl IdlePause {
    const ALL: [IdlePause; 4] = [
        IdlePause::Default,
        IdlePause::After { minutes: 60 },
        IdlePause::After { minutes: 240 },
        IdlePause::Never,
    ];

    fn label(self) -> String {
        match self {
            IdlePause::Default => "Pause after 30 min".into(),
            IdlePause::After { minutes } => format!("Pause after {}", minutes_label(minutes)),
            IdlePause::Never => "Never pause".into(),
        }
    }

    fn auto_suspend(self) -> Option<Duration> {
        match self {
            IdlePause::Default => None,
            IdlePause::After { minutes } => Some(Duration::from_secs(minutes * 60)),
            IdlePause::Never => Some(Duration::ZERO),
        }
    }
}

fn minutes_label(minutes: u64) -> String {
    match minutes {
        60 => "1 hour".into(),
        minutes if minutes % 60 == 0 => format!("{} hours", minutes / 60),
        minutes => format!("{minutes} min"),
    }
}

pub struct NewCloudMachine {
    provider: Provider,
    page: WeakEntity<SettingsPage>,
    name_input: Entity<TextInput>,
    /// What it can start from, once read; why it couldn't be read otherwise.
    sources: Option<Result<CloudSources, SharedString>>,
    start_from: StartFrom,
    template: Option<String>,
    snapshot: Option<String>,
    timeout: Timeout,
    idle_pause: IdlePause,
    error: Option<SharedString>,
    _reading_sources: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for NewCloudMachine {}

impl Focusable for NewCloudMachine {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.name_input.focus_handle(cx)
    }
}

impl NewCloudMachine {
    pub(super) fn new(
        provider: Provider,
        page: WeakEntity<SettingsPage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name_input = cx.new(|cx| TextInput::new("Lowercase letters, digits and -", cx));
        let suggested = suggested_name(provider, cx);
        name_input.update(cx, |input, cx| input.set_text(suggested, cx));
        let subscriptions = vec![
            cx.subscribe(&name_input, |this, _, _: &TextInputEvent, cx| {
                this.error = None;
                cx.notify();
            }),
        ];
        window.focus(&name_input.focus_handle(cx), cx);
        let capabilities = &provider.description().capabilities;
        let reading_sources = (capabilities.templates || capabilities.snapshots).then(|| {
            let request = Machines::local(cx)
                .read(cx)
                .request(Request::CloudSources(provider));
            cx.spawn(async move |this, cx| {
                let sources = match request.await {
                    Ok(Response::CloudSources(sources)) => Ok(sources),
                    Ok(_) => Err("Couldn't read what it starts from.".into()),
                    Err(error) => {
                        Err(format!("Couldn't read what it starts from: {error:#}").into())
                    }
                };
                this.update(cx, |this, cx| this.set_sources(sources, cx))
                    .log_err();
            })
        });
        Self {
            provider,
            page,
            name_input,
            sources: None,
            start_from: StartFrom::Template,
            template: None,
            snapshot: None,
            timeout: Timeout::Never,
            idle_pause: IdlePause::Default,
            error: None,
            _reading_sources: reading_sources,
            _subscriptions: subscriptions,
        }
    }

    /// The templates and snapshots read, with the usual template picked: Instabox's
    /// `universal`, or the first.
    fn set_sources(&mut self, sources: Result<CloudSources, SharedString>, cx: &mut Context<Self>) {
        if let Ok(sources) = &sources {
            self.template = sources
                .templates
                .iter()
                .find(|template| template.id == "universal")
                .or(sources.templates.first())
                .map(|template| template.id.clone());
            self.snapshot = sources
                .snapshots
                .iter()
                .find(|snapshot| snapshot.is_ready)
                .map(|snapshot| snapshot.id.clone());
        }
        self.sources = Some(sources);
        cx.notify();
    }

    /// What the provider lets this account's machines change.
    fn rules(&self, cx: &App) -> AccountRules {
        match Machines::local(cx)
            .read(cx)
            .cloud()
            .provider(self.provider)
            .map(|state| &state.status)
        {
            Some(ProviderStatus::SignedIn { rules, .. }) => rules.clone(),
            _ => AccountRules::default(),
        }
    }

    /// What's being read before Create can make it.
    fn is_reading(&self) -> bool {
        self._reading_sources.is_some() && self.sources.is_none()
    }

    /// The machine as the form describes it, or why it can't be made.
    fn machine(&self, cx: &App) -> Result<CreateMachine, SharedString> {
        let description = self.provider.description();
        let name = self.name_input.read(cx).text().trim().to_string();
        validate_name(&name)?;
        let is_taken = Machines::local(cx)
            .read(cx)
            .cloud()
            .provider(self.provider)
            .is_some_and(|state| state.machines.iter().any(|machine| machine.name == name));
        if is_taken {
            return Err(format!(
                "{} already has a {} called {name}.",
                description.name, description.noun
            )
            .into());
        }
        let source = match self.start_from {
            StartFrom::Template => self.template.clone().map(MachineSource::Template),
            StartFrom::Snapshot => Some(MachineSource::Snapshot(
                self.snapshot
                    .clone()
                    .ok_or("Pick a snapshot to start from.")?,
            )),
        };
        let has_timeouts = description.capabilities.timeouts;
        let rules = self.rules(cx);
        Ok(CreateMachine {
            name,
            source,
            timeout: self
                .timeout
                .timeout()
                .filter(|_| has_timeouts && self.timeout.is_allowed(&rules)),
            auto_suspend: self
                .idle_pause
                .auto_suspend()
                .filter(|_| has_timeouts && rules.fixed_idle_pause.is_none()),
        })
    }

    fn confirm(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_reading() {
            return;
        }
        let machine = match self.machine(cx) {
            Ok(machine) => machine,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        let provider = self.provider;
        let dialog = cx.weak_entity();
        let is_making = self
            .page
            .update(cx, |page, cx| {
                page.create_cloud_machine(provider, machine, dialog, window, cx)
            })
            .log_err()
            .unwrap_or(false);
        if is_making {
            cx.emit(DismissEvent);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn render_start_from(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.weak_entity();
        let pick = move |start_from: StartFrom| {
            let this = this.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                this.update(cx, |this, cx| {
                    this.start_from = start_from;
                    this.error = None;
                    cx.notify();
                })
                .log_err();
            }
        };
        let segments = div()
            .debug_selector(|| "new-cloud-machine-start-from".into())
            .child(
                ToggleButtonGroup::single_row(
                    "new-cloud-machine-start-from",
                    [
                        ToggleButtonSimple::new("Template", pick(StartFrom::Template)),
                        ToggleButtonSimple::new("Snapshot", pick(StartFrom::Snapshot)),
                    ],
                )
                .style(ToggleButtonGroupStyle::Outlined)
                .label_size(LabelSize::Small)
                .auto_width()
                .selected_index(match self.start_from {
                    StartFrom::Template => 0,
                    StartFrom::Snapshot => 1,
                }),
            );
        let sources = self
            .sources
            .as_ref()
            .and_then(|sources| sources.as_ref().ok());
        let (label, choices): (String, Vec<(String, String)>) = match self.start_from {
            StartFrom::Template => {
                let choices: Vec<(String, String)> = sources
                    .map(|sources| {
                        sources
                            .templates
                            .iter()
                            .map(|template| {
                                let label = match &template.description {
                                    Some(description) => {
                                        format!("{} — {description}", template.id)
                                    }
                                    None => template.id.clone(),
                                };
                                (template.id.clone(), label)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let label = choices
                    .iter()
                    .find(|(id, _)| Some(id) == self.template.as_ref())
                    .map(|(_, label)| label.clone());
                (self.choice_label(label, "No templates"), choices)
            }
            StartFrom::Snapshot => {
                let choices: Vec<(String, String)> = sources
                    .map(|sources| {
                        sources
                            .snapshots
                            .iter()
                            .filter(|snapshot| snapshot.is_ready)
                            .map(|snapshot| {
                                let label = match snapshot.created_at {
                                    Some(at) => format!(
                                        "{} · {}",
                                        snapshot.name,
                                        chrono::DateTime::<chrono::Local>::from(at)
                                            .format("%b %-d")
                                    ),
                                    None => snapshot.name.clone(),
                                };
                                (snapshot.id.clone(), label)
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let label = choices
                    .iter()
                    .find(|(id, _)| Some(id) == self.snapshot.as_ref())
                    .map(|(_, label)| label.clone());
                (self.choice_label(label, "No snapshots yet"), choices)
            }
        };
        let is_empty = choices.is_empty();
        let start_from = self.start_from;
        let selected = match start_from {
            StartFrom::Template => self.template.clone(),
            StartFrom::Snapshot => self.snapshot.clone(),
        };
        let this = cx.weak_entity();
        let menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for (id, label) in choices {
                let this = this.clone();
                let is_selected = selected.as_ref() == Some(&id);
                menu = menu.toggleable_entry(
                    label,
                    is_selected,
                    IconPosition::End,
                    None,
                    move |_, cx| {
                        let id = id.clone();
                        this.update(cx, |this, cx| {
                            match start_from {
                                StartFrom::Template => this.template = Some(id),
                                StartFrom::Snapshot => this.snapshot = Some(id),
                            }
                            this.error = None;
                            cx.notify();
                        })
                        .log_err();
                    },
                );
            }
            menu
        });
        h_flex()
            .gap_2()
            .child(segments)
            .child(
                div().flex_1().min_w_0().child(
                    DropdownMenu::new("new-cloud-machine-source", label, menu)
                        .style(DropdownStyle::Outlined)
                        .full_width(true)
                        .disabled(is_empty),
                ),
            )
            .into_any_element()
    }

    /// A dropdown's label: what's picked, or what's missing.
    fn choice_label(&self, picked: Option<String>, none: &str) -> String {
        match (&self.sources, picked) {
            (None, _) => "Loading…".into(),
            (Some(Err(_)), _) => "The provider's default".into(),
            (Some(Ok(_)), Some(picked)) => picked,
            (Some(Ok(_)), None) => none.into(),
        }
    }

    fn render_timeouts(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let rules = self.rules(cx);
        let this = cx.weak_entity();
        let current = self.timeout;
        let allowed: Vec<Timeout> = Timeout::ALL
            .into_iter()
            .filter(|value| value.is_allowed(&rules))
            .collect();
        let timeout_menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            let mut is_deleting = false;
            for value in allowed {
                // Pausing, then deleting.
                if matches!(value, Timeout::After { delete: true, .. }) && !is_deleting {
                    is_deleting = true;
                    menu = menu.separator();
                }
                let this = this.clone();
                menu = menu.toggleable_entry(
                    value.label(),
                    current == value,
                    IconPosition::End,
                    None,
                    move |_, cx| {
                        this.update(cx, |this, cx| {
                            this.timeout = value;
                            cx.notify();
                        })
                        .log_err();
                    },
                );
            }
            menu
        });
        let this = cx.weak_entity();
        let current = self.idle_pause;
        let idle_menu = ContextMenu::build(window, cx, move |mut menu, _, _| {
            for value in IdlePause::ALL {
                let this = this.clone();
                menu = menu.toggleable_entry(
                    value.label(),
                    current == value,
                    IconPosition::End,
                    None,
                    move |_, cx| {
                        this.update(cx, |this, cx| {
                            this.idle_pause = value;
                            cx.notify();
                        })
                        .log_err();
                    },
                );
            }
            menu
        });
        let name = self.provider.description().name;
        let idle_hint = match (rules.fixed_idle_pause, self.idle_pause) {
            (Some(_), _) => format!(
                "{name} sets this for your account. Idle: no agent working, no terminal use, little CPU."
            ),
            (None, IdlePause::Default) => {
                format!("{name}’s default. Idle: no agent working, no terminal use, little CPU.")
            }
            (None, _) => "Idle: no agent working, no terminal use, little CPU.".into(),
        };
        let idle_label = match rules.fixed_idle_pause {
            Some(after) => format!("Pause after {}", minutes_label(after.as_secs() / 60)),
            None => self.idle_pause.label(),
        };
        h_flex()
            .gap_3()
            .items_start()
            .child(
                div().flex_1().min_w_0().child(render_field(
                    "Pause or delete it after",
                    DropdownMenu::new(
                        "new-cloud-machine-timeout",
                        self.timeout.label(),
                        timeout_menu,
                    )
                    .style(DropdownStyle::Outlined)
                    .full_width(true)
                    .into_any_element(),
                    None,
                )),
            )
            .child(
                div().flex_1().min_w_0().child(render_field(
                    "When idle",
                    DropdownMenu::new("new-cloud-machine-idle", idle_label, idle_menu)
                        .style(DropdownStyle::Outlined)
                        .full_width(true)
                        .disabled(rules.fixed_idle_pause.is_some())
                        .into_any_element(),
                    Some(idle_hint),
                )),
            )
            .into_any_element()
    }
}

impl Render for NewCloudMachine {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let provider = self.provider;
        let description = provider.description();
        let colors = cx.theme().colors().clone();
        let state = Machines::local(cx)
            .read(cx)
            .cloud()
            .provider(provider)
            .cloned();
        let account = signed_in_account(state.as_ref());
        let usage = state.as_ref().and_then(|state| state.usage.as_ref());
        let asks = account.as_ref().is_none_or(|account| {
            !AppSettingsStore::global(cx)
                .read(cx)
                .settings()
                .charges_without_asking(provider, account)
        });
        let create_label = match description.charge {
            Some(_) if !asks => format!("Create · {}", description.price),
            _ => format!("Create {}", noun_title(provider)),
        };
        let note: Option<(SharedString, Color)> = match (&self.error, &self.sources) {
            (Some(error), _) => Some((error.clone(), Color::Error)),
            (None, Some(Err(error))) => Some((error.clone(), Color::Error)),
            _ => match usage {
                Some(CloudUsage::Limits {
                    machines,
                    max_machines: Some(max),
                    ..
                }) => Some((
                    format!("{}: {machines} of {max} machines used.", description.price).into(),
                    Color::Muted,
                )),
                _ => None,
            },
        };
        let account_box = h_flex()
            .h(px(28.))
            .px_2()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .child(
                Icon::new(provider_icon(provider))
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Label::new(match &account {
                    Some(account) => format!("{} · {account}", description.name),
                    None => format!("{} isn't signed in", description.name),
                })
                .size(LabelSize::Small),
            )
            .into_any_element();
        let name_box = div()
            .h(px(28.))
            .px_2()
            .flex()
            .items_center()
            .overflow_hidden()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.editor_background)
            .child(self.name_input.clone())
            .into_any_element();
        let capabilities = &description.capabilities;
        let what_it_is = description.new_machine.map(|what| {
            v_flex()
                .px_3()
                .py_2()
                .gap_1()
                .rounded_md()
                .border_1()
                .border_color(colors.border_variant)
                .child(Label::new(what).size(LabelSize::Small))
                .children(description.charge.map(|charge| {
                    Label::new(charge_message(charge, usage))
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                }))
        });
        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(34.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(
                h_flex()
                    .px_4()
                    .pt_3()
                    .pb_2()
                    .gap_2()
                    .child(
                        Icon::new(provider_icon(provider))
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(Label::new(format!(
                        "New {} {}",
                        description.name,
                        noun_title(provider)
                    ))),
            )
            .child(
                v_flex()
                    .px_4()
                    .py_2()
                    .gap_3()
                    .child(render_field("Account", account_box, None))
                    .when(capabilities.templates || capabilities.snapshots, |body| {
                        body.child(render_field(
                            "Start from",
                            self.render_start_from(window, cx),
                            None,
                        ))
                    })
                    .child(render_field(
                        "Name",
                        name_box,
                        Some("Lowercase letters, digits and -.".into()),
                    ))
                    .when(capabilities.timeouts, |body| {
                        body.child(self.render_timeouts(window, cx))
                    })
                    .children(what_it_is),
            )
            .child(
                h_flex()
                    .mt_2()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(colors.border_variant)
                    .child(div().flex_1().min_w_0().children(
                        note.map(|(note, color)| {
                            Label::new(note).size(LabelSize::Small).color(color)
                        }),
                    ))
                    .child(
                        Button::new("new-cloud-machine-cancel", "Cancel")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        div()
                            .debug_selector(|| "new-cloud-machine-create".into())
                            .child(
                                Button::new("new-cloud-machine-create", create_label)
                                    .style(ButtonStyle::Filled)
                                    .disabled(account.is_none() || self.is_reading())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm(&menu::Confirm, window, cx)
                                    })),
                            ),
                    ),
            )
    }
}

fn render_field(title: &'static str, field: AnyElement, hint: Option<String>) -> AnyElement {
    v_flex()
        .gap_1()
        .child(Label::new(title).size(LabelSize::Small))
        .child(field)
        .children(hint.map(|hint| Label::new(hint).size(LabelSize::Small).color(Color::Muted)))
        .into_any_element()
}

/// Names both providers take: lowercase letters, digits and dashes, not at either end.
fn validate_name(name: &str) -> Result<(), SharedString> {
    if name.is_empty() {
        return Err("Give it a name.".into());
    }
    let is_valid = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-');
    if is_valid {
        Ok(())
    } else {
        Err("Use lowercase letters, digits and -.".into())
    }
}

/// `agents-1`, or the first number the account's machines don't have.
fn suggested_name(provider: Provider, cx: &App) -> String {
    let cloud = Machines::local(cx).read(cx).cloud();
    let taken: Vec<&str> = cloud
        .provider(provider)
        .map(|state| {
            state
                .machines
                .iter()
                .map(|machine| machine.name.as_str())
                .collect()
        })
        .unwrap_or_default();
    (1..)
        .map(|number| format!("agents-{number}"))
        .find(|name| !taken.contains(&name.as_str()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use agentz_protocol::cloud::{
        CloudSnapshot, CloudState, CloudTemplate, MachineSize, MachineSnapshot, ProviderStatus,
    };
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::TestAppContext;

    use super::super::SettingsPageEvent;
    use super::super::tests::{cloud_machine, provider_state};
    use super::*;
    use crate::confirm_dialog::ConfirmRequest;
    use crate::machines::MachineId;
    use crate::server_client::ServerClient;

    struct Opened {
        page: Entity<SettingsPage>,
        requests: Rc<RefCell<Vec<Request>>>,
        confirms: Rc<RefCell<Vec<ConfirmRequest>>>,
    }

    /// The settings page over a test server: Instabox signed in with `agents-1`, two
    /// templates and a snapshot; Proton signed in with `devbox-3` and a balance. Making a
    /// machine gives it back as `m-new`.
    fn open(cx: &mut TestAppContext) -> Opened {
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let page = cx.update(|cx| {
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
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    match request {
                        Request::CloudSources(Provider::Instabox) => {
                            Some(Response::CloudSources(CloudSources {
                                templates: vec![
                                    CloudTemplate {
                                        id: "agent-browser".into(),
                                        description: None,
                                        size: MachineSize::default(),
                                    },
                                    CloudTemplate {
                                        id: "universal".into(),
                                        description: Some("Docker, Python, Go, Rust".into()),
                                        size: MachineSize::default(),
                                    },
                                ],
                                snapshots: vec![MachineSnapshot {
                                    id: "s-1".into(),
                                    name: "dev-base".into(),
                                    is_ready: true,
                                    source_machine: None,
                                    created_at: None,
                                    software_only: false,
                                    size_bytes: None,
                                }],
                            }))
                        }
                        Request::CreateCloudMachine { machine, .. } => {
                            Some(Response::CloudMachineCreated(cloud_machine(
                                "m-new",
                                &machine.name,
                                CloudState::Running,
                            )))
                        }
                        _ => None,
                    }
                });
                let signed_in = || ProviderStatus::SignedIn {
                    version: "1.0.0".into(),
                    account: "me@example.com".into(),
                    rules: AccountRules::default(),
                };
                let mut instabox = provider_state(Provider::Instabox, signed_in());
                instabox.machines = vec![cloud_machine("m-1", "agents-1", CloudState::Running)];
                instabox.usage = Some(CloudUsage::Limits {
                    machines: 1,
                    max_machines: Some(5),
                    snapshots: 1,
                    max_snapshots: Some(10),
                    stored_bytes: None,
                    max_stored_gib: None,
                });
                let mut proton = provider_state(Provider::Proton, signed_in());
                proton.machines = vec![cloud_machine("c-1", "devbox-3", CloudState::Running)];
                proton.usage = Some(CloudUsage::Balance {
                    balance_micro_usd: Some(14_200_000),
                    running_hours: 3.,
                    charged_micro_usd: 0,
                });
                client.set_cloud_for_test(
                    CloudSnapshot {
                        providers: vec![instabox, proton],
                        checking: false,
                    },
                    cx,
                );
            });
            crate::machines::init_for_test(vec![client], cx);
            cx.new(SettingsPage::new)
        });
        let confirms: Rc<RefCell<Vec<ConfirmRequest>>> = Rc::default();
        cx.update(|cx| {
            let confirms = confirms.clone();
            cx.subscribe(&page, move |_, event: &SettingsPageEvent, _| {
                if let SettingsPageEvent::Confirm(request) = event {
                    confirms.borrow_mut().push(request.clone());
                }
            })
            .detach();
        });
        Opened {
            page,
            requests,
            confirms,
        }
    }

    fn creates(requests: &RefCell<Vec<Request>>) -> Vec<(Provider, CreateMachine)> {
        requests
            .borrow()
            .iter()
            .filter_map(|request| match request {
                Request::CreateCloudMachine { provider, machine } => {
                    Some((*provider, machine.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn dismissals(
        dialog: &Entity<NewCloudMachine>,
        cx: &mut gpui::VisualTestContext,
    ) -> Rc<RefCell<usize>> {
        let dismissed: Rc<RefCell<usize>> = Rc::default();
        cx.update(|_, cx| {
            let dismissed = dismissed.clone();
            cx.subscribe(dialog, move |_, _: &DismissEvent, _| {
                *dismissed.borrow_mut() += 1
            })
            .detach();
        });
        dismissed
    }

    /// Instabox's dialog reads its templates and snapshots, picks `universal` and a free
    /// name, and makes the machine as picked; once made, it's saved and switched on. A name
    /// Instabox doesn't take is refused before anything is made.
    #[gpui::test]
    fn makes_an_instabox_machine_as_picked_and_saves_it(cx: &mut TestAppContext) {
        let Opened { page, requests, .. } = open(cx);
        let (dialog, cx) = cx.add_window_view(|window, cx| {
            NewCloudMachine::new(Provider::Instabox, page.downgrade(), window, cx)
        });
        cx.run_until_parked();
        let dismissed = dismissals(&dialog, cx);
        dialog.read_with(cx, |dialog, cx| {
            assert_eq!(dialog.template.as_deref(), Some("universal"));
            assert_eq!(dialog.snapshot.as_deref(), Some("s-1"));
            assert_eq!(dialog.name_input.read(cx).text().as_ref(), "agents-2");
        });
        let click_create = |cx: &mut gpui::VisualTestContext| {
            let create = cx
                .debug_bounds("new-cloud-machine-create")
                .expect("Create is shown");
            cx.simulate_click(create.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };

        dialog.update(cx, |dialog, cx| {
            dialog
                .name_input
                .update(cx, |input, cx| input.set_text("Agents 2", cx))
        });
        click_create(cx);
        assert!(creates(&requests).is_empty());
        assert!(dialog.read_with(cx, |dialog, _| dialog.error.is_some()));
        assert_eq!(*dismissed.borrow(), 0);

        dialog.update(cx, |dialog, cx| {
            dialog
                .name_input
                .update(cx, |input, cx| input.set_text("agents-2", cx));
            dialog.start_from = StartFrom::Snapshot;
            dialog.timeout = Timeout::After {
                minutes: 60,
                delete: true,
            };
            dialog.idle_pause = IdlePause::Never;
            cx.notify();
        });
        click_create(cx);
        assert_eq!(
            creates(&requests),
            [(
                Provider::Instabox,
                CreateMachine {
                    name: "agents-2".into(),
                    source: Some(MachineSource::Snapshot("s-1".into())),
                    timeout: Some(MachineTimeout {
                        after: Duration::from_secs(3600),
                        delete: true,
                    }),
                    auto_suspend: Some(Duration::ZERO),
                }
            )]
        );
        assert_eq!(*dismissed.borrow(), 1);
        let saved = cx.update(|_, cx| {
            AppSettingsStore::global(cx)
                .read(cx)
                .settings()
                .cloud_machine(Provider::Instabox, "m-new")
                .cloned()
        });
        let saved = saved.expect("the machine made is saved");
        assert_eq!(saved.label, "agents-2");
        assert!(saved.enabled);
    }

    /// An account whose idle pause Instabox fixes isn't asked for one, and a timeout longer
    /// than the account allows isn't sent.
    #[gpui::test]
    fn keeps_to_the_accounts_rules(cx: &mut TestAppContext) {
        let Opened { page, requests, .. } = open(cx);
        cx.update(|cx| {
            Machines::local(cx).update(cx, |client, cx| {
                let mut cloud = client.cloud().clone();
                for state in &mut cloud.providers {
                    if let ProviderStatus::SignedIn { rules, .. } = &mut state.status {
                        *rules = AccountRules {
                            fixed_idle_pause: Some(Duration::from_secs(1800)),
                            max_timeout: Some(Duration::from_secs(3600)),
                        };
                    }
                }
                client.set_cloud_for_test(cloud, cx);
            })
        });
        let (dialog, cx) = cx.add_window_view(|window, cx| {
            NewCloudMachine::new(Provider::Instabox, page.downgrade(), window, cx)
        });
        cx.run_until_parked();
        dialog.update(cx, |dialog, cx| {
            dialog.timeout = Timeout::After {
                minutes: 240,
                delete: false,
            };
            dialog.idle_pause = IdlePause::Never;
            cx.notify();
        });
        let create = cx
            .debug_bounds("new-cloud-machine-create")
            .expect("Create is shown");
        cx.simulate_click(create.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        let made = creates(&requests);
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].1.timeout, None);
        assert_eq!(made[0].1.auto_suspend, None);
        assert!(!Timeout::ALL[3].is_allowed(&AccountRules {
            fixed_idle_pause: None,
            max_timeout: Some(Duration::from_secs(3600)),
        }));
    }

    /// Making a Proton computer charges, so it asks first, with the price and the balance;
    /// the dialog stays under the question and closes once it's answered.
    #[gpui::test]
    fn making_a_proton_computer_asks_first(cx: &mut TestAppContext) {
        let Opened {
            page,
            requests,
            confirms,
        } = open(cx);
        let (dialog, cx) = cx.add_window_view(|window, cx| {
            NewCloudMachine::new(Provider::Proton, page.downgrade(), window, cx)
        });
        cx.run_until_parked();
        let dismissed = dismissals(&dialog, cx);
        // Proton has nothing to start from, so nothing is read.
        assert!(
            !requests
                .borrow()
                .iter()
                .any(|request| matches!(request, Request::CloudSources(_)))
        );
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert!(creates(&requests).is_empty());
        let confirm = confirms.borrow_mut().pop().expect("it asks first");
        assert!(confirm.is_charge);
        assert_eq!(confirm.title.as_ref(), "Create agents-1?");
        assert!(
            confirm.message.contains("($14.20 left)"),
            "{}",
            confirm.message
        );
        assert_eq!(*dismissed.borrow(), 0);

        cx.update(|window, cx| (confirm.on_confirm)(window, cx));
        cx.run_until_parked();
        assert_eq!(
            creates(&requests),
            [(
                Provider::Proton,
                CreateMachine {
                    name: "agents-1".into(),
                    ..Default::default()
                }
            )]
        );
        assert_eq!(*dismissed.borrow(), 1);
    }

    #[test]
    fn names_are_lowercase_letters_digits_and_dashes() {
        assert!(validate_name("agents-2").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("Agents").is_err());
        assert!(validate_name("my box").is_err());
        assert!(validate_name("-box").is_err());
        assert_eq!(Timeout::ALL[3].label(), "Pause after 4 hours");
        assert_eq!(Timeout::ALL[4].label(), "Delete after 30 min");
        assert_eq!(
            Timeout::ALL[5].timeout(),
            Some(MachineTimeout {
                after: Duration::from_secs(3600),
                delete: true,
            })
        );
        assert_eq!(IdlePause::Default.auto_suspend(), None);
        assert_eq!(IdlePause::Never.auto_suspend(), Some(Duration::ZERO));
    }
}
