//! A slider from 0 to 1, as macOS's Alert volume draws one: a track filled up to the value,
//! with a knob on it. `ui` has none. Pressing on it or dragging its knob moves it, and the
//! value is reported as the mouse lets go.

use std::rc::Rc;

use gpui::{
    App, Bounds, BoxShadow, DispatchPhase, ElementId, HitboxBehavior, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Window, canvas, div, point, relative,
};
use ui::prelude::*;

const WIDTH: Pixels = px(170.);
const KNOB_SIZE: Pixels = px(14.);
const TRACK_HEIGHT: Pixels = px(4.);

type ReleaseHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(crate) struct Slider {
    id: ElementId,
    value: f32,
    on_release: Option<ReleaseHandler>,
}

impl Slider {
    pub fn new(id: impl Into<ElementId>, value: f32) -> Self {
        Self {
            id: id.into(),
            value,
            on_release: None,
        }
    }

    /// Called with the value where the mouse lets go, after a drag or a press on the track.
    pub fn on_release(mut self, handler: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_release = Some(Rc::new(handler));
        self
    }
}

/// The value under `x`. The knob's center travels between the track's ends, which sit half a
/// knob in from the slider's sides so the knob never pokes out of it.
fn value_at(bounds: Bounds<Pixels>, x: Pixels) -> f32 {
    let travel = bounds.size.width - KNOB_SIZE;
    if travel <= px(0.) {
        return 0.;
    }
    ((x - bounds.left() - KNOB_SIZE / 2.) / travel).clamp(0., 1.)
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // The value while the mouse is down, which the caller only learns as it lets go.
        let dragged = window.use_keyed_state(self.id.clone(), cx, |_, _| None::<f32>);
        let value = dragged.read(cx).unwrap_or(self.value).clamp(0., 1.);
        let colors = cx.theme().colors();
        // `ui`'s Switch's colors: the empty track in its border's, the fill in its on color
        // and the knob in its thumb's. Its off fill barely shows on a settings row.
        let track = colors.border;
        let fill = track.blend(cx.theme().status().info.opacity(0.6));
        let knob = colors.text;
        let on_release = self.on_release;
        div()
            .id(self.id)
            .relative()
            .flex_none()
            .w(WIDTH)
            .h(KNOB_SIZE)
            .cursor_pointer()
            .child(
                div()
                    .absolute()
                    .left(KNOB_SIZE / 2.)
                    .right(KNOB_SIZE / 2.)
                    .top((KNOB_SIZE - TRACK_HEIGHT) / 2.)
                    .h(TRACK_HEIGHT)
                    .rounded_full()
                    .bg(track)
                    .child(div().h_full().w(relative(value)).rounded_full().bg(fill)),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left((WIDTH - KNOB_SIZE) * value)
                    .size(KNOB_SIZE)
                    .rounded_full()
                    .bg(knob)
                    .shadow(vec![BoxShadow {
                        color: gpui::black().opacity(0.4),
                        offset: point(px(0.), px(1.)),
                        blur_radius: px(3.),
                        spread_radius: px(0.),
                        inset: false,
                    }]),
            )
            .child(
                canvas(
                    |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                    move |bounds, hitbox, window, _| {
                        window.on_mouse_event({
                            let dragged = dragged.clone();
                            move |event: &MouseDownEvent, phase, window, cx| {
                                if phase == DispatchPhase::Bubble
                                    && event.button == MouseButton::Left
                                    && hitbox.is_hovered(window)
                                {
                                    let value = value_at(bounds, event.position.x);
                                    dragged.update(cx, |dragged, cx| {
                                        *dragged = Some(value);
                                        cx.notify();
                                    });
                                    cx.stop_propagation();
                                }
                            }
                        });
                        // Moves and the release count anywhere in the window, so the knob
                        // follows the mouse past the slider's ends.
                        window.on_mouse_event({
                            let dragged = dragged.clone();
                            move |event: &MouseMoveEvent, phase, _, cx| {
                                if phase == DispatchPhase::Bubble
                                    && event.pressed_button == Some(MouseButton::Left)
                                    && dragged.read(cx).is_some()
                                {
                                    let value = value_at(bounds, event.position.x);
                                    dragged.update(cx, |dragged, cx| {
                                        if *dragged != Some(value) {
                                            *dragged = Some(value);
                                            cx.notify();
                                        }
                                    });
                                }
                            }
                        });
                        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
                                return;
                            }
                            let released = dragged.update(cx, |dragged, cx| {
                                let released = dragged.take();
                                if released.is_some() {
                                    cx.notify();
                                }
                                released
                            });
                            if let (Some(value), Some(on_release)) = (released, &on_release) {
                                on_release(value, window, cx);
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
    }
}
