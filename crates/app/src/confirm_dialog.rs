//! agentZ's own dialog for confirming a destructive action, such as logging out of an agent:
//! an icon in the danger tint beside the question, what will happen, and Cancel beside a red
//! button. It sits in the shell's modal layer, centered, as t3code's dialogs do.

use std::rc::Rc;

use agentz_protocol::accounts::AccountStatus;
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

    /// Logging out of one of an agent's accounts. The External account is the agent's own
    /// login, which its CLI shares.
    pub fn account_logout(
        account_name: &str,
        agent_name: &str,
        is_external: bool,
        on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        let message = if is_external {
            format!(
                "This is {agent_name}'s own login, so its CLI is logged out too. Running threads \
                 on it stop. Thread history is kept."
            )
        } else {
            "Running threads on it stop, and ask to log in again. Thread history is kept. The \
             account stays, to log in again any time."
                .to_string()
        };
        Self {
            icon: IconName::Exit,
            title: format!("Log out of {account_name}?").into(),
            message: message.into(),
            confirm_label: "Log Out".into(),
            on_confirm: Rc::new(on_confirm),
        }
    }

    /// Using one of an account's limit resets, which can't be given back (decisions.md §9).
    /// `status` is the account's last read, with its windows and resets.
    pub fn use_limit_reset(
        account_name: &str,
        agent_name: &str,
        status: &AccountStatus,
        on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        let mut names: Vec<String> = status
            .windows
            .iter()
            .map(|window| {
                let mut chars = window.label.chars();
                chars
                    .next()
                    .map(|first| first.to_lowercase().chain(chars).collect())
                    .unwrap_or_default()
            })
            .collect();
        let limits = match names.pop() {
            None => "limits".to_string(),
            Some(last) if names.is_empty() => format!("{last} limit"),
            Some(last) => format!("{} and {last} limits", names.join(", ")),
        };
        // Which login it is, when its name doesn't say.
        let mut about: Vec<&str> = status
            .email
            .as_deref()
            .filter(|email| *email != account_name)
            .into_iter()
            .collect();
        about.push(agent_name);
        let spends = match status.limit_resets.map_or(1, |resets| resets.available) {
            1 => "your only reset".to_string(),
            available => format!("one of your {available} resets"),
        };
        Self {
            icon: IconName::RotateCcw,
            title: "Use a limit reset?".into(),
            message: format!(
                "This clears {account_name}'s {limits} now ({}). It uses {spends} and can't be \
                 undone.",
                about.join(", ")
            )
            .into(),
            confirm_label: "Use Reset".into(),
            on_confirm: Rc::new(on_confirm),
        }
    }

    /// Removing an agentZ account, which deletes its folder.
    pub fn remove_account(
        account_name: &str,
        on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            icon: IconName::Trash,
            title: format!("Remove {account_name}?").into(),
            message: "This deletes the account's login, sessions and history. Its threads stay \
                      in the sidebar, but can't continue."
                .into(),
            confirm_label: "Remove Account".into(),
            on_confirm: Rc::new(on_confirm),
        }
    }

    /// Deleting one of agentZ's skills, in Zed's words. There's no trash to move it to on the
    /// server's machine, so it's deleted for good.
    pub fn delete_skill(
        skill_name: &str,
        folder: &str,
        on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            icon: IconName::Trash,
            title: format!("Delete the skill \"{skill_name}\"?").into(),
            message: format!(
                "This will delete {folder} for good. Every agent and account loads this skill, \
                 so it will no longer be available to them either."
            )
            .into(),
            confirm_label: "Delete".into(),
            on_confirm: Rc::new(on_confirm),
        }
    }

    /// Settings › Storage's trash on a row: what goes and its size, deleted for good
    /// (design/storage §10).
    pub fn delete_storage(
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            icon: IconName::Trash,
            title: title.into(),
            message: message.into(),
            confirm_label: "Delete".into(),
            on_confirm: Rc::new(on_confirm),
        }
    }

    /// Settings › Storage's Clear on the registry's cache or the server's log.
    pub fn clear_storage(
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            icon: IconName::Eraser,
            title: title.into(),
            message: message.into(),
            confirm_label: "Clear".into(),
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
