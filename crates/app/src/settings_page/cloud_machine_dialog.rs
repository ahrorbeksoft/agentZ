//! Save Snapshot… and Publish Port… (design/cloud topics 7 A and 8 A), from a cloud machine's
//! page or its row's menu: a snapshot's name and whether it keeps the machine's home and
//! workspace, or a port and its name. Each hands its action to the settings page, which does it
//! through the machine's provider.

use agentz_protocol::cloud::{MachineAction, Provider};
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, SharedString,
    Subscription, WeakEntity, Window,
};
use text_input::{TextInput, TextInputEvent};
use ui::{ToggleButtonGroup, ToggleButtonGroupStyle, ToggleButtonSimple, prelude::*};
use util::ResultExt as _;

use super::SettingsPage;
use super::new_cloud_machine::render_field;
use crate::cloud_machines::provider_icon;

pub(super) const KEY_CONTEXT: &str = "CloudMachineDialog";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogKind {
    SaveSnapshot,
    PublishPort,
}

pub struct CloudMachineDialog {
    provider: Provider,
    machine: String,
    machine_name: SharedString,
    kind: DialogKind,
    page: WeakEntity<SettingsPage>,
    /// The snapshot's name, or the port's.
    name_input: Entity<TextInput>,
    port_input: Entity<TextInput>,
    software_only: bool,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for CloudMachineDialog {}

impl Focusable for CloudMachineDialog {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.kind {
            DialogKind::SaveSnapshot => self.name_input.focus_handle(cx),
            DialogKind::PublishPort => self.port_input.focus_handle(cx),
        }
    }
}

impl CloudMachineDialog {
    pub(super) fn new(
        provider: Provider,
        machine: String,
        machine_name: SharedString,
        kind: DialogKind,
        page: WeakEntity<SettingsPage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name_input = cx.new(|cx| {
            TextInput::new(
                match kind {
                    DialogKind::SaveSnapshot => "dev-base",
                    // Instabox's name for one without.
                    DialogKind::PublishPort => "Port N",
                },
                cx,
            )
        });
        if kind == DialogKind::SaveSnapshot {
            let today = chrono::Local::now().format("%Y-%m-%d");
            name_input.update(cx, |input, cx| {
                input.set_text(format!("{machine_name}-{today}"), cx)
            });
        }
        let port_input = cx.new(|cx| TextInput::new("3000", cx));
        let clear_error = |this: &mut Self, _, _: &TextInputEvent, cx: &mut Context<Self>| {
            this.error = None;
            cx.notify();
        };
        let subscriptions = vec![
            cx.subscribe(&name_input, clear_error),
            cx.subscribe(&port_input, clear_error),
        ];
        let this = Self {
            provider,
            machine,
            machine_name,
            kind,
            page,
            name_input,
            port_input,
            software_only: false,
            error: None,
            _subscriptions: subscriptions,
        };
        window.focus(&this.focus_handle(cx), cx);
        this
    }

    /// The action as the form describes it, or why it can't be done.
    fn action(&self, cx: &App) -> Result<MachineAction, SharedString> {
        let name = self.name_input.read(cx).text().trim().to_string();
        match self.kind {
            DialogKind::SaveSnapshot => {
                if name.is_empty() {
                    return Err("Give it a name.".into());
                }
                Ok(MachineAction::SaveSnapshot {
                    name,
                    software_only: self.software_only,
                })
            }
            DialogKind::PublishPort => {
                let port = self
                    .port_input
                    .read(cx)
                    .text()
                    .trim()
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port > 0)
                    .ok_or("A port is a number from 1 to 65535.")?;
                Ok(MachineAction::PublishPort {
                    port,
                    name: Some(name).filter(|name| !name.is_empty()),
                })
            }
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        let action = match self.action(cx) {
            Ok(action) => action,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        let provider = self.provider;
        let machine = self.machine.clone();
        self.page
            .update(cx, |page, cx| {
                page.send_machine_action(provider, machine, action, cx)
            })
            .log_err();
        cx.emit(DismissEvent);
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn render_contents(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let input_box = |input: &Entity<TextInput>| {
            div()
                .h(px(28.))
                .px_2()
                .flex()
                .items_center()
                .overflow_hidden()
                .rounded_md()
                .border_1()
                .border_color(colors.border)
                .bg(colors.editor_background)
                .child(input.clone())
                .into_any_element()
        };
        match self.kind {
            DialogKind::SaveSnapshot => {
                let this = cx.weak_entity();
                let pick = move |software_only: bool| {
                    let this = this.clone();
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                        this.update(cx, |this, cx| {
                            this.software_only = software_only;
                            cx.notify();
                        })
                        .log_err();
                    }
                };
                let keeps = div()
                    .debug_selector(|| "cloud-machine-dialog-keeps".into())
                    .child(
                        ToggleButtonGroup::single_row(
                            "cloud-machine-dialog-keeps",
                            [
                                ToggleButtonSimple::new(
                                    "Software, home and workspace",
                                    pick(false),
                                ),
                                ToggleButtonSimple::new("Software only", pick(true)),
                            ],
                        )
                        .style(ToggleButtonGroupStyle::Outlined)
                        .label_size(LabelSize::Small)
                        .auto_width()
                        .selected_index(usize::from(self.software_only)),
                    )
                    .into_any_element();
                v_flex()
                    .gap_3()
                    .child(render_field("Name", input_box(&self.name_input), None))
                    .child(render_field(
                        "Keep",
                        keeps,
                        Some(
                            "Software only leaves out /data, where its home and workspace are."
                                .into(),
                        ),
                    ))
                    .into_any_element()
            }
            DialogKind::PublishPort => h_flex()
                .gap_3()
                .items_start()
                .child(div().w(px(110.)).child(render_field(
                    "Port",
                    input_box(&self.port_input),
                    None,
                )))
                .child(div().flex_1().min_w_0().child(render_field(
                    "Name",
                    input_box(&self.name_input),
                    Some("Optional.".into()),
                )))
                .into_any_element(),
        }
    }
}

impl Render for CloudMachineDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let provider_name = self.provider.description().name;
        let (title, note, confirm_label) = match self.kind {
            DialogKind::SaveSnapshot => (
                format!("Save a Snapshot of {}", self.machine_name),
                format!(
                    "New machines can start from it. Saving may stop it first, where \
                     {provider_name} can't save it while it runs."
                ),
                "Save Snapshot",
            ),
            DialogKind::PublishPort => (
                format!("Publish a Port of {}", self.machine_name),
                "Anyone with its URL reaches the service until it's unpublished. The URL \
                 answers only while the machine runs."
                    .to_string(),
                "Publish",
            ),
        };
        let (note, note_color) = match &self.error {
            Some(error) => (error.clone(), Color::Error),
            None => (note.into(), Color::Muted),
        };
        let contents = self.render_contents(cx);
        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(32.))
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
                        Icon::new(provider_icon(self.provider))
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(Label::new(title)),
            )
            .child(v_flex().px_4().py_2().child(contents))
            .child(
                h_flex()
                    .mt_2()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(colors.border_variant)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Label::new(note).size(LabelSize::Small).color(note_color)),
                    )
                    .child(
                        Button::new("cloud-machine-dialog-cancel", "Cancel")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        div()
                            .debug_selector(|| "cloud-machine-dialog-confirm".into())
                            .child(
                                Button::new("cloud-machine-dialog-confirm", confirm_label)
                                    .style(ButtonStyle::Filled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm(&menu::Confirm, window, cx)
                                    })),
                            ),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use agentz_protocol::Request;
    use gpui::{TestAppContext, VisualTestContext};

    use super::super::cloud_machine_page::tests::{Served, actions, serve};
    use super::*;

    struct Opened {
        dialog: Entity<CloudMachineDialog>,
        /// The dialog holds its page weakly, so the test keeps it.
        page: Entity<SettingsPage>,
        requests: Rc<RefCell<Vec<Request>>>,
        dismissed: Rc<Cell<usize>>,
    }

    fn open(kind: DialogKind, cx: &mut TestAppContext) -> (Opened, &mut VisualTestContext) {
        let Served { requests, .. } = serve(cx);
        let page = cx.update(|cx| cx.new(SettingsPage::new));
        let (dialog, cx) = cx.add_window_view(|window, cx| {
            CloudMachineDialog::new(
                Provider::Instabox,
                "m-1".into(),
                "agents-1".into(),
                kind,
                page.downgrade(),
                window,
                cx,
            )
        });
        let dismissed = Rc::new(Cell::new(0));
        cx.update(|_, cx| {
            let dismissed = dismissed.clone();
            cx.subscribe(&dialog, move |_, _: &DismissEvent, _| {
                dismissed.set(dismissed.get() + 1)
            })
            .detach();
        });
        let opened = Opened {
            dialog,
            page,
            requests,
            dismissed,
        };
        (opened, cx)
    }

    /// Save Snapshot… names it after the machine and the day, and saves it as picked, then
    /// closes.
    #[gpui::test]
    fn saves_a_snapshot_as_picked(cx: &mut TestAppContext) {
        let (
            Opened {
                dialog,
                page: _page,
                requests,
                dismissed,
            },
            cx,
        ) = open(DialogKind::SaveSnapshot, cx);
        let name = format!("agents-1-{}", chrono::Local::now().format("%Y-%m-%d"));
        dialog.update(cx, |dialog, cx| {
            assert_eq!(dialog.name_input.read(cx).text().as_ref(), name);
            dialog.software_only = true;
            cx.notify();
        });
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        assert_eq!(
            actions(&requests),
            [MachineAction::SaveSnapshot {
                name,
                software_only: true,
            }]
        );
        assert_eq!(dismissed.get(), 1);
    }

    /// Publish Port… takes a port from 1 to 65535 and an optional name.
    #[gpui::test]
    fn publishes_a_port_from_1_to_65535(cx: &mut TestAppContext) {
        let (
            Opened {
                dialog,
                page: _page,
                requests,
                dismissed,
            },
            cx,
        ) = open(DialogKind::PublishPort, cx);
        let click_publish = |cx: &mut VisualTestContext| {
            let publish = cx
                .debug_bounds("cloud-machine-dialog-confirm")
                .expect("Publish is shown");
            cx.simulate_click(publish.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        dialog.update(cx, |dialog, cx| {
            dialog
                .port_input
                .update(cx, |input, cx| input.set_text("0", cx))
        });
        click_publish(cx);
        assert!(actions(&requests).is_empty());
        assert!(dialog.read_with(cx, |dialog, _| dialog.error.is_some()));
        assert_eq!(dismissed.get(), 0);

        dialog.update(cx, |dialog, cx| {
            dialog
                .port_input
                .update(cx, |input, cx| input.set_text("3000", cx));
            dialog
                .name_input
                .update(cx, |input, cx| input.set_text(" web ", cx));
        });
        click_publish(cx);
        assert_eq!(
            actions(&requests),
            [MachineAction::PublishPort {
                port: 3000,
                name: Some("web".into()),
            }]
        );
        assert_eq!(dismissed.get(), 1);
    }
}
