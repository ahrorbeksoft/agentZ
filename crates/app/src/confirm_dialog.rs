//! agentZ's own dialog for confirming a destructive action, such as logging out of an agent:
//! an icon in the danger tint beside the question, what will happen, and Cancel beside a red
//! button. It sits in the shell's modal layer, centered, as t3code's dialogs do.

use std::rc::Rc;

use gpui::{
    App, BoxShadow, Context, DismissEvent, EventEmitter, FocusHandle, Focusable, FontWeight,
    KeyBinding, SharedString, Window,
};
use ui::prelude::*;

use crate::controls::{ActionButton, ActionStyle};

const KEY_CONTEXT: &str = "ConfirmDialog";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

/// What to ask, and what to do when the user agrees.
#[derive(Clone)]
pub struct ConfirmRequest {
    pub icon: IconName,
    pub title: SharedString,
    pub message: SharedString,
    pub confirm_label: SharedString,
    pub on_confirm: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl ConfirmRequest {
    /// Logging out of an agent, from its page or a thread. It affects every thread with it.
    pub fn logout(agent_name: &str, on_confirm: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self {
            icon: IconName::Exit,
            title: format!("Log out of {agent_name}?").into(),
            message: "Running threads that share this login stop. Thread history is kept. You \
                      can log in again any time."
                .into(),
            confirm_label: "Log Out".into(),
            on_confirm: Rc::new(on_confirm),
        }
    }
}

pub struct ConfirmDialog {
    focus_handle: FocusHandle,
    request: ConfirmRequest,
}

impl EventEmitter<DismissEvent> for ConfirmDialog {}

impl Focusable for ConfirmDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ConfirmDialog {
    pub fn new(request: ConfirmRequest, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            focus_handle,
            request,
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
        (self.request.on_confirm)(window, cx);
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }
}

impl Render for ConfirmDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let status = cx.theme().status();
        v_flex()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .w(px(380.))
            .rounded(px(12.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow(vec![
                BoxShadow::new(px(0.), px(24.), gpui::black().opacity(0.45)).blur_radius(px(64.)),
            ])
            .overflow_hidden()
            .child(
                v_flex()
                    .p_5()
                    .gap_3()
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                div()
                                    .size(px(32.))
                                    .flex_none()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(px(8.))
                                    .bg(status.error.opacity(0.12))
                                    .child(
                                        Icon::new(self.request.icon)
                                            .size(IconSize::Medium)
                                            .color(Color::Error),
                                    ),
                            )
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.request.title.clone()),
                            ),
                    )
                    .child(
                        Label::new(self.request.message.clone())
                            .size(LabelSize::Custom(rems_from_px(13_f32)))
                            .color(Color::Muted),
                    ),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .px_5()
                    .py_3()
                    .border_t_1()
                    .border_color(colors.border_variant)
                    .bg(gpui::black().opacity(0.08))
                    .child(
                        ActionButton::new("confirm-dialog-cancel", "Cancel")
                            .style(ActionStyle::Ghost)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel(&menu::Cancel, window, cx)
                            })),
                    )
                    .child(
                        ActionButton::new(
                            "confirm-dialog-confirm",
                            self.request.confirm_label.clone(),
                        )
                        .style(ActionStyle::Danger)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.confirm(&menu::Confirm, window, cx)
                        })),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn escape_cancels_and_enter_confirms(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
        });
        let confirmed = Rc::new(Cell::new(0));
        let request = {
            let confirmed = confirmed.clone();
            ConfirmRequest::logout("Mock", move |_, _| confirmed.set(confirmed.get() + 1))
        };
        let (dialog, cx) = cx.add_window_view(|window, cx| ConfirmDialog::new(request, window, cx));
        let dismissed = Rc::new(Cell::new(0));
        cx.update(|_, cx| {
            let dismissed = dismissed.clone();
            cx.subscribe(&dialog, move |_, _: &DismissEvent, _| {
                dismissed.set(dismissed.get() + 1)
            })
            .detach();
        });

        cx.simulate_keystrokes("escape");
        assert_eq!((dismissed.get(), confirmed.get()), (1, 0));
        cx.simulate_keystrokes("enter");
        assert_eq!((dismissed.get(), confirmed.get()), (2, 1));
    }
}
