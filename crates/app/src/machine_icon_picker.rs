//! The popover a machine row's icon opens: the machine kinds as a grid of tiles, with the
//! one in use selected and the one its server detected marked.

use agentz_protocol::{CAPABILITY_MACHINE_ICON, MachineKind};
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent,
    Subscription, Window,
};
use ui::{ButtonLike, Tooltip, prelude::*};

use crate::machines::machine_kind_icon;
use crate::server_client::ServerClient;

const COLUMNS: u16 = 4;

pub struct MachineIconPicker {
    client: Option<Entity<ServerClient>>,
    focus_handle: FocusHandle,
    _subscription: Option<Subscription>,
}

impl EventEmitter<DismissEvent> for MachineIconPicker {}

impl Focusable for MachineIconPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl MachineIconPicker {
    pub fn new(
        client: Option<Entity<ServerClient>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let subscription = client
            .as_ref()
            .map(|client| cx.observe(client, |_, _, cx| cx.notify()));
        Self {
            client,
            focus_handle,
            _subscription: subscription,
        }
    }

    /// The server keeps the choice, so it's locked until a server that can is connected.
    fn lock(&self, cx: &App) -> Option<&'static str> {
        match self.client.as_ref().map(|client| client.read(cx)) {
            Some(client) if client.is_online() => {
                if client.has_capability(CAPABILITY_MACHINE_ICON) {
                    None
                } else if client.is_outdated() {
                    Some("Its server is too old to keep an icon. Update it to choose one.")
                } else {
                    Some("Its server is too old to keep an icon.")
                }
            }
            _ => Some("Connect to this machine to change its icon."),
        }
    }

    fn choose(&mut self, kind: MachineKind, cx: &mut Context<Self>) {
        if let Some(client) = &self.client {
            client.read(cx).choose_machine_icon(kind, cx);
        }
        cx.emit(DismissEvent);
    }
}

impl Render for MachineIconPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lock = self.lock(cx);
        let icon = self
            .client
            .as_ref()
            .map(|client| client.read(cx).machine_icon().clone())
            .unwrap_or_default();
        let current = icon.kind();
        let detected = icon.detected.unwrap_or(MachineKind::Server);
        let tiles = MachineKind::ALL.into_iter().map(|kind| {
            let is_detected = kind == detected;
            let tooltip: SharedString = if is_detected {
                format!("{} (detected)", kind.label()).into()
            } else {
                kind.label().into()
            };
            ButtonLike::new(SharedString::from(format!("machine-icon-{}", kind.label())))
                .toggle_state(kind == current)
                .disabled(lock.is_some())
                .tooltip(Tooltip::text(tooltip))
                .child(
                    v_flex()
                        .w(rems(4.5))
                        .py_2()
                        .gap_1()
                        .items_center()
                        .child(
                            Icon::new(machine_kind_icon(&kind))
                                .size(IconSize::Medium)
                                .color(if lock.is_some() {
                                    Color::Disabled
                                } else {
                                    Color::Default
                                }),
                        )
                        .child(
                            Label::new(kind.label())
                                .size(LabelSize::XSmall)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.choose(kind.clone(), cx)))
        });
        v_flex()
            .track_focus(&self.focus_handle)
            .on_mouse_down_out(cx.listener(|_, _, _, cx| cx.emit(DismissEvent)))
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.emit(DismissEvent)
                }
            }))
            .elevation_3(cx)
            .p_1()
            .gap_1()
            .when_some(lock, |picker, lock| {
                picker.child(
                    div()
                        .px_1()
                        .pt_0p5()
                        .max_w(rems(19.))
                        .child(Label::new(lock).size(LabelSize::Small).color(Color::Muted)),
                )
            })
            .child(div().grid().grid_cols(COLUMNS).gap_0p5().children(tiles))
    }
}
