//! Zed's Welcome page (`workspace/src/welcome.rs`), for a main area with nothing in it: a
//! headline over sections, each an uppercase header with a rule and rows of an icon, a label
//! and the row's key.

use gpui::{Action, ClickEvent, FocusHandle};
use ui::{ButtonLike, Divider, DividerColor, KeyBinding, prelude::*};

/// The headline and sections in a column down the middle, as Zed's Welcome page lays them out.
pub fn render_welcome(
    id: &'static str,
    headline: impl Into<SharedString>,
    sections: impl IntoIterator<Item = Section>,
) -> impl IntoElement {
    h_flex().size_full().justify_center().child(
        v_flex()
            .id(id)
            .p_8()
            .max_w_128()
            .size_full()
            .gap_6()
            .justify_center()
            .overflow_y_scroll()
            .child(
                h_flex()
                    .w_full()
                    .justify_center()
                    .child(Headline::new(headline)),
            )
            .children(sections),
    )
}

#[derive(IntoElement)]
pub struct Section {
    title: SharedString,
    buttons: Vec<SectionButton>,
}

impl Section {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            buttons: Vec::new(),
        }
    }

    pub fn button(mut self, button: SectionButton) -> Self {
        self.buttons.push(button);
        self
    }
}

impl RenderOnce for Section {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex()
            .min_w_full()
            .child(
                h_flex()
                    .px_1()
                    .mb_2()
                    .gap_2()
                    .child(
                        Label::new(self.title.to_ascii_uppercase())
                            .buffer_font(cx)
                            .color(Color::Muted)
                            .size(LabelSize::XSmall),
                    )
                    .child(Divider::horizontal().color(DividerColor::BorderVariant)),
            )
            .children(self.buttons)
    }
}

#[derive(IntoElement)]
pub struct SectionButton {
    label: SharedString,
    icon: IconName,
    key_binding: Option<KeyBinding>,
    on_click: Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>,
}

impl SectionButton {
    pub fn new(
        label: impl Into<SharedString>,
        icon: IconName,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            icon,
            key_binding: None,
            on_click: Box::new(on_click),
        }
    }

    /// Runs `action` where `focus` is, and shows its key there, as Zed's rows do.
    pub fn for_action(
        label: impl Into<SharedString>,
        icon: IconName,
        action: &dyn Action,
        focus: &FocusHandle,
        cx: &App,
    ) -> Self {
        let key_binding = KeyBinding::for_action_in(action, focus, cx);
        let action = action.boxed_clone();
        let focus = focus.clone();
        Self::new(label, icon, move |_, window, cx| {
            focus.dispatch_action(&*action, window, cx)
        })
        .key_binding(key_binding)
    }

    pub fn key_binding(mut self, key_binding: KeyBinding) -> Self {
        self.key_binding = Some(key_binding.size(rems_from_px(12_f32)));
        self
    }
}

impl RenderOnce for SectionButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let on_click = self.on_click;
        let id = format!("welcome-{}", self.label);
        div().w_full().debug_selector(|| id.clone()).child(
            ButtonLike::new(SharedString::from(id))
                .full_width()
                .size(ButtonSize::Medium)
                .child(
                    h_flex()
                        .w_full()
                        .justify_between()
                        .child(
                            h_flex()
                                .gap_2()
                                .child(
                                    Icon::new(self.icon)
                                        .color(Color::Muted)
                                        .size(IconSize::Small),
                                )
                                .child(Label::new(self.label)),
                        )
                        .children(self.key_binding),
                )
                .on_click(move |event, window, cx| on_click(event, window, cx)),
        )
    }
}
