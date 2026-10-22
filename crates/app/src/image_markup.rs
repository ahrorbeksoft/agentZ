//! Marking up an image in the composer before it's sent, as macOS Quick Look's Markup does
//! (`design/composer-2/decisions.md`, topics 4 to 6). The marks stay apart from the image, in
//! its own pixels, so they can be moved, erased or undone until the message goes. The agent
//! gets a copy with them drawn in ([`draw_marks`]), and the numbered notes as text
//! ([`notes_text`]).

use std::collections::HashMap;
use std::io::Cursor;

use agentz_protocol::attachments::AttachmentId;
use anyhow::Context as _;
use gpui::{
    App, DevicePixels, Entity, Focusable as _, FontWeight, Global, Hsla, MouseButton, PathBuilder,
    PathStyle, Pixels, Point, StrokeOptions, Subscription, SvgRenderer, SvgSize, Window, canvas,
    point, size,
};
use lyon::tessellation::{LineCap, LineJoin};
use text_input::{TextInput, TextInputEvent};
use ui::{Tooltip, prelude::*};

use crate::attachment_image::{AttachmentImage, ImageViewer};

/// The key context of the field a label or note is typed in.
pub(crate) const MARKUP_TEXT_KEY_CONTEXT: &str = "ImageMarkupText";

/// Sizes in points of the screen, at the zoom a mark was made at.
const LINE_WIDTH: f32 = 3.;
const TEXT_SIZE: f32 = 16.;
const TEXT_LINE_HEIGHT: f32 = 1.25;
const PIN_SIZE: f32 = 20.;
const PIN_TEXT_SIZE: f32 = 11.;
const ARROW_HEAD: f32 = 14.;
/// How near a press must be to a mark to pick it, in points of the screen.
const PICK_DISTANCE: f32 = 6.;
/// How far a rectangle or arrow must be dragged to be kept, in points of the screen.
const SMALLEST_SHAPE: f32 = 3.;

/// What a press on the image does while marking up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MarkupTool {
    Pen,
    Rectangle,
    Arrow,
    Text,
    Note,
    Eraser,
}

impl MarkupTool {
    /// The tools that make marks, in the toolbar's order.
    pub const DRAWING: [MarkupTool; 5] = [
        MarkupTool::Pen,
        MarkupTool::Rectangle,
        MarkupTool::Arrow,
        MarkupTool::Text,
        MarkupTool::Note,
    ];

    fn icon(self) -> IconName {
        match self {
            MarkupTool::Pen => IconName::Pencil,
            MarkupTool::Rectangle => IconName::Square,
            MarkupTool::Arrow => IconName::ArrowUpRight,
            MarkupTool::Text => IconName::Font,
            MarkupTool::Note => IconName::Chat,
            MarkupTool::Eraser => IconName::Eraser,
        }
    }

    fn label(self) -> &'static str {
        match self {
            MarkupTool::Pen => "Pen",
            MarkupTool::Rectangle => "Rectangle",
            MarkupTool::Arrow => "Arrow",
            MarkupTool::Text => "Text",
            MarkupTool::Note => "Numbered Note",
            MarkupTool::Eraser => "Eraser",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            MarkupTool::Pen => "image-markup-pen",
            MarkupTool::Rectangle => "image-markup-rectangle",
            MarkupTool::Arrow => "image-markup-arrow",
            MarkupTool::Text => "image-markup-text",
            MarkupTool::Note => "image-markup-note",
            MarkupTool::Eraser => "image-markup-eraser",
        }
    }
}

/// macOS Markup's colors, as the round's mocks have them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MarkupColor {
    #[default]
    Red,
    Yellow,
    Blue,
    Green,
    White,
}

impl MarkupColor {
    pub const ALL: [MarkupColor; 5] = [
        MarkupColor::Red,
        MarkupColor::Yellow,
        MarkupColor::Blue,
        MarkupColor::Green,
        MarkupColor::White,
    ];

    fn rgb(self) -> u32 {
        match self {
            MarkupColor::Red => 0xff453a,
            MarkupColor::Yellow => 0xffd60a,
            MarkupColor::Blue => 0x0a84ff,
            MarkupColor::Green => 0x30d158,
            MarkupColor::White => 0xf5f5f7,
        }
    }

    /// What a pin's number is in, to read on the pin.
    fn number_rgb(self) -> u32 {
        match self {
            MarkupColor::Red | MarkupColor::Blue => 0xffffff,
            MarkupColor::Yellow | MarkupColor::Green | MarkupColor::White => 0x1d1d1f,
        }
    }

    fn hsla(self) -> Hsla {
        gpui::rgb(self.rgb()).into()
    }

    fn label(self) -> &'static str {
        match self {
            MarkupColor::Red => "Red",
            MarkupColor::Yellow => "Yellow",
            MarkupColor::Blue => "Blue",
            MarkupColor::Green => "Green",
            MarkupColor::White => "White",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            MarkupColor::Red => "image-markup-red",
            MarkupColor::Yellow => "image-markup-yellow",
            MarkupColor::Blue => "image-markup-blue",
            MarkupColor::Green => "image-markup-green",
            MarkupColor::White => "image-markup-white",
        }
    }
}

/// A mark, in the image's own pixels, so it stays where it was put at any zoom.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Mark {
    pub shape: Shape,
    pub color: MarkupColor,
    /// How many of the image's pixels a point of the screen was when it was made, which its
    /// line and text are sized in: it's drawn in as it was seen.
    pub unit: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Shape {
    Pen(Vec<Point<f32>>),
    Rectangle {
        from: Point<f32>,
        to: Point<f32>,
    },
    /// From its tail to its point.
    Arrow {
        from: Point<f32>,
        to: Point<f32>,
    },
    /// A label, from its top left.
    Text {
        at: Point<f32>,
        text: String,
    },
    /// A numbered pin, at its center. Only the pin is drawn; the note goes with the message as
    /// text.
    Note {
        at: Point<f32>,
        text: String,
    },
}

impl Mark {
    pub fn new(shape: Shape, color: MarkupColor, unit: f32) -> Self {
        Self { shape, color, unit }
    }

    fn line_width(&self) -> f32 {
        LINE_WIDTH * self.unit
    }

    fn text_size(&self) -> f32 {
        TEXT_SIZE * self.unit
    }

    fn pin_radius(&self) -> f32 {
        PIN_SIZE * 0.5 * self.unit
    }

    /// A label's box, guessed from its length, since it's drawn by GPUI and by the SVG renderer
    /// with fonts that differ a little.
    fn text_box(&self, at: Point<f32>, text: &str) -> (Point<f32>, Point<f32>) {
        let size = self.text_size();
        let width = size * 0.6 * text.chars().count().max(1) as f32;
        (at, point(at.x + width, at.y + size * TEXT_LINE_HEIGHT))
    }

    /// How far `at` is from what's drawn of the mark, in the image's pixels.
    pub fn distance(&self, at: Point<f32>) -> f32 {
        let line = self.line_width() * 0.5;
        match &self.shape {
            Shape::Pen(points) => {
                let nearest = match points.as_slice() {
                    [] => f32::INFINITY,
                    [only] => distance(at, *only),
                    points => points
                        .windows(2)
                        .map(|pair| segment_distance(at, pair[0], pair[1]))
                        .fold(f32::INFINITY, f32::min),
                };
                (nearest - line).max(0.)
            }
            Shape::Rectangle { from, to } => {
                let corners = [*from, point(to.x, from.y), *to, point(from.x, to.y), *from];
                let nearest = corners
                    .windows(2)
                    .map(|pair| segment_distance(at, pair[0], pair[1]))
                    .fold(f32::INFINITY, f32::min);
                (nearest - line).max(0.)
            }
            Shape::Arrow { from, to } => {
                let [left, right] = arrow_head(*from, *to, self.unit);
                let nearest = segment_distance(at, *from, *to)
                    .min(segment_distance(at, left, *to))
                    .min(segment_distance(at, right, *to));
                (nearest - line).max(0.)
            }
            Shape::Text { at: origin, text } => {
                let (start, end) = self.text_box(*origin, text);
                let dx = (start.x - at.x).max(at.x - end.x).max(0.);
                let dy = (start.y - at.y).max(at.y - end.y).max(0.);
                dx.hypot(dy)
            }
            Shape::Note { at: center, .. } => (distance(at, *center) - self.pin_radius()).max(0.),
        }
    }

    fn translate(&mut self, by: Point<f32>) {
        let shift = |point: &mut Point<f32>| *point = *point + by;
        match &mut self.shape {
            Shape::Pen(points) => points.iter_mut().for_each(shift),
            Shape::Rectangle { from, to } | Shape::Arrow { from, to } => {
                shift(from);
                shift(to);
            }
            Shape::Text { at, .. } | Shape::Note { at, .. } => shift(at),
        }
    }

    /// Whether it's too small to have been meant: a click with the rectangle or the arrow, or
    /// a label or note left empty.
    fn is_empty(&self) -> bool {
        match &self.shape {
            Shape::Pen(points) => points.is_empty(),
            Shape::Rectangle { from, to } => {
                (from.x - to.x).abs().max((from.y - to.y).abs()) < SMALLEST_SHAPE * self.unit
            }
            Shape::Arrow { from, to } => distance(*from, *to) < SMALLEST_SHAPE * self.unit,
            Shape::Text { text, .. } | Shape::Note { text, .. } => text.trim().is_empty(),
        }
    }

    fn text_mut(&mut self) -> Option<&mut String> {
        match &mut self.shape {
            Shape::Text { text, .. } | Shape::Note { text, .. } => Some(text),
            _ => None,
        }
    }
}

fn distance(a: Point<f32>, b: Point<f32>) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn segment_distance(at: Point<f32>, start: Point<f32>, end: Point<f32>) -> f32 {
    let along = point(end.x - start.x, end.y - start.y);
    let length = along.x * along.x + along.y * along.y;
    if length == 0. {
        return distance(at, start);
    }
    let t = (((at.x - start.x) * along.x + (at.y - start.y) * along.y) / length).clamp(0., 1.);
    distance(at, point(start.x + along.x * t, start.y + along.y * t))
}

/// The ends of an arrow's two head strokes, which go back from its point on either side.
fn arrow_head(from: Point<f32>, to: Point<f32>, unit: f32) -> [Point<f32>; 2] {
    let length = (ARROW_HEAD * unit).min(distance(from, to) * 0.6);
    let angle = (from.y - to.y).atan2(from.x - to.x);
    let spread = 0.5_f32;
    [angle + spread, angle - spread]
        .map(|side| point(to.x + length * side.cos(), to.y + length * side.sin()))
}

/// The notes' texts, in the order their pins are numbered.
pub(crate) fn notes(marks: &[Mark]) -> Vec<String> {
    marks
        .iter()
        .filter_map(|mark| match &mark.shape {
            Shape::Note { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The notes of a message's images (each image's, in its order in the message) as the text that
/// goes after it, as t3code sends an annotation's comment with its screenshot: "1: Make this
/// full width", under the image's place in the message when more than one image has notes.
pub(crate) fn notes_text(images: &[Vec<String>]) -> Option<String> {
    let with_notes = images.iter().filter(|notes| !notes.is_empty()).count();
    if with_notes == 0 {
        return None;
    }
    let mut text = String::new();
    for (index, notes) in images.iter().enumerate() {
        if notes.is_empty() {
            continue;
        }
        text.push_str("\n\n");
        if with_notes > 1 {
            text.push_str(&format!("Image {}:\n", index + 1));
        }
        let lines: Vec<String> = notes
            .iter()
            .enumerate()
            .map(|(number, note)| format!("{}: {note}", number + 1))
            .collect();
        text.push_str(&lines.join("\n"));
    }
    Some(text)
}

fn xml_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            character => escaped.push(character),
        }
    }
    escaped
}

/// The marks as an SVG of the image's size, to draw over it.
fn marks_svg(marks: &[Mark], width: u32, height: u32) -> String {
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    );
    let mut note_number = 0;
    for mark in marks {
        let color = format!("#{:06x}", mark.color.rgb());
        let stroke = format!(
            r#"fill="none" stroke="{color}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round""#,
            mark.line_width()
        );
        match &mark.shape {
            Shape::Pen(points) => match points.as_slice() {
                [] => {}
                [only] => svg.push_str(&format!(
                    r#"<circle cx="{}" cy="{}" r="{}" fill="{color}"/>"#,
                    only.x,
                    only.y,
                    mark.line_width() * 0.5
                )),
                [first, rest @ ..] => {
                    let mut path = format!("M{} {}", first.x, first.y);
                    for point in rest {
                        path.push_str(&format!(" L{} {}", point.x, point.y));
                    }
                    svg.push_str(&format!(r#"<path d="{path}" {stroke}/>"#));
                }
            },
            Shape::Rectangle { from, to } => svg.push_str(&format!(
                r#"<rect x="{}" y="{}" width="{}" height="{}" {stroke}/>"#,
                from.x.min(to.x),
                from.y.min(to.y),
                (from.x - to.x).abs(),
                (from.y - to.y).abs()
            )),
            Shape::Arrow { from, to } => {
                let [left, right] = arrow_head(*from, *to, mark.unit);
                svg.push_str(&format!(
                    r#"<path d="M{} {} L{} {} M{} {} L{} {} L{} {}" {stroke}/>"#,
                    from.x, from.y, to.x, to.y, left.x, left.y, to.x, to.y, right.x, right.y
                ));
            }
            Shape::Text { at, text } => {
                let size = mark.text_size();
                svg.push_str(&format!(
                    r#"<text x="{}" y="{}" font-family="IBM Plex Sans, sans-serif" font-size="{size}" font-weight="600" dominant-baseline="central" fill="{color}">{}</text>"#,
                    at.x,
                    at.y + size * TEXT_LINE_HEIGHT * 0.5,
                    xml_escape(text)
                ));
            }
            Shape::Note { at, .. } => {
                note_number += 1;
                svg.push_str(&format!(
                    r#"<circle cx="{}" cy="{}" r="{}" fill="{color}"/><text x="{}" y="{}" font-family="IBM Plex Sans, sans-serif" font-size="{}" font-weight="600" text-anchor="middle" dominant-baseline="central" fill="{}">{note_number}</text>"#,
                    at.x,
                    at.y,
                    mark.pin_radius(),
                    at.x,
                    at.y,
                    PIN_TEXT_SIZE * mark.unit,
                    format_args!("#{:06x}", mark.color.number_rgb()),
                ));
            }
        }
    }
    svg.push_str("</svg>");
    svg
}

/// The image (its file's bytes) with the marks drawn in, as a PNG.
pub(crate) fn draw_marks(
    original: &[u8],
    marks: &[Mark],
    svg_renderer: &SvgRenderer,
) -> anyhow::Result<Vec<u8>> {
    let mut image = image::load_from_memory(original)
        .context("decoding the image")?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let svg = marks_svg(marks, width, height);
    let tree = svg_renderer
        .parse_svg(svg.as_bytes())
        .context("reading the marks")?;
    let drawn = svg_renderer
        .render_parsed(
            &tree,
            SvgSize::ExactSize(size(
                DevicePixels(width as i32),
                DevicePixels(height as i32),
            )),
        )
        .context("drawing the marks")?;
    let drawn_size = drawn.size(0);
    let mut pixels = drawn
        .as_bytes(0)
        .context("the drawn marks have no pixels")?
        .to_vec();
    // GPUI keeps images as BGRA.
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let marks_image = image::RgbaImage::from_raw(
        drawn_size.width.0 as u32,
        drawn_size.height.0 as u32,
        pixels,
    )
    .context("the drawn marks' size")?;
    // GPUI draws an SVG no larger than 8192 pixels a side.
    let marks_image = if marks_image.dimensions() == (width, height) {
        marks_image
    } else {
        image::imageops::resize(
            &marks_image,
            width,
            height,
            image::imageops::FilterType::Triangle,
        )
    };
    image::imageops::overlay(&mut image, &marks_image, 0, 0);
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .context("encoding the marked-up image")?;
    Ok(png)
}

/// An image marked up in a composer: the original, and the marks a copy has drawn in.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MarkedImage {
    pub original: AttachmentId,
    pub marks: Vec<Mark>,
}

/// Marked-up copies by what they're copies of, so a chip whose copy is opened again (after
/// leaving the thread too) shows the original under the marks, to change or revert, while the
/// app runs.
#[derive(Default)]
struct MarkedImages(HashMap<AttachmentImage, MarkedImage>);

impl Global for MarkedImages {}

/// What `image` is a marked-up copy of, if it's one.
pub(crate) fn marked_image(image: &AttachmentImage, cx: &App) -> Option<MarkedImage> {
    cx.try_global::<MarkedImages>()?.0.get(image).cloned()
}

pub(crate) fn remember_marked_image(copy: AttachmentImage, marked: MarkedImage, cx: &mut App) {
    cx.default_global::<MarkedImages>().0.insert(copy, marked);
}

/// What the viewer emits when markup ends: the marks Done kept, or Revert to Original.
pub(crate) enum MarkupEvent {
    Done(Vec<Mark>),
    Revert,
}

/// A press on the image while marking up, until it's released.
enum Gesture {
    /// Drawing the last mark.
    Drawing { before: Vec<Mark> },
    /// Moving a mark, from where the mouse last was. One that isn't moved was clicked.
    Moving {
        index: usize,
        last: Point<f32>,
        before: Vec<Mark>,
        moved: bool,
    },
}

/// A label or note being typed, in a field over the image.
struct Typing {
    index: usize,
    input: Entity<TextInput>,
    /// The marks before it, which Escape goes back to.
    before: Vec<Mark>,
    _changes: Subscription,
}

/// The marks on an image the viewer was opened on from the composer, and marking it up.
pub(crate) struct Markup {
    /// The marks as Done last kept them, which Cancel goes back to.
    kept: Vec<Mark>,
    marks: Vec<Mark>,
    /// Whether the toolbar is up, so a press on the image draws.
    editing: bool,
    tool: MarkupTool,
    color: MarkupColor,
    undo: Vec<Vec<Mark>>,
    redo: Vec<Vec<Mark>>,
    gesture: Option<Gesture>,
    typing: Option<Typing>,
}

impl Markup {
    pub fn new(marks: Vec<Mark>) -> Self {
        Self {
            kept: marks.clone(),
            marks,
            editing: false,
            tool: MarkupTool::Pen,
            color: MarkupColor::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
            typing: None,
        }
    }

    #[cfg(test)]
    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    pub fn is_editing(&self) -> bool {
        self.editing
    }

    pub fn is_typing(&self) -> bool {
        self.typing.is_some()
    }

    pub fn has_gesture(&self) -> bool {
        self.gesture.is_some()
    }

    /// Whether the image has marks Done kept, which Revert to Original drops.
    pub fn has_kept_marks(&self) -> bool {
        !self.kept.is_empty()
    }

    pub fn start(&mut self) {
        self.editing = true;
        self.undo.clear();
        self.redo.clear();
    }

    /// Ends markup, keeping the marks, and gives them if they changed.
    pub fn finish(&mut self, cx: &App) -> Option<Vec<Mark>> {
        self.finish_typing(cx);
        self.gesture = None;
        self.editing = false;
        if self.marks == self.kept {
            return None;
        }
        self.kept = self.marks.clone();
        Some(self.marks.clone())
    }

    /// Ends markup, dropping the marks made since it started.
    pub fn cancel(&mut self) {
        self.typing = None;
        self.gesture = None;
        self.editing = false;
        self.marks = self.kept.clone();
    }

    pub fn revert(&mut self) {
        self.cancel();
        self.marks.clear();
        self.kept.clear();
    }

    pub fn set_tool(&mut self, tool: MarkupTool, cx: &App) {
        self.finish_typing(cx);
        self.tool = tool;
    }

    /// Picks the color to draw in, which the label or note being typed takes too.
    pub fn set_color(&mut self, color: MarkupColor) {
        self.color = color;
        if let Some(mark) = self
            .typing
            .as_ref()
            .and_then(|typing| self.marks.get_mut(typing.index))
        {
            mark.color = color;
        }
    }

    pub fn undo(&mut self, cx: &App) {
        self.finish_typing(cx);
        if let Some(before) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.marks, before));
        }
    }

    pub fn redo(&mut self, cx: &App) {
        self.finish_typing(cx);
        if let Some(after) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.marks, after));
        }
    }

    /// Keeps `before` to undo to, if the marks changed from it.
    fn checkpoint(&mut self, before: Vec<Mark>) {
        if before != self.marks {
            self.undo.push(before);
            self.redo.clear();
        }
    }

    /// The topmost mark a press at `at` picks, where a point of the screen is `unit` of the
    /// image's pixels.
    fn mark_at(&self, at: Point<f32>, unit: f32) -> Option<usize> {
        self.marks
            .iter()
            .rposition(|mark| mark.distance(at) <= PICK_DISTANCE * unit)
    }

    /// A press at `at` on the image: a mark under it is picked to move, or erased with the
    /// eraser; elsewhere the tool begins a mark. A label or note begun is returned, to type.
    pub fn press(&mut self, at: Point<f32>, unit: f32, cx: &App) -> Option<(usize, Vec<Mark>)> {
        if self.typing.is_some() {
            // A press beside the field being typed in ends it, and does nothing else.
            self.finish_typing(cx);
            return None;
        }
        let before = self.marks.clone();
        if let Some(index) = self.mark_at(at, unit) {
            if self.tool == MarkupTool::Eraser {
                self.marks.remove(index);
                self.checkpoint(before);
            } else {
                self.gesture = Some(Gesture::Moving {
                    index,
                    last: at,
                    before,
                    moved: false,
                });
            }
            return None;
        }
        let shape = match self.tool {
            MarkupTool::Eraser => return None,
            MarkupTool::Pen => Shape::Pen(vec![at]),
            MarkupTool::Rectangle => Shape::Rectangle { from: at, to: at },
            MarkupTool::Arrow => Shape::Arrow { from: at, to: at },
            MarkupTool::Text => Shape::Text {
                at: point(at.x, at.y - TEXT_SIZE * TEXT_LINE_HEIGHT * 0.5 * unit),
                text: String::new(),
            },
            MarkupTool::Note => Shape::Note {
                at,
                text: String::new(),
            },
        };
        let is_typed = matches!(shape, Shape::Text { .. } | Shape::Note { .. });
        self.marks.push(Mark::new(shape, self.color, unit));
        if is_typed {
            return Some((self.marks.len() - 1, before));
        }
        self.gesture = Some(Gesture::Drawing { before });
        None
    }

    /// The mouse moved to `at` while pressed, `unit` of the image's pixels a point.
    pub fn drag(&mut self, at: Point<f32>, unit: f32) {
        match &mut self.gesture {
            Some(Gesture::Drawing { .. }) => {
                let Some(mark) = self.marks.last_mut() else {
                    return;
                };
                match &mut mark.shape {
                    Shape::Pen(points) => {
                        if points.last().is_none_or(|last| distance(*last, at) >= unit) {
                            points.push(at);
                        }
                    }
                    Shape::Rectangle { to, .. } | Shape::Arrow { to, .. } => *to = at,
                    _ => {}
                }
            }
            Some(Gesture::Moving {
                index, last, moved, ..
            }) => {
                let by = point(at.x - last.x, at.y - last.y);
                if !*moved && distance(at, *last) < unit {
                    return;
                }
                *moved = true;
                *last = at;
                if let Some(mark) = self.marks.get_mut(*index) {
                    mark.translate(by);
                }
            }
            None => {}
        }
    }

    /// The press ended. A label or note clicked without moving it is returned, to type in.
    pub fn release(&mut self) -> Option<(usize, Vec<Mark>)> {
        match self.gesture.take()? {
            Gesture::Drawing { before } => {
                if self.marks.last().is_some_and(Mark::is_empty) {
                    self.marks.pop();
                }
                self.checkpoint(before);
                None
            }
            Gesture::Moving {
                index,
                before,
                moved,
                ..
            } => {
                if moved {
                    self.checkpoint(before);
                    return None;
                }
                let is_typed = self.marks.get(index).is_some_and(|mark| {
                    matches!(mark.shape, Shape::Text { .. } | Shape::Note { .. })
                });
                is_typed.then_some((index, before))
            }
        }
    }

    /// Puts a field over a label or note to type it in.
    pub fn start_typing(
        &mut self,
        index: usize,
        before: Vec<Mark>,
        window: &mut Window,
        cx: &mut Context<ImageViewer>,
    ) {
        let Some(mark) = self.marks.get(index) else {
            return;
        };
        let (placeholder, text) = match &mark.shape {
            Shape::Text { text, .. } => ("Label", text.clone()),
            Shape::Note { text, .. } => ("Note", text.clone()),
            _ => return,
        };
        let input = cx.new(|cx| {
            let mut input = TextInput::new(placeholder, cx);
            input.set_text(text, cx);
            input
        });
        window.focus(&input.focus_handle(cx), cx);
        let changes = cx.subscribe(&input, |_, _, _: &TextInputEvent, cx| cx.notify());
        self.typing = Some(Typing {
            index,
            input,
            before,
            _changes: changes,
        });
    }

    /// Keeps what was typed; a label or note left empty goes.
    pub fn finish_typing(&mut self, cx: &App) {
        let Some(typing) = self.typing.take() else {
            return;
        };
        let typed = typing.input.read(cx).text().trim().to_string();
        let Some(mark) = self.marks.get_mut(typing.index) else {
            return;
        };
        if let Some(text) = mark.text_mut() {
            *text = typed;
        }
        if mark.is_empty() {
            self.marks.remove(typing.index);
        }
        self.checkpoint(typing.before);
    }

    /// Drops what was typed, and a label or note begun with it.
    pub fn cancel_typing(&mut self) {
        if let Some(typing) = self.typing.take() {
            self.marks = typing.before;
        }
    }

    /// The marks drawn over the image, `scale` points of the screen to its pixel, in an
    /// element the image's size.
    pub fn render_marks(&self, scale: f32) -> Div {
        let typed_label = self.typing.as_ref().and_then(|typing| {
            self.marks
                .get(typing.index)
                .filter(|mark| matches!(mark.shape, Shape::Text { .. }))
                .map(|_| typing.index)
        });
        let screen = move |at: Point<f32>| point(px(at.x * scale), px(at.y * scale));
        // Lines: each mark's strokes, in the window once the canvas knows where it is.
        let mut lines: Vec<(Vec<Vec<Point<Pixels>>>, Pixels, Hsla)> = Vec::new();
        let mut labels = Vec::new();
        let mut note_number = 0;
        for (index, mark) in self.marks.iter().enumerate() {
            let color = mark.color.hsla();
            let width = px(mark.line_width() * scale);
            match &mark.shape {
                Shape::Pen(points) if points.len() == 1 => {
                    let center = screen(points[0]);
                    labels.push(
                        div()
                            .absolute()
                            .left(center.x - width * 0.5)
                            .top(center.y - width * 0.5)
                            .size(width)
                            .rounded_full()
                            .bg(color)
                            .into_any_element(),
                    );
                }
                Shape::Pen(points) => lines.push((
                    vec![points.iter().copied().map(screen).collect()],
                    width,
                    color,
                )),
                Shape::Rectangle { from, to } => {
                    let corners = [*from, point(to.x, from.y), *to, point(from.x, to.y), *from];
                    lines.push((vec![corners.map(screen).to_vec()], width, color));
                }
                Shape::Arrow { from, to } => {
                    let [left, right] = arrow_head(*from, *to, mark.unit);
                    lines.push((
                        vec![
                            vec![screen(*from), screen(*to)],
                            vec![screen(left), screen(*to), screen(right)],
                        ],
                        width,
                        color,
                    ));
                }
                Shape::Text { at, text } => {
                    if typed_label == Some(index) {
                        continue;
                    }
                    let size = mark.text_size() * scale;
                    let at = screen(*at);
                    labels.push(
                        div()
                            .absolute()
                            .left(at.x)
                            .top(at.y)
                            .whitespace_nowrap()
                            .text_size(px(size))
                            .line_height(px(size * TEXT_LINE_HEIGHT))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(color)
                            .child(text.clone())
                            .into_any_element(),
                    );
                }
                Shape::Note { at, .. } => {
                    note_number += 1;
                    let radius = px(mark.pin_radius() * scale);
                    let center = screen(*at);
                    labels.push(
                        render_pin(
                            note_number,
                            mark.color,
                            radius * 2.,
                            PIN_TEXT_SIZE * mark.unit * scale,
                        )
                        .absolute()
                        .left(center.x - radius)
                        .top(center.y - radius)
                        .debug_selector(move || format!("image-markup-pin-{note_number}"))
                        .into_any_element(),
                    );
                }
            }
        }
        div()
            .debug_selector(|| "image-markup-marks".into())
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        for (strokes, width, color) in lines {
                            let options = StrokeOptions::default()
                                .with_line_width(f32::from(width))
                                .with_line_cap(LineCap::Round)
                                .with_line_join(LineJoin::Round);
                            let mut path =
                                PathBuilder::stroke(width).with_style(PathStyle::Stroke(options));
                            for stroke in strokes {
                                let mut points = stroke.into_iter();
                                let Some(first) = points.next() else {
                                    continue;
                                };
                                path.move_to(bounds.origin + first);
                                for point in points {
                                    path.line_to(bounds.origin + point);
                                }
                            }
                            match path.build() {
                                Ok(path) => window.paint_path(path, color),
                                Err(error) => log::error!("failed to draw a mark: {error}"),
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(labels)
    }

    /// The field a label or note is typed in, over the image whose top left is at `origin` in
    /// the element it's drawn in, `scale` points of the screen to its pixel.
    pub fn render_typing(
        &self,
        origin: Point<Pixels>,
        scale: f32,
        cx: &Context<ImageViewer>,
    ) -> Option<AnyElement> {
        let typing = self.typing.as_ref()?;
        let mark = self.marks.get(typing.index)?;
        let colors = cx.theme().colors();
        let field = div()
            .key_context(MARKUP_TEXT_KEY_CONTEXT)
            .debug_selector(|| "image-markup-typing".into())
            .on_action(cx.listener(|viewer, _: &menu::Confirm, window, cx| {
                viewer.end_typing(true, window, cx);
            }))
            .on_action(cx.listener(|viewer, _: &menu::Cancel, window, cx| {
                viewer.end_typing(false, window, cx);
            }))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
        match &mark.shape {
            Shape::Text { at, .. } => {
                let size = mark.text_size() * scale;
                Some(
                    field
                        .absolute()
                        .left(origin.x + px(at.x * scale) - px(5.))
                        .top(origin.y + px(at.y * scale) - px(3.))
                        .min_w(px(160.))
                        .px(px(4.))
                        .py(px(2.))
                        .rounded_sm()
                        .border_1()
                        .border_color(mark.color.hsla().opacity(0.7))
                        .bg(gpui::black().opacity(0.25))
                        .text_size(px(size))
                        .line_height(px(size * TEXT_LINE_HEIGHT))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(mark.color.hsla())
                        .child(typing.input.clone())
                        .into_any_element(),
                )
            }
            Shape::Note { at, .. } => {
                let number = self.marks[..=typing.index]
                    .iter()
                    .filter(|mark| matches!(mark.shape, Shape::Note { .. }))
                    .count();
                let pin = px(mark.pin_radius() * scale);
                Some(
                    field
                        .absolute()
                        .left(origin.x + px(at.x * scale) + pin + px(6.))
                        .top(origin.y + px(at.y * scale) - px(12.))
                        .w(px(240.))
                        .p_2()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .rounded_md()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.elevated_surface_background)
                        .shadow_md()
                        .child(
                            h_flex()
                                .gap_1()
                                .child(render_pin(number, mark.color, px(16.), 10.))
                                .child(
                                    Label::new("Note")
                                        .size(LabelSize::Small)
                                        .color(Color::Muted),
                                ),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .border_1()
                                .border_color(colors.border)
                                .bg(colors.editor_background)
                                .text_sm()
                                .text_color(colors.text)
                                .child(typing.input.clone()),
                        )
                        .into_any_element(),
                )
            }
            _ => None,
        }
    }

    /// The tools, the colors, Undo, Redo and the eraser, then Cancel and Done.
    pub fn render_toolbar(&self, cx: &Context<ImageViewer>) -> Div {
        let colors = cx.theme().colors();
        let separator = || div().mx(px(3.)).w(px(1.)).h(px(16.)).bg(colors.border);
        let tool_button = |tool: MarkupTool, cx: &Context<ImageViewer>| {
            IconButton::new(tool.id(), tool.icon())
                .icon_size(IconSize::Small)
                .toggle_state(self.tool == tool)
                .tooltip(Tooltip::text(tool.label()))
                .on_click(cx.listener(move |viewer, _, window, cx| {
                    viewer.set_markup_tool(tool, window, cx);
                }))
        };
        let color_buttons = MarkupColor::ALL.map(|color| {
            let is_picked = self.color == color;
            div()
                .id(color.id())
                .debug_selector(move || color.id().into())
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .border_1()
                .border_color(if is_picked {
                    colors.text
                } else {
                    gpui::transparent_black()
                })
                .cursor_pointer()
                .child(div().size(px(14.)).rounded_full().bg(color.hsla()))
                .tooltip(Tooltip::text(color.label()))
                .on_click(cx.listener(move |viewer, _, _, cx| {
                    viewer.set_markup_color(color, cx);
                }))
        });
        h_flex()
            .debug_selector(|| "image-markup-toolbar".into())
            .gap_0p5()
            .p_1()
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .children(MarkupTool::DRAWING.map(|tool| tool_button(tool, cx)))
            .child(separator())
            .child(h_flex().gap_0p5().px_1().children(color_buttons))
            .child(separator())
            .child(
                IconButton::new("image-markup-undo", IconName::Undo)
                    .icon_size(IconSize::Small)
                    .disabled(self.undo.is_empty())
                    .tooltip(Tooltip::text("Undo"))
                    .on_click(cx.listener(|viewer, _, window, cx| viewer.undo_mark(window, cx))),
            )
            .child(
                IconButton::new("image-markup-redo", IconName::Redo)
                    .icon_size(IconSize::Small)
                    .disabled(self.redo.is_empty())
                    .tooltip(Tooltip::text("Redo"))
                    .on_click(cx.listener(|viewer, _, window, cx| viewer.redo_mark(window, cx))),
            )
            .child(tool_button(MarkupTool::Eraser, cx))
            .child(separator())
            .child(
                Button::new("image-markup-cancel", "Cancel")
                    .label_size(LabelSize::Small)
                    .on_click(cx.listener(|viewer, _, window, cx| {
                        viewer.cancel_markup(window, cx);
                    })),
            )
            .child(
                Button::new("image-markup-done", "Done")
                    .label_size(LabelSize::Small)
                    .style(ButtonStyle::Filled)
                    .on_click(cx.listener(|viewer, _, window, cx| {
                        viewer.finish_markup(window, cx);
                    })),
            )
    }
}

/// A numbered note's pin.
fn render_pin(number: usize, color: MarkupColor, diameter: Pixels, text_size: f32) -> Div {
    div()
        .size(diameter)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(color.hsla())
        .text_size(px(text_size))
        .line_height(px(text_size))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(gpui::rgb(color.number_rgb()))
        .child(number.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn rectangle(from: (f32, f32), to: (f32, f32)) -> Mark {
        Mark::new(
            Shape::Rectangle {
                from: point(from.0, from.1),
                to: point(to.0, to.1),
            },
            MarkupColor::Red,
            1.,
        )
    }

    #[test]
    fn notes_go_after_the_message_numbered_as_their_pins() {
        assert_eq!(notes_text(&[]), None);
        assert_eq!(notes_text(&[Vec::new()]), None);
        assert_eq!(
            notes_text(&[vec![
                "Make this full width".into(),
                "Remove the shipping line".into()
            ]])
            .as_deref(),
            Some("\n\n1: Make this full width\n2: Remove the shipping line")
        );
        assert_eq!(
            notes_text(&[
                vec!["Bigger".into()],
                Vec::new(),
                vec!["Blue".into(), "Round".into()]
            ])
            .as_deref(),
            Some("\n\nImage 1:\n1: Bigger\n\nImage 3:\n1: Blue\n2: Round")
        );
    }

    #[test]
    fn a_press_picks_the_mark_drawn_there() {
        let mark = rectangle((10., 10.), (110., 60.));
        assert_eq!(mark.distance(point(60., 10.)), 0.);
        assert_eq!(mark.distance(point(110., 40.)), 0.);
        assert!(mark.distance(point(60., 35.)) > 20., "not inside it");
        let arrow = Mark::new(
            Shape::Arrow {
                from: point(0., 0.),
                to: point(100., 0.),
            },
            MarkupColor::Red,
            1.,
        );
        assert!(arrow.distance(point(50., 4.)) < 3.);
        let note = Mark::new(
            Shape::Note {
                at: point(50., 50.),
                text: "Here".into(),
            },
            MarkupColor::Red,
            1.,
        );
        assert_eq!(note.distance(point(55., 55.)), 0.);
        assert!(note.distance(point(70., 50.)) > 9.);
    }

    /// Moving, erasing and undoing, as the viewer's presses do them.
    #[gpui::test]
    fn marks_move_erase_and_undo(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let mut markup = Markup::new(Vec::new());
            markup.start();
            markup.set_tool(MarkupTool::Rectangle, cx);
            assert!(markup.press(point(10., 10.), 1., cx).is_none());
            markup.drag(point(110., 60.), 1.);
            assert!(markup.release().is_none());
            assert_eq!(markup.marks(), [rectangle((10., 10.), (110., 60.))]);

            // A click with the rectangle draws nothing.
            markup.press(point(300., 300.), 1., cx);
            markup.release();
            assert_eq!(markup.marks().len(), 1);

            markup.press(point(60., 10.), 1., cx);
            markup.drag(point(70., 30.), 1.);
            markup.release();
            assert_eq!(markup.marks(), [rectangle((20., 30.), (120., 80.))]);

            markup.set_tool(MarkupTool::Eraser, cx);
            markup.press(point(20., 50.), 1., cx);
            assert!(markup.marks().is_empty());

            markup.undo(cx);
            assert_eq!(markup.marks(), [rectangle((20., 30.), (120., 80.))]);
            markup.undo(cx);
            assert_eq!(markup.marks(), [rectangle((10., 10.), (110., 60.))]);
            markup.redo(cx);
            assert_eq!(markup.marks(), [rectangle((20., 30.), (120., 80.))]);

            assert_eq!(
                markup.finish(cx),
                Some(vec![rectangle((20., 30.), (120., 80.))])
            );
            assert_eq!(markup.finish(cx), None, "nothing changed since");
            markup.start();
            markup.set_tool(MarkupTool::Pen, cx);
            markup.press(point(400., 400.), 1., cx);
            markup.drag(point(420., 410.), 1.);
            markup.release();
            assert_eq!(markup.marks().len(), 2);
            markup.cancel();
            assert_eq!(markup.marks(), [rectangle((20., 30.), (120., 80.))]);
            assert!(markup.has_kept_marks());
            markup.revert();
            assert!(markup.marks().is_empty() && !markup.has_kept_marks());
        });
    }

    /// The marks are drawn over the image's own pixels, the rest of it as it was.
    #[test]
    fn marks_are_drawn_into_a_copy() {
        let mut original = Vec::new();
        image::RgbaImage::from_pixel(40, 30, image::Rgba([255, 255, 255, 255]))
            .write_to(&mut Cursor::new(&mut original), image::ImageFormat::Png)
            .expect("encodes");
        let renderer = SvgRenderer::new(Arc::new(()));
        let png =
            draw_marks(&original, &[rectangle((5., 5.), (35., 25.))], &renderer).expect("draws");
        let drawn = image::load_from_memory(&png).expect("a PNG").to_rgba8();
        assert_eq!(drawn.dimensions(), (40, 30));
        let edge = drawn.get_pixel(20, 5);
        assert!(edge[0] > 200 && edge[1] < 120 && edge[2] < 120, "{edge:?}");
        assert_eq!(drawn.get_pixel(20, 15), &image::Rgba([255, 255, 255, 255]));
        assert_eq!(drawn.get_pixel(1, 1), &image::Rgba([255, 255, 255, 255]));
    }
}
