//! A thread's terminal or Changes panel moved out of the main window into its own, at the
//! user's request. The panel's owner keeps it; closing the window puts it back.

use gpui::{
    AnyView, App, AppContext as _, Bounds, Context, Entity, Point, Render, TitlebarOptions, Window,
    WindowBounds, WindowHandle, WindowOptions, px, size,
};
use ui::prelude::*;

/// A detached panel's first size.
const SIZE: gpui::Size<Pixels> = size(px(720.), px(480.));

pub struct DetachedPanel {
    content: AnyView,
}

impl DetachedPanel {
    /// Opens a window showing `content`, its top left at `origin` on screen (where a drag let
    /// go), or beside the main window. Returns the window and its panel, whose release means
    /// the window closed.
    pub fn open(
        title: SharedString,
        content: AnyView,
        origin: Option<Point<Pixels>>,
        window: &Window,
        cx: &mut App,
    ) -> Option<(WindowHandle<DetachedPanel>, Entity<DetachedPanel>)> {
        let main = window.bounds();
        let origin = origin.unwrap_or_else(|| {
            gpui::point(
                main.origin.x + main.size.width - SIZE.width / 2.,
                main.origin.y + px(80.),
            )
        });
        let mut panel = None;
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, SIZE))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(title),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |_, cx| {
                    let entity = cx.new(|_| DetachedPanel { content });
                    panel = Some(entity.clone());
                    entity
                },
            )
            .map_err(|error| log::error!("failed to open a window for the panel: {error:#}"))
            .ok()?;
        Some((handle, panel?))
    }

    /// Shows another view, as when the Changes panel follows a newly opened thread.
    pub fn set_content(&mut self, content: AnyView, cx: &mut Context<Self>) {
        self.content = content;
        cx.notify();
    }
}

impl Render for DetachedPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .size_full()
            .bg(colors.editor_background)
            .text_color(colors.text)
            .font_ui(cx)
            .text_ui(cx)
            .child(self.content.clone())
    }
}

/// Closes a detached panel's window.
pub fn close(handle: WindowHandle<DetachedPanel>, cx: &mut App) {
    handle
        .update(cx, |_, window, _| window.remove_window())
        .ok();
}
