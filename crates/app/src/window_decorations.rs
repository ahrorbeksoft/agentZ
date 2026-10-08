//! The window's frame where agentZ draws it itself: on Linux, unless the desktop draws it.
//! Zed's `workspace::client_side_decorations` (a shadow and border to resize the window by)
//! and `platform_title_bar`'s Linux window controls.

use gpui::{
    AnyElement, Bounds, CursorStyle, Decorations, Div, Global, HitboxBehavior, Hsla, MouseButton,
    Point, ResizeEdge, Size, Stateful, Tiling, WindowButton, canvas, point, prelude::*, px, size,
    svg, transparent_black,
};
use ui::prelude::*;
use util::ResultExt as _;

/// Rounds the window's top corners where they aren't tiled. Zed rounds the bottom ones too,
/// where its status bar rounds its own; agentZ's sidebar and panes reach the bottom with
/// square backgrounds, which GPUI can't clip to a rounded corner, so they stay square.
pub trait RoundedTopCorners: Styled {
    fn rounded_top_corners(mut self, tiling: Tiling) -> Self {
        if !tiling.top && !tiling.left {
            self = self.rounded_tl(theme::CLIENT_SIDE_DECORATION_ROUNDING);
        }
        if !tiling.top && !tiling.right {
            self = self.rounded_tr(theme::CLIENT_SIDE_DECORATION_ROUNDING);
        }
        self
    }
}

impl<E: Styled> RoundedTopCorners for E {}

/// Which end of the title bar window controls go at, as the desktop lays them out.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// The window's content in its frame: with client-side decorations, a border, a shadow to
/// resize by and rounded corners, except where the window is tiled.
pub fn client_side_decorations(
    element: impl IntoElement,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let decorations = window.window_decorations();
    let is_resizable = window.is_resizable();
    let tiling = match decorations {
        Decorations::Server => Tiling::default(),
        Decorations::Client { tiling } => tiling,
    };

    match decorations {
        Decorations::Client { .. } => window.set_client_inset(theme::CLIENT_SIDE_DECORATION_SHADOW),
        Decorations::Server => window.set_client_inset(px(0.0)),
    }

    struct GlobalResizeEdge(ResizeEdge);
    impl Global for GlobalResizeEdge {}

    div()
        .id("window-backdrop")
        .bg(transparent_black())
        .map(|div| match decorations {
            Decorations::Server => div,
            Decorations::Client { .. } => div
                .rounded_top_corners(tiling)
                .when(!tiling.top, |div| {
                    div.pt(theme::CLIENT_SIDE_DECORATION_SHADOW)
                })
                .when(!tiling.bottom, |div| {
                    div.pb(theme::CLIENT_SIDE_DECORATION_SHADOW)
                })
                .when(!tiling.left, |div| {
                    div.pl(theme::CLIENT_SIDE_DECORATION_SHADOW)
                })
                .when(!tiling.right, |div| {
                    div.pr(theme::CLIENT_SIDE_DECORATION_SHADOW)
                })
                .when(is_resizable, |div| {
                    div.on_mouse_move(move |event, window, cx| {
                        let size = window.window_bounds().get_bounds().size;
                        let new_edge = resize_edge(
                            event.position,
                            theme::CLIENT_SIDE_DECORATION_SHADOW,
                            size,
                            tiling,
                        );
                        let edge = cx.try_global::<GlobalResizeEdge>();
                        if new_edge != edge.map(|edge| edge.0) {
                            window
                                .window_handle()
                                .update(cx, |root, _, cx| cx.notify(root.entity_id()))
                                .log_err();
                        }
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        move |event, window, _| {
                            let size = window.window_bounds().get_bounds().size;
                            if let Some(edge) = resize_edge(
                                event.position,
                                theme::CLIENT_SIDE_DECORATION_SHADOW,
                                size,
                                tiling,
                            ) {
                                window.start_window_resize(edge);
                            }
                        },
                    )
                }),
        })
        .size_full()
        .child(
            div()
                .cursor(CursorStyle::Arrow)
                .map(|div| match decorations {
                    Decorations::Server => div,
                    Decorations::Client { .. } => div
                        .border_color(cx.theme().colors().border)
                        .rounded_top_corners(tiling)
                        .when(!tiling.top, |div| {
                            div.border_t(theme::CLIENT_SIDE_DECORATION_BORDER)
                        })
                        .when(!tiling.bottom, |div| {
                            div.border_b(theme::CLIENT_SIDE_DECORATION_BORDER)
                        })
                        .when(!tiling.left, |div| {
                            div.border_l(theme::CLIENT_SIDE_DECORATION_BORDER)
                        })
                        .when(!tiling.right, |div| {
                            div.border_r(theme::CLIENT_SIDE_DECORATION_BORDER)
                        })
                        .when(!tiling.is_tiled(), |div| {
                            div.shadow(vec![
                                gpui::BoxShadow::new(
                                    px(0.),
                                    px(0.),
                                    Hsla {
                                        h: 0.,
                                        s: 0.,
                                        l: 0.,
                                        a: 0.4,
                                    },
                                )
                                .blur_radius(theme::CLIENT_SIDE_DECORATION_SHADOW / 2.),
                            ])
                        }),
                })
                .on_mouse_move(|_, _, cx| cx.stop_propagation())
                .size_full()
                .child(element),
        )
        .map(|div| match decorations {
            Decorations::Server => div,
            Decorations::Client { tiling } if is_resizable => div.child(
                canvas(
                    |_bounds, window, _| {
                        window.insert_hitbox(
                            Bounds::new(
                                point(px(0.0), px(0.0)),
                                window.window_bounds().get_bounds().size,
                            ),
                            HitboxBehavior::Normal,
                        )
                    },
                    move |_bounds, hitbox, window, cx| {
                        let mouse = window.mouse_position();
                        let size = window.window_bounds().get_bounds().size;
                        let Some(edge) =
                            resize_edge(mouse, theme::CLIENT_SIDE_DECORATION_SHADOW, size, tiling)
                        else {
                            return;
                        };
                        cx.set_global(GlobalResizeEdge(edge));
                        window.set_cursor_style(
                            match edge {
                                ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                                ResizeEdge::Left | ResizeEdge::Right => {
                                    CursorStyle::ResizeLeftRight
                                }
                                ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                                    CursorStyle::ResizeUpLeftDownRight
                                }
                                ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                                    CursorStyle::ResizeUpRightDownLeft
                                }
                            },
                            &hitbox,
                        );
                    },
                )
                .size_full()
                .absolute(),
            ),
            Decorations::Client { .. } => div,
        })
}

fn resize_edge(
    position: Point<Pixels>,
    shadow_size: Pixels,
    window_size: Size<Pixels>,
    tiling: Tiling,
) -> Option<ResizeEdge> {
    let bounds = Bounds::new(Point::default(), window_size).inset(shadow_size * 1.5);
    if bounds.contains(&position) {
        return None;
    }

    let corner_size = size(shadow_size * 1.5, shadow_size * 1.5);
    let top_left_bounds = Bounds::new(Point::new(px(0.), px(0.)), corner_size);
    if !tiling.top && top_left_bounds.contains(&position) {
        return Some(ResizeEdge::TopLeft);
    }

    let top_right_bounds = Bounds::new(
        Point::new(window_size.width - corner_size.width, px(0.)),
        corner_size,
    );
    if !tiling.top && top_right_bounds.contains(&position) {
        return Some(ResizeEdge::TopRight);
    }

    let bottom_left_bounds = Bounds::new(
        Point::new(px(0.), window_size.height - corner_size.height),
        corner_size,
    );
    if !tiling.bottom && bottom_left_bounds.contains(&position) {
        return Some(ResizeEdge::BottomLeft);
    }

    let bottom_right_bounds = Bounds::new(
        Point::new(
            window_size.width - corner_size.width,
            window_size.height - corner_size.height,
        ),
        corner_size,
    );
    if !tiling.bottom && bottom_right_bounds.contains(&position) {
        return Some(ResizeEdge::BottomRight);
    }

    if !tiling.top && position.y < shadow_size {
        Some(ResizeEdge::Top)
    } else if !tiling.bottom && position.y > window_size.height - shadow_size {
        Some(ResizeEdge::Bottom)
    } else if !tiling.left && position.x < shadow_size {
        Some(ResizeEdge::Left)
    } else if !tiling.right && position.x > window_size.width - shadow_size {
        Some(ResizeEdge::Right)
    } else {
        None
    }
}

/// Minimize, maximize and close at one end of the title bar, as the desktop lays them out,
/// when agentZ draws the window's frame.
pub fn window_controls(side: Side, window: &Window, cx: &App) -> Option<AnyElement> {
    if !matches!(window.window_decorations(), Decorations::Client { .. }) {
        return None;
    }
    let layout = cx.button_layout()?;
    let buttons = match side {
        Side::Left => layout.left,
        Side::Right => layout.right,
    };
    let supported = window.window_controls();
    let controls: Vec<AnyElement> = buttons
        .into_iter()
        .flatten()
        .filter(|button| match button {
            WindowButton::Minimize => supported.minimize,
            WindowButton::Maximize => supported.maximize,
            WindowButton::Close => true,
        })
        .map(|button| window_control(button, window, cx))
        .collect();
    if controls.is_empty() {
        return None;
    }
    Some(
        h_flex()
            .id(match side {
                Side::Left => "left-window-controls",
                Side::Right => "right-window-controls",
            })
            .gap_3()
            .px_3()
            // Keeps a press on a button from starting a window drag.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(controls)
            .into_any_element(),
    )
}

fn window_control(button: WindowButton, window: &Window, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let (icon, enabled) = match button {
        WindowButton::Minimize => (IconName::GenericMinimize, window.is_minimizable()),
        WindowButton::Maximize if window.is_maximized() => {
            (IconName::GenericRestore, window.is_resizable())
        }
        WindowButton::Maximize => (IconName::GenericMaximize, window.is_resizable()),
        WindowButton::Close => (IconName::GenericClose, true),
    };
    let icon_hover = colors.icon_muted;
    let background_hover = colors.ghost_element_hover;
    h_flex()
        .id(button.id())
        .group("")
        .justify_center()
        .content_center()
        .rounded_2xl()
        .w_5()
        .h_5()
        .when(enabled, |this| {
            this.cursor_pointer()
                .hover(|this| this.bg(background_hover))
                .active(|this| this.bg(background_hover))
        })
        .child(
            svg()
                .size_4()
                .flex_none()
                .path(icon.path())
                .text_color(if enabled {
                    colors.icon
                } else {
                    colors.icon_disabled
                })
                .when(enabled, |this| {
                    this.group_hover("", |this| this.text_color(icon_hover))
                }),
        )
        .on_mouse_move(|_, _, cx| cx.stop_propagation())
        .on_click(move |_, window, cx| {
            cx.stop_propagation();
            if !enabled {
                return;
            }
            match button {
                WindowButton::Minimize => window.minimize_window(),
                WindowButton::Maximize => window.zoom_window(),
                // agentZ has one window, so closing it quits.
                WindowButton::Close => window.remove_window(),
            }
        })
        .into_any_element()
}
