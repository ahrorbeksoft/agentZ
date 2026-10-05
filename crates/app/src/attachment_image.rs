//! Images kept by a thread's server ([`agentz_protocol::attachments`]), fetched from it to show:
//! a thumbnail on hover, and the original in a viewer over the whole window, as t3code's
//! expanded image preview.

use std::sync::Arc;

use agentz_protocol::attachments::AttachmentId;
use agentz_protocol::{Request, Response};
use anyhow::{Context as _, anyhow};
use base64::Engine as _;
use gpui::{
    App, Asset, DismissEvent, EventEmitter, FocusHandle, Focusable, ImageCacheError, ImageFormat,
    ImageSource, KeyBinding, MouseButton, ObjectFit, RenderImage, Window, img,
};
use projects::ThreadId;
use ui::{CommonAnimationExt as _, Tooltip, prelude::*};

use crate::machines::{MachineId, Machines};

const VIEWER_KEY_CONTEXT: &str = "ImageViewer";
/// The composer's chip previews are this big at most, and the server's thumbnails fit them.
const PREVIEW_MAX_WIDTH: Pixels = px(320.);
const PREVIEW_MAX_HEIGHT: Pixels = px(240.);

pub fn init(cx: &mut App) {
    cx.bind_keys([KeyBinding::new(
        "escape",
        menu::Cancel,
        Some(VIEWER_KEY_CONTEXT),
    )]);
}

/// An image kept for a thread, on the thread's machine.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AttachmentImage {
    pub machine: MachineId,
    pub thread_id: ThreadId,
    pub id: AttachmentId,
}

impl AttachmentImage {
    /// A copy that fits a hover preview, which the server makes so a large image isn't sent
    /// whole.
    pub fn thumbnail(&self) -> ImageSource {
        self.source(true)
    }

    pub fn original(&self) -> ImageSource {
        self.source(false)
    }

    fn source(&self, thumbnail: bool) -> ImageSource {
        let request = AttachmentRequest {
            image: self.clone(),
            thumbnail,
        };
        ImageSource::Custom(Arc::new(move |window, cx| {
            let result = window.use_asset::<AttachmentLoader>(&request, cx)?;
            match result {
                Ok(image) => Some(Ok(image)),
                Err(failure) => {
                    // Asked for while the machine was offline: asked for again once it's back.
                    if failure.while_offline && is_online(request.image.machine, cx) {
                        cx.remove_asset::<AttachmentLoader>(&request);
                    }
                    Some(Err(failure.error))
                }
            }
        }))
    }
}

fn is_online(machine: MachineId, cx: &App) -> bool {
    Machines::global(cx)
        .read(cx)
        .client(machine, cx)
        .is_some_and(|client| client.read(cx).is_online())
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct AttachmentRequest {
    image: AttachmentImage,
    thumbnail: bool,
}

#[derive(Clone)]
struct LoadFailure {
    error: ImageCacheError,
    while_offline: bool,
}

/// Fetches an image from its thread's server ([`Request::Attachment`]) and decodes it.
enum AttachmentLoader {}

impl Asset for AttachmentLoader {
    type Source = AttachmentRequest;
    type Output = Result<Arc<RenderImage>, LoadFailure>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let client = Machines::global(cx)
            .read(cx)
            .client(source.image.machine, cx);
        let while_offline = !client
            .as_ref()
            .is_some_and(|client| client.read(cx).is_online());
        let response = client.map(|client| {
            client.read(cx).request(Request::Attachment {
                thread_id: source.image.thread_id,
                id: source.image.id.clone(),
                thumbnail: source.thumbnail,
            })
        });
        let svg_renderer = cx.svg_renderer();
        async move {
            let image = async {
                let response = response.context("the machine was removed")?.await?;
                let Response::AttachmentData(data) = response else {
                    return Err(anyhow!("unexpected response: {response:?}"));
                };
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data.data)
                    .context("decoding the image's base64")?;
                let format = ImageFormat::from_mime_type(&data.mime_type)
                    .with_context(|| format!("not an image: {}", data.mime_type))?;
                gpui::Image::from_bytes(format, bytes).to_image_data(svg_renderer)
            }
            .await;
            image.map_err(|error| {
                log::error!(
                    "failed to load the image {}: {error:#}",
                    source.image.id.as_str()
                );
                LoadFailure {
                    error: error.into(),
                    while_offline,
                }
            })
        }
    }
}

/// What hovering an image shows: a thumbnail, in a popover like the composer's chip previews.
pub(crate) fn render_hover_preview(source: ImageSource, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    div()
        .p_1()
        .bg(colors.elevated_surface_background)
        .border_1()
        .border_color(colors.border)
        .rounded_md()
        .shadow_md()
        .child(
            img(source)
                .max_w(PREVIEW_MAX_WIDTH)
                .max_h(PREVIEW_MAX_HEIGHT)
                .object_fit(ObjectFit::ScaleDown)
                .with_loading(|| {
                    div()
                        .p_2()
                        .child(
                            Icon::new(IconName::LoadCircle)
                                .size(IconSize::Small)
                                .color(Color::Muted)
                                .with_rotate_animation(2),
                        )
                        .into_any_element()
                })
                .with_fallback(|| {
                    div()
                        .px_2()
                        .py_1()
                        .child(
                            Label::new("Image unavailable")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        )
                        .into_any_element()
                }),
        )
        .into_any_element()
}

/// [`render_hover_preview`] as a tooltip, for an image shown small.
pub(crate) struct ImagePreviewTooltip(pub ImageSource);

impl Render for ImagePreviewTooltip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        render_hover_preview(self.0.clone(), cx)
    }
}

/// t3code's expanded image preview: the image as large as the window allows, over a dark
/// backdrop. Escape, the close button or a click beside the image closes it.
pub(crate) struct ImageViewer {
    focus_handle: FocusHandle,
    source: ImageSource,
}

impl EventEmitter<DismissEvent> for ImageViewer {}

impl Focusable for ImageViewer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ImageViewer {
    pub fn new(source: ImageSource, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            focus_handle,
            source,
        }
    }

    fn close(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }
}

impl Render for ImageViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        // t3code leaves room around the image for the close button and the backdrop to click.
        let max_width = viewport.width * 0.92;
        let max_height = (viewport.height * 0.86).min(viewport.height - px(120.));
        div()
            .id("image-viewer")
            .debug_selector(|| "image-viewer".into())
            .key_context(VIEWER_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::close))
            .w(viewport.width)
            .h(viewport.height)
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.8))
            // As the shell's modal backdrop: nothing under it gets the mouse.
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.emit(DismissEvent)),
            )
            .child(
                v_flex()
                    .items_end()
                    .gap_2()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        IconButton::new("close-image-viewer", IconName::Close)
                            .icon_size(IconSize::Small)
                            .tooltip(Tooltip::text("Close"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                    )
                    .child(
                        img(self.source.clone())
                            .debug_selector(|| "image-viewer-image".into())
                            .max_w(max_width)
                            .max_h(max_height)
                            .object_fit(ObjectFit::Contain)
                            .rounded_lg()
                            .with_loading(|| {
                                Icon::new(IconName::LoadCircle)
                                    .size(IconSize::Medium)
                                    .color(Color::Muted)
                                    .with_rotate_animation(2)
                                    .into_any_element()
                            })
                            .with_fallback(|| {
                                div()
                                    .p_6()
                                    .rounded_lg()
                                    .bg(gpui::black())
                                    .text_sm()
                                    .text_color(gpui::white())
                                    .child("Image unavailable. It may have been deleted.")
                                    .into_any_element()
                            }),
                    ),
            )
    }
}
