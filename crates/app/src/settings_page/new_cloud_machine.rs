//! A provider's New Machine dialog (design/cloud topic 4 C), from Add Machine's menu or the
//! provider's group: the account, the machine's name, and what each of the provider's machines
//! is and costs. Create hands the machine to the settings page, which asks first when making
//! it charges (topic 5 C), makes it, and saves it once it's made.

use agentz_protocol::cloud::{CreateMachine, Provider};
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, SharedString,
    Subscription, WeakEntity, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::prelude::*;
use util::ResultExt as _;

use super::SettingsPage;
use crate::app_settings::AppSettingsStore;
use crate::cloud_machines::{charge_message, noun_title, provider_icon, signed_in_account};
use crate::machines::Machines;

pub(super) const KEY_CONTEXT: &str = "NewCloudMachine";

pub(super) fn minutes_label(minutes: u64) -> String {
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
    error: Option<SharedString>,
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
        Self {
            provider,
            page,
            name_input,
            error: None,
            _subscriptions: subscriptions,
        }
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
        Ok(CreateMachine { name })
    }

    fn confirm(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
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
}

impl Render for NewCloudMachine {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(render_field(
                        "Name",
                        name_box,
                        Some("Lowercase letters, digits and -.".into()),
                    ))
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
                        self.error.clone().map(|error| {
                            Label::new(error).size(LabelSize::Small).color(Color::Error)
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
                                    .disabled(account.is_none())
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm(&menu::Confirm, window, cx)
                                    })),
                            ),
                    ),
            )
    }
}

pub(super) fn render_field(
    title: &'static str,
    field: AnyElement,
    hint: Option<String>,
) -> AnyElement {
    v_flex()
        .gap_1()
        .child(Label::new(title).size(LabelSize::Small))
        .child(field)
        .children(hint.map(|hint| Label::new(hint).size(LabelSize::Small).color(Color::Muted)))
        .into_any_element()
}

/// Names Proton takes: lowercase letters, digits and dashes, not at either end.
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

    use agentz_protocol::cloud::{CloudSnapshot, CloudState, CloudUsage, ProviderStatus};
    use agentz_protocol::spaces::SpacesSnapshot;
    use agentz_protocol::{Request, Response};
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

    /// The settings page over a test server: Proton signed in with `devbox-3` and a balance.
    /// Making a computer gives it back as `c-new`.
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
                        Request::CreateCloudMachine { machine, .. } => {
                            Some(Response::CloudMachineCreated(cloud_machine(
                                "c-new",
                                &machine.name,
                                CloudState::Running,
                            )))
                        }
                        _ => None,
                    }
                });
                let mut proton = provider_state(
                    Provider::Proton,
                    ProviderStatus::SignedIn {
                        version: "1.0.0".into(),
                        account: "me@example.com".into(),
                    },
                );
                proton.machines = vec![cloud_machine("c-1", "devbox-3", CloudState::Running)];
                proton.usage = Some(CloudUsage::Balance {
                    balance_micro_usd: Some(14_200_000),
                    running_hours: 3.,
                    charged_micro_usd: 0,
                });
                client.set_cloud_for_test(
                    CloudSnapshot {
                        providers: vec![proton],
                        ..Default::default()
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

    /// A name Proton doesn't take, or one of the account's computers has, is refused before
    /// anything is made or asked.
    #[gpui::test]
    fn refuses_names_proton_doesnt_take(cx: &mut TestAppContext) {
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
        let create_as = |name: &str, cx: &mut gpui::VisualTestContext| {
            dialog.update(cx, |dialog, cx| {
                dialog
                    .name_input
                    .update(cx, |input, cx| input.set_text(name.to_string(), cx))
            });
            let create = cx
                .debug_bounds("new-cloud-machine-create")
                .expect("Create is shown");
            cx.simulate_click(create.center(), gpui::Modifiers::none());
            cx.run_until_parked();
            dialog.read_with(cx, |dialog, _| {
                dialog.error.as_ref().map(|error| error.to_string())
            })
        };

        assert_eq!(
            create_as("Agents 2", cx).as_deref(),
            Some("Use lowercase letters, digits and -.")
        );
        assert_eq!(
            create_as("devbox-3", cx).as_deref(),
            Some("Proton already has a computer called devbox-3.")
        );
        assert!(creates(&requests).is_empty());
        assert!(confirms.borrow().is_empty());
        assert_eq!(*dismissed.borrow(), 0);
    }

    /// Making a Proton computer charges, so it asks first, with the price and the balance;
    /// the dialog stays under the question and closes once it's answered. Once made, it's
    /// saved and switched on.
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
        dialog.read_with(cx, |dialog, cx| {
            assert_eq!(dialog.name_input.read(cx).text().as_ref(), "agents-1");
        });
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
                }
            )]
        );
        assert_eq!(*dismissed.borrow(), 1);
        let saved = cx.update(|_, cx| {
            AppSettingsStore::global(cx)
                .read(cx)
                .settings()
                .cloud_machine(Provider::Proton, "c-new")
                .cloned()
        });
        let saved = saved.expect("the computer made is saved");
        assert_eq!(saved.label, "agents-1");
        assert!(saved.enabled);
    }

    #[test]
    fn names_are_lowercase_letters_digits_and_dashes() {
        assert!(validate_name("agents-2").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("Agents").is_err());
        assert!(validate_name("my box").is_err());
        assert!(validate_name("-box").is_err());
        assert_eq!(minutes_label(30), "30 min");
        assert_eq!(minutes_label(60), "1 hour");
        assert_eq!(minutes_label(240), "4 hours");
    }
}
