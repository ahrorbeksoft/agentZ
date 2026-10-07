//! agentZ's own controls for the login, account and input-request surfaces, where `ui`'s
//! styles fall short: buttons with a solid accent or red fill, a field frame with a focus
//! ring, an account avatar, an icon tile, and a one-time code in boxes.

use gpui::{
    AnyElement, App, BoxShadow, ClickEvent, ElementId, Entity, Focusable as _, FontWeight, Hsla,
    SharedString, Window, div, linear_color_stop, linear_gradient,
};
use text_input::TextInput;
use ui::{CommonAnimationExt as _, Tooltip, prelude::*};

/// What a field's text, and a button's label, are set in.
pub(crate) const CONTROL_TEXT_SIZE: f32 = 13.;
pub(crate) const FIELD_HEIGHT: Pixels = px(28.);

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionStyle {
    /// Filled with the accent color: the one thing to do next.
    Primary,
    /// Filled red: a confirmed destructive action.
    Danger,
    Outline,
    Ghost,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionSize {
    /// 28px, beside fields and in rows.
    Medium,
    /// 32px, for a panel's main choices.
    Large,
}

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A text button in one of [`ActionStyle`]'s styles.
#[derive(IntoElement)]
pub(crate) struct ActionButton {
    id: ElementId,
    label: SharedString,
    style: ActionStyle,
    size: ActionSize,
    start_icon: Option<Icon>,
    end_icon: Option<Icon>,
    full_width: bool,
    disabled: bool,
    tooltip: Option<SharedString>,
    on_click: Option<ClickHandler>,
}

impl ActionButton {
    pub(crate) fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            style: ActionStyle::Outline,
            size: ActionSize::Medium,
            start_icon: None,
            end_icon: None,
            full_width: false,
            disabled: false,
            tooltip: None,
            on_click: None,
        }
    }

    pub(crate) fn style(mut self, style: ActionStyle) -> Self {
        self.style = style;
        self
    }

    pub(crate) fn size(mut self, size: ActionSize) -> Self {
        self.size = size;
        self
    }

    /// An icon before the label. Filled buttons draw it in their text color.
    pub(crate) fn start_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.start_icon = Some(icon.into());
        self
    }

    pub(crate) fn end_icon(mut self, icon: impl Into<Icon>) -> Self {
        self.end_icon = Some(icon.into());
        self
    }

    pub(crate) fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub(crate) fn tooltip(mut self, tooltip: impl Into<SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub(crate) fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for ActionButton {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors();
        let status = cx.theme().status();
        let (height, padding, radius) = match self.size {
            ActionSize::Medium => (px(28.), px(11.), px(6.)),
            ActionSize::Large => (px(32.), px(14.), px(7.)),
        };
        let transparent = gpui::transparent_black();
        let (background, border, text) = match self.style {
            ActionStyle::Primary => (colors.text_accent, transparent, on_fill_color(cx)),
            ActionStyle::Danger => (status.error, transparent, on_fill_color(cx)),
            ActionStyle::Outline => (transparent, colors.border, colors.text),
            ActionStyle::Ghost => (transparent, transparent, colors.text),
        };
        let is_filled = matches!(self.style, ActionStyle::Primary | ActionStyle::Danger);
        let (hover, pressed) = if is_filled {
            (shade(background, 0.12, cx), shade(background, 0.2, cx))
        } else {
            (colors.ghost_element_hover, colors.ghost_element_active)
        };
        let icon_color = if is_filled {
            Color::Custom(text)
        } else {
            Color::Muted
        };
        let tooltip = self.tooltip;
        h_flex()
            .id(self.id)
            .flex_none()
            .h(height)
            .px(padding)
            .gap(px(6.))
            .justify_center()
            .rounded(radius)
            .border_1()
            .border_color(border)
            .bg(background)
            .text_size(rems_from_px(CONTROL_TEXT_SIZE))
            .font_weight(if is_filled {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .text_color(text)
            .whitespace_nowrap()
            .when(self.full_width, |this| this.w_full())
            .map(|this| match self.on_click {
                Some(on_click) if !self.disabled => this
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .active(move |style| style.bg(pressed))
                    .on_click(on_click),
                _ => this,
            })
            .when(self.disabled, |this| this.opacity(0.45))
            .when_some(tooltip, |this, tooltip| {
                this.tooltip(Tooltip::text(tooltip))
            })
            .children(self.start_icon.map(|icon| icon.color(icon_color)))
            .child(self.label)
            .children(self.end_icon.map(|icon| icon.color(icon_color)))
    }
}

/// Text on an accent or red fill: dark on a dark theme's light fills, white on a light one's.
pub(crate) fn on_fill_color(cx: &App) -> Hsla {
    if cx.theme().appearance().is_light() {
        gpui::white()
    } else {
        gpui::hsla(220. / 360., 0.15, 0.1, 1.)
    }
}

/// A fill a little lighter (dark themes) or darker (light ones), for hover and press.
fn shade(color: Hsla, amount: f32, cx: &App) -> Hsla {
    let toward = if cx.theme().appearance().is_light() {
        gpui::black()
    } else {
        gpui::white()
    };
    color.blend(toward.opacity(amount))
}

/// The frame a text field sits in. A focused field has an accent ring, an invalid one a red
/// border.
pub(crate) fn field_frame(is_focused: bool, is_invalid: bool, cx: &App) -> gpui::Div {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let (border, ring) = if is_invalid {
        (status.error, status.error.opacity(0.18))
    } else {
        (colors.border_focused, colors.text_accent.opacity(0.16))
    };
    h_flex()
        .h(FIELD_HEIGHT)
        .min_w_0()
        .px(px(9.))
        .gap_1p5()
        .overflow_hidden()
        .rounded(px(6.))
        .border_1()
        .border_color(if is_focused || is_invalid {
            border
        } else {
            colors.border
        })
        .bg(colors.editor_background)
        .text_size(rems_from_px(CONTROL_TEXT_SIZE))
        .when(is_focused, |this| {
            this.shadow(vec![
                BoxShadow::new(px(0.), px(0.), ring).spread_radius(px(3.)),
            ])
        })
}

/// A text input in its frame, ringed while it has focus.
pub(crate) fn text_field(
    input: &Entity<TextInput>,
    is_invalid: bool,
    window: &Window,
    cx: &App,
) -> gpui::Div {
    let is_focused = input.read(cx).focus_handle(cx).is_focused(window);
    field_frame(is_focused, is_invalid, cx).child(div().flex_1().min_w_0().child(input.clone()))
}

/// A field's label above it.
pub(crate) fn field_label(text: impl Into<SharedString>) -> Label {
    Label::new(text).size(LabelSize::Small).color(Color::Muted)
}

/// A square with rounded corners holding an icon: an agent's, or a login method's.
pub(crate) fn icon_tile(icon: Icon, size: Pixels, cx: &App) -> gpui::Div {
    let colors = cx.theme().colors();
    let radius = if size >= px(48.) {
        px(12.)
    } else if size >= px(36.) {
        px(8.)
    } else {
        px(6.)
    };
    div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded(radius)
        .border_1()
        .border_color(colors.border_variant)
        .bg(colors.element_background)
        .child(icon.size(IconSize::Custom(rems_from_px(
            (f32::from(size) / 2.).round(),
        ))))
}

/// A round avatar with the account's initial, as t3code shows an account, in the account's
/// color or else the theme's accent.
pub(crate) fn avatar(name: &str, color: Option<Hsla>, size: Pixels, cx: &App) -> AnyElement {
    let accent = color.unwrap_or(cx.theme().colors().text_accent);
    let initial: SharedString = name
        .chars()
        .find(|character| character.is_alphanumeric())
        .map(|character| character.to_uppercase().collect::<String>())
        .unwrap_or_default()
        .into();
    div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(linear_gradient(
            135.,
            linear_color_stop(accent.blend(gpui::white().opacity(0.3)), 0.),
            linear_color_stop(accent.blend(gpui::black().opacity(0.15)), 1.),
        ))
        .text_size(rems_from_px((f32::from(size) * 13. / 32.).round().max(9.)))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(on_fill_color(cx))
        .child(initial)
        .into_any_element()
}

/// The colors an account can be given, from t3code's project colors: Tailwind's 600 shade,
/// which an account keeps as `#rrggbb`, and the 400 shade dark themes show it in.
pub(crate) const ACCOUNT_COLORS: [(&str, u32, u32); 8] = [
    ("Red", 0xdc2626, 0xf87171),
    ("Orange", 0xea580c, 0xfb923c),
    ("Yellow", 0xca8a04, 0xfacc15),
    ("Green", 0x16a34a, 0x4ade80),
    ("Teal", 0x0d9488, 0x2dd4bf),
    ("Blue", 0x2563eb, 0x60a5fa),
    ("Purple", 0x9333ea, 0xc084fc),
    ("Pink", 0xdb2777, 0xf472b6),
];

/// How an account's color is kept.
pub(crate) fn color_hex(color: u32) -> String {
    format!("#{color:06x}")
}

/// An account's color in the current theme: one of [`ACCOUNT_COLORS`] in the theme's shade,
/// any other `#rrggbb` as it is.
pub(crate) fn account_color(hex: &str, cx: &App) -> Option<Hsla> {
    let is_light = cx.theme().appearance().is_light();
    if let Some((_, light, dark)) = ACCOUNT_COLORS
        .iter()
        .find(|(_, light, _)| color_hex(*light).eq_ignore_ascii_case(hex))
    {
        return Some(gpui::rgb(if is_light { *light } else { *dark }).into());
    }
    gpui::Rgba::try_from(hex).ok().map(Hsla::from)
}

/// A round badge for an account the agent didn't name: a person with a check.
pub(crate) fn account_badge(size: Pixels, cx: &App) -> AnyElement {
    let status = cx.theme().status();
    div()
        .size(size)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(status.success_background)
        .child(
            Icon::new(IconName::UserCheck)
                .size(IconSize::Custom(rems_from_px(
                    (f32::from(size) * 0.45).round().max(10.),
                )))
                .color(Color::Success),
        )
        .into_any_element()
}

/// A one-time code, a character to a box, as device logins show it.
pub(crate) fn code_boxes(code: &str, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    h_flex()
        .gap(px(6.))
        .font_buffer(cx)
        .text_size(rems_from_px(18_f32))
        .children(code.chars().map(|character| {
            if character == '-' || character == ' ' {
                div()
                    .w(px(8.))
                    .h(px(2.))
                    .rounded_sm()
                    .bg(colors.text_muted)
                    .into_any_element()
            } else {
                div()
                    .w(px(30.))
                    .h(px(38.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.editor_background)
                    .child(character.to_string())
                    .into_any_element()
            }
        }))
        .into_any_element()
}

/// A key's symbol in a small outlined box, for hints such as "⏎ to submit".
pub(crate) fn key_hint(key: &'static str, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    div()
        .h(px(16.))
        .px(px(4.))
        .flex()
        .items_center()
        .rounded(px(4.))
        .border_1()
        .border_color(colors.border)
        .text_size(rems_from_px(10.5_f32))
        .text_color(colors.text_muted)
        .child(key)
        .into_any_element()
}

/// A small colored dot, for a status.
pub(crate) fn status_dot(color: Hsla) -> gpui::Div {
    div().size(px(7.)).flex_none().rounded_full().bg(color)
}

/// A status in a tinted badge with a dot, as beside an agent's name.
pub(crate) fn status_badge(label: impl Into<SharedString>, color: Hsla) -> AnyElement {
    h_flex()
        .h(px(18.))
        .px(px(6.))
        .gap(px(5.))
        .flex_none()
        .rounded(px(4.))
        .bg(color.opacity(0.1))
        .text_size(rems_from_px(11_f32))
        .text_color(color)
        .whitespace_nowrap()
        .child(div().size(px(6.)).rounded_full().bg(color))
        .child(label.into())
        .into_any_element()
}

/// A spinner, the icon Zed spins for work in progress.
pub(crate) fn spinner(color: Color) -> AnyElement {
    Icon::new(IconName::ArrowCircle)
        .size(IconSize::Small)
        .color(color)
        .with_rotate_animation(2)
        .into_any_element()
}

/// What the link's page is on, for "Open github.com".
pub(crate) fn link_host(url: &str) -> Option<String> {
    let url = url::Url::parse(url).ok()?;
    let host = url.host_str()?;
    Some(host.strip_prefix("www.").unwrap_or(host).to_string())
}

/// Copies text to the clipboard.
pub(crate) fn copy_to_clipboard(text: &str, cx: &mut App) {
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.to_string()));
}
