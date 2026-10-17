//! Images kept by a thread's server ([`agentz_protocol::attachments`]) and image files on its
//! machine, fetched from it to show: a thumbnail on hover, and the original in a viewer over the
//! whole window, as t3code's expanded image preview.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use agentz_protocol::attachments::AttachmentId;
use agentz_protocol::{Request, Response};
use anyhow::{Context as _, anyhow};
use base64::Engine as _;
use gpui::{
    App, Asset, Bounds, CursorStyle, DismissEvent, EventEmitter, FocusHandle, Focusable,
    ImageCacheError, ImageFormat, ImageSource, ImgResourceLoader, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PinchEvent, Point, RenderImage, Resource,
    ScrollWheelEvent, SharedUri, Size, Stateful, Window, img, point, size,
};
use projects::ThreadId;
use text_input::{CHIP_IMAGE_PREVIEW_SIZE, FittedImage, image_size};
use ui::{CommonAnimationExt as _, Tooltip, prelude::*};

use crate::machines::{MachineId, Machines};

const VIEWER_KEY_CONTEXT: &str = "ImageViewer";

gpui::actions!(
    image_viewer,
    [
        /// Shows the image before this one, or moves a zoomed image left.
        PreviousImage,
        /// Shows the image after this one, or moves a zoomed image right.
        NextImage,
        /// Moves a zoomed image up.
        PanUp,
        /// Moves a zoomed image down.
        PanDown,
        /// Zooms to 200%, or back to the whole image.
        ToggleZoom,
        /// Zooms in a step.
        ZoomIn,
        /// Zooms out a step.
        ZoomOut,
        /// Shows the whole image again.
        ZoomToFit,
    ]
);

pub fn init(cx: &mut App) {
    let context = Some(VIEWER_KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("escape", menu::Cancel, context),
        KeyBinding::new("left", PreviousImage, context),
        KeyBinding::new("right", NextImage, context),
        KeyBinding::new("up", PanUp, context),
        KeyBinding::new("down", PanDown, context),
        KeyBinding::new("enter", ToggleZoom, context),
        KeyBinding::new("space", ToggleZoom, context),
        KeyBinding::new("=", ZoomIn, context),
        KeyBinding::new("+", ZoomIn, context),
        KeyBinding::new("-", ZoomOut, context),
        KeyBinding::new("0", ZoomToFit, context),
    ]);
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
        image_source(AttachmentRequest {
            machine: self.machine,
            thread_id: self.thread_id,
            image: RequestedImage::Attachment(self.id.clone()),
            thumbnail,
        })
    }
}

/// An image file on a thread's machine, which its agent's markdown names or a tool read:
/// absolute, from `~`, or in the folder the thread works in ([`Request::ImageFile`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FileImage {
    pub machine: MachineId,
    pub thread_id: ThreadId,
    pub path: String,
}

impl FileImage {
    pub fn original(&self) -> ImageSource {
        image_source(AttachmentRequest {
            machine: self.machine,
            thread_id: self.thread_id,
            image: RequestedImage::File(self.path.clone()),
            thumbnail: false,
        })
    }
}

fn image_source(request: AttachmentRequest) -> ImageSource {
    ImageSource::Custom(Arc::new(move |window, cx| {
        let result = window.use_asset::<AttachmentLoader>(&request, cx)?;
        match result {
            Ok(image) => Some(Ok(image)),
            Err(failure) => {
                // Asked for while the machine was offline: asked for again once it's back.
                if failure.while_offline && is_online(request.machine, cx) {
                    cx.remove_asset::<AttachmentLoader>(&request);
                }
                Some(Err(failure.error))
            }
        }
    }))
}

/// A web image, loaded as `img` loads it, but as a custom source, so its size can be read
/// once it's loaded ([`text_input::image_size`]).
pub(crate) fn web_image(uri: String) -> ImageSource {
    let resource = Resource::Uri(SharedUri::from(uri));
    ImageSource::Custom(Arc::new(move |window, cx| {
        window.use_asset::<ImgResourceLoader>(&resource, cx)
    }))
}

/// Where a markdown image's bytes come from, as t3code's `classifyMarkdownImageSource` sorts
/// them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MarkdownImageSource {
    Web(String),
    Attachment(AttachmentId),
    /// A path on the thread's machine, absolute, from `~`, or in the thread's folder.
    File(String),
}

/// Where the image a markdown image names is: on the web, kept for the thread, or a file on
/// its machine. Anything else (another scheme, a fragment) isn't an image to load.
pub(crate) fn classify_markdown_image(source: &str) -> Option<MarkdownImageSource> {
    let source = source.trim();
    if source.is_empty() || source.starts_with('#') || source.starts_with('?') {
        return None;
    }
    let lowercase = source.to_ascii_lowercase();
    if lowercase.starts_with("http://") || lowercase.starts_with("https://") {
        return Some(MarkdownImageSource::Web(source.to_string()));
    }
    if source.starts_with("//") {
        return Some(MarkdownImageSource::Web(format!("https:{source}")));
    }
    if let Some(id) = AttachmentId::from_uri(source) {
        return Some(MarkdownImageSource::Attachment(id));
    }
    if lowercase.starts_with("file:") {
        let path = url::Url::parse(source).ok()?.to_file_path().ok()?;
        return Some(MarkdownImageSource::File(
            path.to_string_lossy().into_owned(),
        ));
    }
    let path = source.split(['?', '#']).next().unwrap_or_default();
    let path = percent_encoding::percent_decode_str(path)
        .decode_utf8()
        .ok()?
        .into_owned();
    let has_scheme = path.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|character: char| character.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "+.-".contains(character))
    });
    if path.is_empty() || has_scheme {
        return None;
    }
    Some(MarkdownImageSource::File(path))
}

/// Whether the image is still being fetched, so its size isn't known yet.
pub(crate) fn is_loading(source: &ImageSource, window: &mut Window, cx: &mut App) -> bool {
    match source {
        ImageSource::Custom(load) => load(window, cx).is_none(),
        _ => false,
    }
}

pub(crate) fn is_online(machine: MachineId, cx: &App) -> bool {
    Machines::global(cx)
        .read(cx)
        .client(machine, cx)
        .is_some_and(|client| client.read(cx).is_online())
}

const IMAGE_LINK_OPENING: &str = "[@Image](";

/// A message's image links ([`AttachmentId::markdown_link`]), and where each is in it.
fn image_links(text: &str) -> Vec<(Range<usize>, AttachmentId)> {
    let mut links = Vec::new();
    let mut cursor = 0;
    while let Some(found) = text[cursor..].find(IMAGE_LINK_OPENING) {
        let start = cursor + found;
        let uri_start = start + IMAGE_LINK_OPENING.len();
        cursor = uri_start;
        let Some(length) = text[uri_start..].find(')') else {
            break;
        };
        if let Some(id) = AttachmentId::from_uri(&text[uri_start..uri_start + length]) {
            cursor = uri_start + length + 1;
            links.push((start..cursor, id));
        }
    }
    links
}

/// A piece of an agent's message as it shows: text, or images side by side.
#[derive(Debug, PartialEq)]
pub(crate) enum MessagePiece<'a> {
    Text(&'a str),
    Images(Vec<AttachmentId>),
}

/// An agent's message cut at its images, which show where they are: the text between them,
/// and the images that only spaces part together.
pub(crate) fn message_pieces(text: &str) -> Vec<MessagePiece<'_>> {
    let links = image_links(text);
    if links.is_empty() {
        return vec![MessagePiece::Text(text)];
    }
    let mut pieces = Vec::new();
    let mut text_start = 0;
    for (range, id) in links {
        let before = text[text_start..range.start].trim();
        if !before.is_empty() {
            pieces.push(MessagePiece::Text(before));
        }
        match pieces.last_mut() {
            Some(MessagePiece::Images(images)) => images.push(id),
            _ => pieces.push(MessagePiece::Images(vec![id])),
        }
        text_start = range.end;
    }
    let rest = text[text_start..].trim();
    if !rest.is_empty() {
        pieces.push(MessagePiece::Text(rest));
    }
    pieces
}

/// The message's text without its image links, for where images don't show.
pub(crate) fn without_image_links(text: &str) -> Cow<'_, str> {
    let links = image_links(text);
    if links.is_empty() {
        return Cow::Borrowed(text);
    }
    let mut rest = String::new();
    let mut text_start = 0;
    for (range, _) in links {
        rest.push_str(&text[text_start..range.start]);
        text_start = range.end;
    }
    rest.push_str(&text[text_start..]);
    Cow::Owned(rest.trim().to_string())
}

/// The images a message links, in order.
pub(crate) fn message_images(text: &str) -> Vec<AttachmentId> {
    image_links(text).into_iter().map(|(_, id)| id).collect()
}

/// What copying an image's mention gives, as the composer's chip does.
const IMAGE_MENTION: &str = "@Image";

/// The message's text with each image link as `@Image`, as copying the composer's chip gives,
/// and where each is in the result.
pub(crate) fn with_image_names(text: &str) -> (String, Vec<(Range<usize>, AttachmentId)>) {
    let mut named = String::new();
    let mut images = Vec::new();
    let mut text_start = 0;
    for (range, id) in image_links(text) {
        named.push_str(&text[text_start..range.start]);
        let start = named.len();
        named.push_str(IMAGE_MENTION);
        images.push((start..named.len(), id));
        text_start = range.end;
    }
    named.push_str(&text[text_start..]);
    (named, images)
}

/// An image link drawn as the composer draws the image's mention: an outlined box with the
/// image icon.
pub(crate) fn image_chip(url: &str) -> Option<markdown::LinkChip> {
    AttachmentId::from_uri(url)?;
    Some(markdown::LinkChip {
        icon: IconName::Image.path().into(),
        label: "Image".into(),
        copy_text: IMAGE_MENTION.into(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct AttachmentRequest {
    machine: MachineId,
    thread_id: ThreadId,
    image: RequestedImage,
    thumbnail: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum RequestedImage {
    Attachment(AttachmentId),
    File(String),
}

impl RequestedImage {
    fn name(&self) -> &str {
        match self {
            RequestedImage::Attachment(id) => id.as_str(),
            RequestedImage::File(path) => path,
        }
    }
}

/// An image a server couldn't send, and whether its machine was offline when it was asked
/// for, so it's asked for again once it's back.
#[derive(Clone)]
pub(crate) struct LoadFailure {
    pub error: ImageCacheError,
    pub while_offline: bool,
}

/// Fetches an image from its thread's server ([`Request::Attachment`], [`Request::ImageFile`])
/// and decodes it.
enum AttachmentLoader {}

impl Asset for AttachmentLoader {
    type Source = AttachmentRequest;
    type Output = Result<Arc<RenderImage>, LoadFailure>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let client = Machines::global(cx).read(cx).client(source.machine, cx);
        let while_offline = !client
            .as_ref()
            .is_some_and(|client| client.read(cx).is_online());
        let request = match &source.image {
            RequestedImage::Attachment(id) => Request::Attachment {
                thread_id: source.thread_id,
                id: id.clone(),
                thumbnail: source.thumbnail,
            },
            RequestedImage::File(path) => Request::ImageFile {
                thread_id: source.thread_id,
                path: path.clone(),
            },
        };
        let response = client.map(|client| client.read(cx).request(request));
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
                    source.image.name()
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
            FittedImage::new(source, CHIP_IMAGE_PREVIEW_SIZE).map_image(|image| {
                image
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
                    })
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

/// An image the viewer shows, and the name under it.
#[derive(Clone)]
pub(crate) struct ViewedImage {
    pub source: ImageSource,
    pub name: SharedString,
}

const MAX_ZOOM: f32 = 8.;
const CLICK_ZOOM: f32 = 2.;
const KEY_ZOOM_STEP: f32 = 1.5;
/// How far a press on a zoomed image moves before it's a drag rather than a click.
const DRAG_DISTANCE: f32 = 4.;
const PAN_STEP: Pixels = px(40.);
const ARROW_SIZE: Pixels = px(32.);

/// t3code's expanded image preview: the image as large as the window allows, over a dark
/// backdrop, with arrows to the other images it was opened with. A click zooms to 200% where
/// it's clicked and back; scrolling zooms, and dragging moves a zoomed image. Escape, the
/// close button or a click beside the image closes it.
pub(crate) struct ImageViewer {
    focus_handle: FocusHandle,
    images: Vec<ViewedImage>,
    index: usize,
    zoom: f32,
    /// How far a zoomed image is scrolled in its frame.
    scroll: Point<Pixels>,
    /// The shown image's size, once it's loaded.
    image_size: Option<Size<Pixels>>,
    /// The window's size the image was last fitted to.
    viewport: Size<Pixels>,
    press: Option<Press>,
}

/// A press on the image: a click zooms, and a drag moves a zoomed image.
struct Press {
    position: Point<Pixels>,
    scroll: Point<Pixels>,
    is_drag: bool,
}

/// Where the viewer draws an image: fitted to the room the window has for it, then zoomed, in
/// a frame centered in the window that shows as much of it as fits.
#[derive(Clone, Copy, Debug)]
struct ViewerLayout {
    image: Size<Pixels>,
    frame: Bounds<Pixels>,
}

impl ViewerLayout {
    fn new(viewport: Size<Pixels>, image: Size<Pixels>, zoom: f32) -> Self {
        let room = viewer_room(viewport);
        let fit = (room.width / image.width)
            .min(room.height / image.height)
            .min(1.);
        let image = size(image.width * (fit * zoom), image.height * (fit * zoom));
        let frame = size(image.width.min(room.width), image.height.min(room.height));
        let origin = point(
            (viewport.width - frame.width) * 0.5,
            (viewport.height - frame.height) * 0.5,
        );
        Self {
            image,
            frame: Bounds::new(origin, frame),
        }
    }

    fn clamp_scroll(&self, scroll: Point<Pixels>) -> Point<Pixels> {
        let max = point(
            (self.image.width - self.frame.size.width).max(px(0.)),
            (self.image.height - self.frame.size.height).max(px(0.)),
        );
        scroll.clamp(&Point::default(), &max)
    }
}

/// t3code's room for an image: most of the window, less room for the arrows beside it, and
/// for the close button and name above and below it.
fn viewer_room(viewport: Size<Pixels>) -> Size<Pixels> {
    let arrows = if viewport.width >= px(640.) {
        px(96.)
    } else {
        px(0.)
    };
    size(
        (viewport.width * 0.92 - arrows).max(px(1.)),
        (viewport.height * 0.86)
            .min(viewport.height - px(160.))
            .max(px(1.)),
    )
}

impl EventEmitter<DismissEvent> for ImageViewer {}

impl Focusable for ImageViewer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ImageViewer {
    /// Shows `images[index]`, with arrows to the others.
    pub fn new(
        images: Vec<ViewedImage>,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            focus_handle,
            index: index.min(images.len().saturating_sub(1)),
            images,
            zoom: 1.,
            scroll: Point::default(),
            image_size: None,
            viewport: window.viewport_size(),
            press: None,
        }
    }

    fn close(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    /// Shows the next or previous image, wrapping around as t3code's does, whole.
    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let count = self.images.len();
        if count < 2 {
            return;
        }
        self.index = if forward {
            (self.index + 1) % count
        } else {
            (self.index + count - 1) % count
        };
        self.zoom = 1.;
        self.scroll = Point::default();
        self.image_size = None;
        self.press = None;
        cx.notify();
    }

    fn layout(&self, viewport: Size<Pixels>) -> Option<ViewerLayout> {
        Some(ViewerLayout::new(viewport, self.image_size?, self.zoom))
    }

    /// Moves a zoomed image. Sideways, an image no wider than its frame doesn't move, so the
    /// arrow keys step through the images instead, as in t3code.
    fn pan(&mut self, delta: Point<Pixels>, window: &Window, cx: &mut Context<Self>) -> bool {
        if self.zoom <= 1. {
            return false;
        }
        let Some(layout) = self.layout(window.viewport_size()) else {
            return false;
        };
        if delta.x != px(0.) && layout.image.width <= layout.frame.size.width {
            return false;
        }
        self.scroll = layout.clamp_scroll(self.scroll + delta);
        cx.notify();
        true
    }

    fn previous(&mut self, _: &PreviousImage, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pan(point(-PAN_STEP, px(0.)), window, cx) {
            self.step(false, cx);
        }
    }

    fn next(&mut self, _: &NextImage, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pan(point(PAN_STEP, px(0.)), window, cx) {
            self.step(true, cx);
        }
    }

    fn pan_up(&mut self, _: &PanUp, window: &mut Window, cx: &mut Context<Self>) {
        self.pan(point(px(0.), -PAN_STEP), window, cx);
    }

    fn pan_down(&mut self, _: &PanDown, window: &mut Window, cx: &mut Context<Self>) {
        self.pan(point(px(0.), PAN_STEP), window, cx);
    }

    fn toggle_zoom(&mut self, _: &ToggleZoom, window: &mut Window, cx: &mut Context<Self>) {
        self.toggle_zoom_at(None, window, cx);
    }

    fn zoom_in(&mut self, _: &ZoomIn, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom_to(self.zoom * KEY_ZOOM_STEP, None, window, cx);
    }

    fn zoom_out(&mut self, _: &ZoomOut, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom_to(self.zoom / KEY_ZOOM_STEP, None, window, cx);
    }

    fn zoom_to_fit(&mut self, _: &ZoomToFit, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom_to(1., None, window, cx);
    }

    fn toggle_zoom_at(
        &mut self,
        at: Option<Point<Pixels>>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let zoom = if self.zoom > 1. { 1. } else { CLICK_ZOOM };
        self.zoom_to(zoom, at, window, cx);
    }

    /// Zooms, keeping the point of the image at `at` (or the frame's center) where it is, as
    /// t3code does.
    fn zoom_to(
        &mut self,
        zoom: f32,
        at: Option<Point<Pixels>>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let zoom = zoom.clamp(1., MAX_ZOOM);
        let viewport = window.viewport_size();
        let (Some(image_size), Some(before)) = (self.image_size, self.layout(viewport)) else {
            return;
        };
        if zoom == self.zoom {
            return;
        }
        let at = at.unwrap_or_else(|| before.frame.center());
        // Where `at` is on the image, at its fitted size.
        let anchor = (self.scroll + (at - before.frame.origin)) * (1. / self.zoom);
        let after = ViewerLayout::new(viewport, image_size, zoom);
        self.zoom = zoom;
        self.scroll = after.clamp_scroll(anchor * zoom - (at - after.frame.origin));
        cx.notify();
    }

    fn press(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        // A press on the image isn't one beside it, which closes the viewer.
        cx.stop_propagation();
        self.press = Some(Press {
            position: event.position,
            scroll: self.scroll,
            is_drag: false,
        });
        cx.notify();
    }

    fn drag(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.press.is_none() {
            return;
        }
        // Released outside the window.
        if event.pressed_button != Some(MouseButton::Left) {
            self.press = None;
            cx.notify();
            return;
        }
        let is_zoomed = self.zoom > 1.;
        let Some(press) = self.press.as_mut().filter(|_| is_zoomed) else {
            return;
        };
        let moved = event.position - press.position;
        if f32::from(moved.x).hypot(f32::from(moved.y)) > DRAG_DISTANCE {
            press.is_drag = true;
        }
        let scroll = press.scroll - moved;
        if let Some(layout) = self.layout(window.viewport_size()) {
            self.scroll = layout.clamp_scroll(scroll);
        }
        cx.notify();
    }

    fn release(&mut self, event: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(press) = self.press.take() else {
            return;
        };
        // A drag ends with a release that isn't a click, and a double click zooms once.
        if !press.is_drag && event.click_count <= 1 {
            self.toggle_zoom_at(Some(event.position), window, cx);
        }
        cx.notify();
    }

    fn wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = f32::from(event.delta.pixel_delta(px(16.)).y);
        if delta == 0. {
            return;
        }
        cx.stop_propagation();
        // t3code's rates: faster with control, which browsers add to a trackpad's pinch.
        let rate = if event.modifiers.control { 0.01 } else { 0.002 };
        self.zoom_to(
            self.zoom * (delta * rate).exp(),
            Some(event.position),
            window,
            cx,
        );
    }

    fn pinch(&mut self, event: &PinchEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.zoom_to(
            self.zoom * (1. + event.delta),
            Some(event.position),
            window,
            cx,
        );
    }

    /// The image's name, where it is among the others, and its zoom.
    pub(crate) fn caption(&self) -> String {
        let Some(image) = self.images.get(self.index) else {
            return String::new();
        };
        let mut caption = image.name.to_string();
        if self.images.len() > 1 {
            caption.push_str(&format!(" · {} of {}", self.index + 1, self.images.len()));
        }
        if self.zoom > 1. {
            caption.push_str(&format!(" · {}% zoom", (self.zoom * 100.).round()));
        }
        caption
    }

    /// A loaded image, in its frame, with the close button above it and the caption under it.
    fn render_frame(
        &self,
        source: ImageSource,
        layout: ViewerLayout,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let frame = layout.frame;
        let room = viewer_room(self.viewport);
        let cursor = if self.zoom <= 1. {
            CursorStyle::ZoomIn
        } else if self.press.is_some() {
            CursorStyle::ClosedHand
        } else {
            CursorStyle::OpenHand
        };
        div()
            .absolute()
            .left(frame.origin.x)
            .top(frame.origin.y)
            .w(frame.size.width)
            .h(frame.size.height)
            .child(
                div()
                    .id("image-viewer-frame")
                    .debug_selector(|| "image-viewer-frame".into())
                    .relative()
                    .size_full()
                    .overflow_hidden()
                    .rounded_lg()
                    .cursor(cursor)
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::press))
                    .on_scroll_wheel(cx.listener(Self::wheel))
                    .on_pinch(cx.listener(Self::pinch))
                    .child(
                        img(source)
                            .debug_selector(|| "image-viewer-image".into())
                            .absolute()
                            .left(-self.scroll.x)
                            .top(-self.scroll.y)
                            .w(layout.image.width)
                            .h(layout.image.height)
                            .rounded_lg(),
                    ),
            )
            .child(
                div()
                    .absolute()
                    .top(px(-30.))
                    .right_0()
                    .child(self.render_close_button(cx)),
            )
            .child(
                self.render_caption()
                    .absolute()
                    .top(frame.size.height + px(8.))
                    .left((frame.size.width - room.width) * 0.5)
                    .w(room.width),
            )
            .into_any_element()
    }

    /// An image loading, or one that couldn't load, as large as it may be once it's shown.
    fn render_unsized(&self, source: ImageSource, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .items_end()
            .gap_2()
            .child(self.render_close_button(cx))
            .child(
                FittedImage::new(source, viewer_room(self.viewport)).map_image(|image| {
                    image
                        .debug_selector(|| "image-viewer-image".into())
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
                        })
                }),
            )
            .child(self.render_caption().w_full())
            .into_any_element()
    }

    fn render_close_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                IconButton::new("close-image-viewer", IconName::Close)
                    .icon_size(IconSize::Small)
                    .tooltip(Tooltip::text("Close"))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
            )
    }

    fn render_caption(&self) -> Div {
        div()
            .flex()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .debug_selector(|| "image-viewer-caption".into())
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(gpui::white().opacity(0.8))
                    .child(self.caption()),
            )
    }

    fn render_arrow(
        &self,
        id: &'static str,
        icon: IconName,
        label: &'static str,
        forward: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .debug_selector(move || id.into())
            .absolute()
            .size(ARROW_SIZE)
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(gpui::white().opacity(0.12))
            .hover(|style| style.bg(gpui::white().opacity(0.2)))
            .cursor_pointer()
            .child(
                Icon::new(icon)
                    .size(IconSize::Small)
                    .color(Color::Custom(gpui::white())),
            )
            .tooltip(Tooltip::text(label))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, _, _, cx| this.step(forward, cx)))
    }
}

impl Render for ImageViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        if viewport != self.viewport {
            // As t3code's: a resized window shows the image whole again.
            self.viewport = viewport;
            self.zoom = 1.;
            self.scroll = Point::default();
        }
        let source = self
            .images
            .get(self.index)
            .map(|image| image.source.clone());
        self.image_size = source
            .as_ref()
            .and_then(|source| image_size(source, window, cx));
        let content = source.map(|source| match self.layout(viewport) {
            Some(layout) => {
                self.scroll = layout.clamp_scroll(self.scroll);
                self.render_frame(source, layout, cx)
            }
            None => self.render_unsized(source, cx),
        });
        let arrows = (self.images.len() > 1).then(|| {
            let top = (viewport.height - ARROW_SIZE) * 0.5;
            let inset = viewport.width * 0.04;
            [
                self.render_arrow(
                    "image-viewer-previous",
                    IconName::ChevronLeft,
                    "Previous Image",
                    false,
                    cx,
                )
                .left(inset)
                .top(top),
                self.render_arrow(
                    "image-viewer-next",
                    IconName::ChevronRight,
                    "Next Image",
                    true,
                    cx,
                )
                .right(inset)
                .top(top),
            ]
        });
        div()
            .id("image-viewer")
            .debug_selector(|| "image-viewer".into())
            .key_context(VIEWER_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::close))
            .on_action(cx.listener(Self::previous))
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::pan_up))
            .on_action(cx.listener(Self::pan_down))
            .on_action(cx.listener(Self::toggle_zoom))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::zoom_to_fit))
            .relative()
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
            .on_mouse_move(cx.listener(Self::drag))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::release))
            .children(content)
            .children(arrows.into_iter().flatten())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        Entity, Modifiers, RenderImage, ScrollDelta, TestAppContext, TouchPhase, VisualTestContext,
    };

    use super::*;

    fn id(digit: &str) -> AttachmentId {
        AttachmentId::parse(&format!("{}.png", digit.repeat(64))).expect("an id")
    }

    #[test]
    fn an_agents_images_show_where_they_are() {
        let (first, second, third) = (id("a"), id("b"), id("c"));
        let text = format!(
            "Before:\n\n{} {}\n\nBetween {} after.",
            first.markdown_link(),
            second.markdown_link(),
            third.markdown_link()
        );
        assert_eq!(
            message_pieces(&text),
            [
                MessagePiece::Text("Before:"),
                MessagePiece::Images(vec![first, second]),
                MessagePiece::Text("Between"),
                MessagePiece::Images(vec![third]),
                MessagePiece::Text("after."),
            ]
        );
        let text = "Not one: [@Image](https://example.com/a.png) ";
        assert_eq!(message_pieces(text), [MessagePiece::Text(text)]);
    }

    #[test]
    fn a_messages_images_can_be_left_out_or_named() {
        let (first, second) = (id("a"), id("b"));
        let text = format!(
            "{} {} The sidebar is cut off.",
            first.markdown_link(),
            second.markdown_link()
        );
        assert_eq!(without_image_links(&text), "The sidebar is cut off.");
        assert_eq!(without_image_links("Plain"), "Plain");
        assert_eq!(message_images(&text), [first.clone(), second.clone()]);

        let text = format!(
            "In this {} it looks like that, and this: {}.",
            first.markdown_link(),
            second.markdown_link()
        );
        let (named, images) = with_image_names(&text);
        assert_eq!(
            named,
            "In this @Image it looks like that, and this: @Image."
        );
        assert_eq!(images.len(), 2);
        assert_eq!(&named[images[0].0.clone()], "@Image");
        assert_eq!(images[0].1, first);
        assert_eq!(&named[images[1].0.clone()], "@Image");
        assert_eq!(images[1].1, second);
        assert_eq!(with_image_names("Plain"), ("Plain".to_string(), Vec::new()));
    }

    #[test]
    fn only_an_attachments_link_is_an_image_chip() {
        let link = id("a").markdown_link();
        let uri = link
            .trim_start_matches(IMAGE_LINK_OPENING)
            .trim_end_matches(')');
        assert_eq!(
            image_chip(uri).map(|chip| chip.copy_text.to_string()),
            Some("@Image".to_string())
        );
        assert!(image_chip("https://example.com/a.png").is_none());
    }

    #[test]
    fn markdown_images_load_from_the_web_or_the_threads_machine() {
        use MarkdownImageSource::*;
        let file = |path: &str| Some(File(path.to_string()));
        assert_eq!(
            classify_markdown_image("https://example.com/a.png"),
            Some(Web("https://example.com/a.png".into()))
        );
        assert_eq!(
            classify_markdown_image("//example.com/a.png"),
            Some(Web("https://example.com/a.png".into()))
        );
        assert_eq!(
            classify_markdown_image(&id("a").uri()),
            Some(Attachment(id("a")))
        );
        assert_eq!(
            classify_markdown_image("shots/cart.png"),
            file("shots/cart.png")
        );
        assert_eq!(
            classify_markdown_image("./my%20cart.png?raw=1#top"),
            file("./my cart.png")
        );
        assert_eq!(
            classify_markdown_image("/tmp/cart.png"),
            file("/tmp/cart.png")
        );
        assert_eq!(classify_markdown_image("~/cart.png"), file("~/cart.png"));
        assert_eq!(
            classify_markdown_image("file:///tmp/my%20cart.png"),
            file("/tmp/my cart.png")
        );
        assert_eq!(classify_markdown_image("mailto:someone@example.com"), None);
        assert_eq!(
            classify_markdown_image("data:image/png;base64,broken"),
            None
        );
        assert_eq!(classify_markdown_image("#top"), None);
        assert_eq!(classify_markdown_image(""), None);
    }

    /// A blank image of this many pixels, as the viewer has one once it's loaded.
    fn blank(width: u32, height: u32) -> ImageSource {
        let frame = image::Frame::new(image::RgbaImage::new(width, height));
        ImageSource::Render(Arc::new(RenderImage::new([frame])))
    }

    /// A viewer of blank images of these sizes and names, and whether it was dismissed.
    fn open_viewer<'a>(
        images: &[(u32, u32, &str)],
        cx: &'a mut TestAppContext,
    ) -> (
        Entity<ImageViewer>,
        Rc<Cell<bool>>,
        &'a mut VisualTestContext,
    ) {
        cx.update(crate::init_for_test);
        let images = images
            .iter()
            .map(|(width, height, name)| ViewedImage {
                source: blank(*width, *height),
                name: name.to_string().into(),
            })
            .collect();
        let (viewer, cx) = cx.add_window_view(|window, cx| ImageViewer::new(images, 0, window, cx));
        let dismissed = Rc::new(Cell::new(false));
        cx.update(|_, cx| {
            let dismissed = dismissed.clone();
            cx.subscribe(&viewer, move |_, _: &DismissEvent, _| dismissed.set(true))
                .detach();
        });
        cx.run_until_parked();
        (viewer, dismissed, cx)
    }

    fn caption(viewer: &Entity<ImageViewer>, cx: &mut VisualTestContext) -> String {
        viewer.read_with(cx, |viewer, _| viewer.caption())
    }

    fn image_bounds(cx: &mut VisualTestContext) -> Bounds<Pixels> {
        cx.debug_bounds("image-viewer-image")
            .expect("the viewer's image")
    }

    /// The arrows and the ← → keys step through the images, around from the last to the
    /// first, each named with where it is.
    #[gpui::test]
    fn arrows_step_through_the_images(cx: &mut TestAppContext) {
        let (viewer, dismissed, cx) =
            open_viewer(&[(800, 600, "one.png"), (600, 800, "two.png")], cx);
        assert_eq!(caption(&viewer, cx), "one.png · 1 of 2");
        cx.simulate_keystrokes("right");
        assert_eq!(caption(&viewer, cx), "two.png · 2 of 2");
        cx.simulate_keystrokes("right");
        assert_eq!(caption(&viewer, cx), "one.png · 1 of 2");
        cx.simulate_keystrokes("left");
        assert_eq!(caption(&viewer, cx), "two.png · 2 of 2");

        let next = cx
            .debug_bounds("image-viewer-next")
            .expect("arrows, with more than one image");
        cx.simulate_click(next.center(), Modifiers::none());
        assert_eq!(caption(&viewer, cx), "one.png · 1 of 2");
        assert!(!dismissed.get());

        cx.simulate_click(gpui::point(px(4.), px(4.)), Modifiers::none());
        assert!(dismissed.get(), "a click beside the image closes it");
    }

    /// t3code's zoom: a click zooms to 200% keeping the point clicked under the mouse, and
    /// another shows the whole image again.
    #[gpui::test]
    fn a_click_zooms_in_where_it_is_and_back(cx: &mut TestAppContext) {
        let (viewer, dismissed, cx) = open_viewer(&[(1600, 1000, "wide.png")], cx);
        assert_eq!(caption(&viewer, cx), "wide.png");
        assert!(cx.debug_bounds("image-viewer-previous").is_none());
        let fitted = image_bounds(cx);
        let at = fitted.origin + gpui::point(fitted.size.width * 0.4, fitted.size.height * 0.6);
        cx.simulate_click(at, Modifiers::none());
        assert_eq!(caption(&viewer, cx), "wide.png · 200% zoom");
        let zoomed = image_bounds(cx);
        assert_eq!(zoomed.size.width, fitted.size.width * 2.);
        let on_image = (
            (at.x - zoomed.left()) / zoomed.size.width,
            (at.y - zoomed.top()) / zoomed.size.height,
        );
        assert!(
            (on_image.0 - 0.4).abs() < 0.01 && (on_image.1 - 0.6).abs() < 0.01,
            "{on_image:?}"
        );

        cx.simulate_click(at, Modifiers::none());
        assert_eq!(caption(&viewer, cx), "wide.png");
        assert_eq!(image_bounds(cx), fitted);
        assert!(!dismissed.get());
    }

    /// Scrolling zooms at the mouse, and dragging moves a zoomed image without zooming back.
    #[gpui::test]
    fn scrolling_zooms_and_dragging_moves_the_image(cx: &mut TestAppContext) {
        let (viewer, _, cx) = open_viewer(&[(1600, 1000, "wide.png")], cx);
        let center = image_bounds(cx).center();
        cx.simulate_event(ScrollWheelEvent {
            position: center,
            delta: ScrollDelta::Pixels(gpui::point(px(0.), px(200.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        assert_eq!(caption(&viewer, cx), "wide.png · 149% zoom");

        let zoomed = image_bounds(cx);
        let moved = center - gpui::point(px(100.), px(0.));
        cx.simulate_mouse_down(center, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_move(moved, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(moved, MouseButton::Left, Modifiers::none());
        assert_eq!(image_bounds(cx).left(), zoomed.left() - px(100.));
        assert_eq!(caption(&viewer, cx), "wide.png · 149% zoom");

        cx.simulate_event(ScrollWheelEvent {
            position: center,
            delta: ScrollDelta::Pixels(gpui::point(px(0.), px(-2000.))),
            modifiers: Modifiers::none(),
            touch_phase: TouchPhase::Moved,
        });
        assert_eq!(
            caption(&viewer, cx),
            "wide.png",
            "no smaller than the whole image"
        );
    }
}
