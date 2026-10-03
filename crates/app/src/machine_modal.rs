//! Settings › Machines' Add Machine and Edit… dialog (t3code's Add Environment dialog, SSH
//! only): the target ssh is given and the name shown for it.

use gpui::{App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding};
use text_input::{TextInput, TextInputEvent};
use ui::prelude::*;

use crate::app_settings::{AppSettingsStore, MachineProfile};

const KEY_CONTEXT: &str = "MachineModal";

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

pub struct MachineModal {
    /// The machine being edited; none adds one.
    editing: Option<u64>,
    app_settings: Entity<AppSettingsStore>,
    target_input: Entity<TextInput>,
    label_input: Entity<TextInput>,
    error: Option<SharedString>,
    _subscriptions: Vec<gpui::Subscription>,
}

impl EventEmitter<DismissEvent> for MachineModal {}

impl Focusable for MachineModal {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.target_input.focus_handle(cx)
    }
}

impl MachineModal {
    pub fn new(
        editing: Option<&MachineProfile>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let target_input =
            cx.new(|cx| TextInput::new("user@host, or a Host from ~/.ssh/config", cx));
        let label_input = cx.new(|cx| TextInput::new("Name (optional)", cx));
        if let Some(profile) = editing {
            target_input.update(cx, |input, cx| input.set_text(profile.target.clone(), cx));
            label_input.update(cx, |input, cx| input.set_text(profile.label.clone(), cx));
        }
        let subscriptions = vec![
            cx.subscribe(&target_input, |this, _, _: &TextInputEvent, cx| {
                this.error = None;
                cx.notify();
            }),
            cx.subscribe(&label_input, |_, _, _: &TextInputEvent, cx| cx.notify()),
        ];
        window.focus(&target_input.focus_handle(cx), cx);
        Self {
            editing: editing.map(|profile| profile.id),
            app_settings: AppSettingsStore::global(cx),
            target_input,
            label_input,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        let target = self.target_input.read(cx).text().trim().to_string();
        let label = self.label_input.read(cx).text().trim().to_string();
        if let Err(error) = agentz_client::ssh::validate_target(&target) {
            self.error = Some(format!("{error:#}").into());
            cx.notify();
            return;
        }
        let editing = self.editing;
        let is_taken = self
            .app_settings
            .read(cx)
            .settings()
            .machines
            .iter()
            .any(|profile| profile.target == target && Some(profile.id) != editing);
        if is_taken {
            self.error = Some(format!("{target} is already saved.").into());
            cx.notify();
            return;
        }
        self.app_settings.update(cx, |store, cx| {
            store.update(
                |settings| match editing {
                    Some(id) => {
                        if let Some(profile) = settings
                            .machines
                            .iter_mut()
                            .find(|profile| profile.id == id)
                        {
                            profile.label = label;
                            profile.target = target;
                        }
                    }
                    None => {
                        settings.add_machine(label, target);
                    }
                },
                cx,
            )
        });
        cx.emit(DismissEvent);
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn render_field(
        &self,
        title: &'static str,
        description: &'static str,
        input: Entity<TextInput>,
        cx: &App,
    ) -> impl IntoElement {
        let colors = cx.theme().colors();
        v_flex()
            .gap_1()
            .child(Label::new(title).size(LabelSize::Small))
            .child(
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
                    .child(input),
            )
            .child(
                Label::new(description)
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
    }
}

impl Render for MachineModal {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let is_editing = self.editing.is_some();
        let has_target = !self.target_input.read(cx).text().trim().is_empty();
        let (note, note_color): (SharedString, Color) = match &self.error {
            Some(error) => (error.clone(), Color::Error),
            None => (
                "agentZ installs its server in ~/.agentz there, and nothing else.".into(),
                Color::Muted,
            ),
        };
        v_flex()
            .key_context(KEY_CONTEXT)
            .w(rems(32.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(
                v_flex()
                    .px_4()
                    .pt_3()
                    .pb_2()
                    .gap_0p5()
                    .child(Label::new(if is_editing {
                        "Edit Machine"
                    } else {
                        "Add Machine"
                    }))
                    .child(
                        Label::new("Connect to another machine over SSH.")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
            )
            .child(
                v_flex()
                    .px_4()
                    .py_2()
                    .gap_3()
                    .child(self.render_field(
                        "SSH target",
                        "What you'd give ssh. Your keys, agent and ~/.ssh/config are used; \
                         password prompts aren't.",
                        self.target_input.clone(),
                        cx,
                    ))
                    .child(self.render_field(
                        "Name",
                        "Shown in the sidebar. Leave it empty for the target.",
                        self.label_input.clone(),
                        cx,
                    )),
            )
            .child(
                h_flex()
                    .mt_2()
                    .px_4()
                    .py_2()
                    .gap_2()
                    .border_t_1()
                    .border_color(border_variant)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(Label::new(note).size(LabelSize::Small).color(note_color)),
                    )
                    .child(
                        Button::new("machine-modal-cancel", "Cancel")
                            .style(ButtonStyle::Subtle)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        Button::new(
                            "machine-modal-save",
                            if is_editing { "Save" } else { "Add Machine" },
                        )
                        .style(ButtonStyle::Filled)
                        .disabled(!has_target)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.confirm(&menu::Confirm, window, cx)
                        })),
                    ),
            )
    }
}
