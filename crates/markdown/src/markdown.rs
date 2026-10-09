pub mod html;
mod language;
mod mermaid;
pub mod parser;
mod path_range;
mod selection;

use crate::language::LanguageName;
use base64::Engine as _;
use gpui::EdgesRefinement;
use gpui::HitboxBehavior;
use gpui::UnderlineStyle;

use log::Level;
use mermaid::{
    MermaidState, ParsedMarkdownMermaidDiagram, extract_mermaid_diagrams, render_mermaid_diagram,
};
pub use path_range::{LineCol, PathWithRange};
use smallvec::SmallVec;
use util::maybe;

use std::borrow::Cow;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::mem;
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use crate::language::{
    Bias, CharClassifier, Language, LanguageRegistry, OffsetUtf16, ResolvedHighlights,
};
use collections::{HashMap, HashSet};
use gpui::{
    AnyElement, App, BorderStyle, Bounds, ClipboardItem, CursorStyle, DispatchPhase, Edges, Entity,
    FocusHandle, Focusable, FontStyle, FontWeight, GlobalElementId, Hitbox, Hsla, Image,
    ImageFormat, ImageSource, InputHandler, KeyContext, Length, MouseButton, MouseDownEvent,
    MouseEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollHandle, Stateful,
    StrikethroughStyle, StyleRefinement, StyledImage, StyledText, Subscription, Task, TextAlign,
    TextLayout, TextRun, TextStyle, TextStyleRefinement, UTF16Selection, WrappedLineLayout,
    actions, canvas, img, point, quad, relative, size,
};
use parser::CodeBlockMetadata;
use parser::{
    MarkdownEvent, MarkdownTag, MarkdownTagEnd, ParsedMetadataBlock, parse_links_only,
    parse_markdown_with_options,
};
use pulldown_cmark::{Alignment, BlockQuoteKind};
use rope::Rope;
use sum_tree::TreeMap;
use theme::SyntaxTheme;
use ui::{Checkbox, CopyButton, ScrollAxes, Scrollbars, Tooltip, WithScrollbar, prelude::*};
use util::ResultExt;

use crate::parser::CodeBlockKind;

const MERMAID_MAX_ZOOM: f32 = 2.0;
/// Zoom levels within this distance of 1.0 snap back to exactly 1.0 so users
/// can easily return to the default size.
const MERMAID_ZOOM_SNAP_TOLERANCE: f32 = 0.05;
const MERMAID_ZOOM_DEBOUNCE: Duration = Duration::from_millis(300);

/// A callback function that can be used to customize the style of links based on the destination URL.
/// If the callback returns `None`, the default link style will be used.
type LinkStyleCallback = Rc<dyn Fn(&str, &App) -> Option<TextStyleRefinement>>;
pub type CodeSpanLinkCallback = Arc<dyn Fn(&str, &App) -> Option<SharedString> + 'static>;
type UrlHoverCallback = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App)>;
type SourceClickCallback = Box<dyn Fn(usize, usize, &mut Window, &mut App) -> bool>;
type CheckboxToggleCallback = Rc<dyn Fn(Range<usize>, bool, &mut Window, &mut App)>;
/// Invoked when a mermaid diagram's zoom level changes (via scroll gesture or
/// the reset button), so a scroll container can keep the diagram anchored.
pub type MermaidZoomCallback = Rc<dyn Fn(&mut Window, &mut App)>;

#[derive(Clone, Copy, Default)]
pub struct BlockQuoteKindColors {
    pub note: Hsla,
    pub tip: Hsla,
    pub important: Hsla,
    pub warning: Hsla,
    pub caution: Hsla,
}

impl BlockQuoteKindColors {
    fn for_kind(&self, kind: Option<BlockQuoteKind>, default: Hsla) -> Hsla {
        match kind {
            Some(BlockQuoteKind::Note) => self.note,
            Some(BlockQuoteKind::Tip) => self.tip,
            Some(BlockQuoteKind::Important) => self.important,
            Some(BlockQuoteKind::Warning) => self.warning,
            Some(BlockQuoteKind::Caution) => self.caution,
            None => default,
        }
    }
}

#[derive(Clone, Default)]
pub struct HeadingLevelStyles {
    pub h1: Option<TextStyleRefinement>,
    pub h2: Option<TextStyleRefinement>,
    pub h3: Option<TextStyleRefinement>,
    pub h4: Option<TextStyleRefinement>,
    pub h5: Option<TextStyleRefinement>,
    pub h6: Option<TextStyleRefinement>,
}

#[derive(Clone)]
pub struct MarkdownStyle {
    pub base_text_style: TextStyle,
    pub container_style: StyleRefinement,
    pub code_block: StyleRefinement,
    pub code_block_overflow_x_scroll: bool,
    pub inline_code: TextStyleRefinement,
    pub block_quote: TextStyleRefinement,
    pub link: TextStyleRefinement,
    pub link_callback: Option<LinkStyleCallback>,
    pub rule_color: Hsla,
    pub block_quote_border_color: Hsla,
    pub block_quote_kind_colors: BlockQuoteKindColors,
    pub syntax: Arc<SyntaxTheme>,
    pub selection_background_color: Hsla,
    pub heading: StyleRefinement,
    pub heading_level_styles: Option<HeadingLevelStyles>,
    pub heading_border_color: Option<Hsla>,
    pub paragraph_spacing: Pixels,
    pub paragraph_line_height: DefiniteLength,
    /// Bottom margin of top-level lists only
    pub list_spacing: Pixels,
    /// Horizontal (`x`) and vertical (`y`) padding of table cells
    pub table_cell_padding: Point<Pixels>,
    pub height_is_multiple_of_line_height: bool,
    pub prevent_mouse_interaction: bool,
    pub table_columns_min_size: bool,
    pub soft_break_as_hard_break: bool,
}

impl Default for MarkdownStyle {
    fn default() -> Self {
        Self {
            base_text_style: Default::default(),
            container_style: Default::default(),
            code_block: Default::default(),
            code_block_overflow_x_scroll: false,
            inline_code: Default::default(),
            block_quote: Default::default(),
            link: Default::default(),
            link_callback: None,
            rule_color: Default::default(),
            block_quote_border_color: Default::default(),
            block_quote_kind_colors: Default::default(),
            syntax: Arc::new(SyntaxTheme::default()),
            selection_background_color: Default::default(),
            heading: Default::default(),
            heading_level_styles: None,
            heading_border_color: None,
            paragraph_spacing: px(8.),
            paragraph_line_height: rems(1.3).into(),
            list_spacing: px(0.),
            table_cell_padding: point(px(4.), px(2.)),
            height_is_multiple_of_line_height: false,
            prevent_mouse_interaction: false,
            table_columns_min_size: false,
            soft_break_as_hard_break: false,
        }
    }
}

#[derive(Clone, Copy)]
pub enum MarkdownFont {
    Agent,
    Editor,
    Preview,
}

impl MarkdownStyle {
    pub fn themed(font: MarkdownFont, window: &Window, cx: &App) -> Self {
        let colors = cx.theme().colors();
        let syntax = cx.theme().syntax().clone();
        Self::themed_with_overrides(font, colors, &syntax, window, cx)
    }

    /// Like [`Self::themed`], but takes explicit [`ThemeColors`] and
    /// [`SyntaxTheme`] so callers (e.g. the markdown preview) can render the
    /// markdown using a theme other than the active editor theme.
    pub fn themed_with_overrides(
        font: MarkdownFont,
        colors: &theme::ThemeColors,
        syntax: &Arc<SyntaxTheme>,
        window: &Window,
        cx: &App,
    ) -> Self {
        let settings = theme::theme_settings(cx);
        let ui_font = settings.ui_font(cx).clone();
        let buffer_font = settings.buffer_font(cx).clone();
        let is_preview = matches!(font, MarkdownFont::Preview);

        let buffer_font_weight = buffer_font.weight;
        let (buffer_font_size, ui_font_size) = match font {
            MarkdownFont::Agent => (settings.buffer_font_size(cx), settings.ui_font_size(cx)),
            MarkdownFont::Editor => (settings.buffer_font_size(cx), settings.ui_font_size(cx)),
            MarkdownFont::Preview => (settings.buffer_font_size(cx), settings.ui_font_size(cx)),
        };

        let body_font_family = match font {
            MarkdownFont::Preview => ui_font.family.clone(),
            MarkdownFont::Agent => ui_font.family.clone(),
            MarkdownFont::Editor => ui_font.family.clone(),
        };
        let code_font_family = match font {
            MarkdownFont::Preview => buffer_font.family.clone(),
            MarkdownFont::Agent => buffer_font.family.clone(),
            MarkdownFont::Editor => buffer_font.family.clone(),
        };

        let mut text_style = window.text_style();
        let line_height = buffer_font_size * 1.75;

        text_style.refine(&TextStyleRefinement {
            font_family: Some(body_font_family),
            font_fallbacks: ui_font.fallbacks.clone(),
            font_features: Some(ui_font.features),
            font_size: Some(if is_preview {
                rems(1.0).into()
            } else {
                ui_font_size.into()
            }),
            line_height: Some(line_height.into()),
            color: Some(colors.text),
            ..Default::default()
        });

        let style = MarkdownStyle {
            base_text_style: text_style.clone(),
            syntax: syntax.clone(),
            selection_background_color: colors.element_selection_background,
            rule_color: colors.border,
            block_quote_border_color: colors.border,
            block_quote_kind_colors: {
                let status = cx.theme().status();
                BlockQuoteKindColors {
                    note: status.info,
                    tip: status.success,
                    important: status.info,
                    warning: status.warning,
                    caution: status.error,
                }
            },
            code_block_overflow_x_scroll: true,
            code_block: StyleRefinement {
                padding: EdgesRefinement {
                    top: Some(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(8.)))),
                    left: Some(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(8.)))),
                    right: Some(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(8.)))),
                    bottom: Some(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(8.)))),
                },
                margin: EdgesRefinement {
                    top: Some(Length::Definite(px(8.).into())),
                    left: Some(Length::Definite(px(0.).into())),
                    right: Some(Length::Definite(px(0.).into())),
                    bottom: Some(Length::Definite(px(12.).into())),
                },
                border_style: Some(BorderStyle::Solid),
                border_widths: EdgesRefinement {
                    top: Some(AbsoluteLength::Pixels(px(1.))),
                    left: Some(AbsoluteLength::Pixels(px(1.))),
                    right: Some(AbsoluteLength::Pixels(px(1.))),
                    bottom: Some(AbsoluteLength::Pixels(px(1.))),
                },
                border_color: Some(colors.border_variant),
                background: Some(colors.editor_background.into()),
                text: TextStyleRefinement {
                    font_family: Some(code_font_family.clone()),
                    font_fallbacks: buffer_font.fallbacks.clone(),
                    font_features: Some(buffer_font.features.clone()),
                    font_size: Some(buffer_font_size.into()),
                    font_weight: Some(buffer_font_weight),
                    line_height: Some(relative(1.618)),
                    ..Default::default()
                },
                ..Default::default()
            },
            inline_code: TextStyleRefinement {
                font_family: Some(code_font_family),
                font_fallbacks: buffer_font.fallbacks.clone(),
                font_features: Some(buffer_font.features),
                font_size: Some(buffer_font_size.into()),
                font_weight: Some(buffer_font_weight),
                background_color: Some(colors.editor_foreground.opacity(0.08)),
                ..Default::default()
            },
            link: TextStyleRefinement {
                background_color: Some(colors.editor_foreground.opacity(0.025)),
                color: Some(colors.text_accent),
                underline: Some(UnderlineStyle {
                    color: Some(colors.text_accent.opacity(0.5)),
                    thickness: px(1.),
                    ..Default::default()
                }),
                ..Default::default()
            },
            soft_break_as_hard_break: matches!(font, MarkdownFont::Agent),
            heading_level_styles: matches!(font, MarkdownFont::Agent).then_some(
                HeadingLevelStyles {
                    h1: Some(TextStyleRefinement {
                        font_size: Some(rems(1.15).into()),
                        ..Default::default()
                    }),
                    h2: Some(TextStyleRefinement {
                        font_size: Some(rems(1.1).into()),
                        ..Default::default()
                    }),
                    h3: Some(TextStyleRefinement {
                        font_size: Some(rems(1.05).into()),
                        ..Default::default()
                    }),
                    h4: Some(TextStyleRefinement {
                        font_size: Some(rems(1.).into()),
                        ..Default::default()
                    }),
                    h5: Some(TextStyleRefinement {
                        font_size: Some(rems(0.95).into()),
                        ..Default::default()
                    }),
                    h6: Some(TextStyleRefinement {
                        font_size: Some(rems(0.875).into()),
                        ..Default::default()
                    }),
                },
            ),
            ..Default::default()
        };

        if is_preview {
            style.with_preview_overrides(colors)
        } else {
            style
        }
    }

    fn with_preview_overrides(mut self, colors: &theme::ThemeColors) -> Self {
        let body_font_size = rems(1.0);
        self.base_text_style.font_size = body_font_size.into();
        self.container_style.text.font_size = Some(body_font_size.into());

        self.base_text_style.color = colors.text;
        self.base_text_style.line_height = relative(1.5);
        self.paragraph_spacing = px(16.);
        self.paragraph_line_height = relative(1.5);
        self.list_spacing = px(12.);
        self.table_cell_padding = point(px(10.), px(4.));

        self.inline_code.color = Some(colors.text);
        self.inline_code.font_size = Some(rems(0.875).into());

        self.link.background_color = None;

        self.block_quote.color = Some(colors.text_muted);

        self.code_block.padding = EdgesRefinement {
            top: Some(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(12.)))),
            left: Some(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(12.)))),
            right: Some(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(12.)))),
            bottom: Some(DefiniteLength::Absolute(AbsoluteLength::Pixels(px(12.)))),
        };
        self.code_block.margin.top = Some(Length::Definite(px(16.).into()));
        self.code_block.margin.bottom = Some(Length::Definite(px(16.).into()));
        let code_block_corner_radius = AbsoluteLength::Pixels(px(6.));
        self.code_block.corner_radii.top_left = Some(code_block_corner_radius);
        self.code_block.corner_radii.top_right = Some(code_block_corner_radius);
        self.code_block.corner_radii.bottom_left = Some(code_block_corner_radius);
        self.code_block.corner_radii.bottom_right = Some(code_block_corner_radius);

        self.heading.text.color = Some(colors.text);
        self.heading.margin.top = Some(Length::Definite(px(24.).into()));
        self.heading.margin.bottom = Some(Length::Definite(px(12.).into()));

        let heading_text_style = |font_size: Rems| TextStyleRefinement {
            font_size: Some(font_size.into()),
            font_weight: Some(FontWeight::SEMIBOLD),
            line_height: Some(relative(1.25)),
            ..Default::default()
        };
        self.heading_level_styles = Some(HeadingLevelStyles {
            h1: Some(heading_text_style(rems(1.75))),
            h2: Some(heading_text_style(rems(1.4))),
            h3: Some(heading_text_style(rems(1.2))),
            h4: Some(heading_text_style(rems(1.0))),
            h5: Some(heading_text_style(rems(0.875))),
            h6: Some(TextStyleRefinement {
                color: Some(colors.text_muted),
                ..heading_text_style(rems(0.85))
            }),
        });

        self.heading_border_color = Some(colors.border_variant);

        self
    }

    pub fn with_buffer_font(mut self, cx: &App) -> Self {
        let buffer_font = theme::theme_settings(cx).buffer_font(cx).clone();
        self.base_text_style.font_family = buffer_font.family.clone();
        self.base_text_style.font_fallbacks = buffer_font.fallbacks.clone();
        self.base_text_style.font_features = buffer_font.features.clone();
        self.base_text_style.font_weight = buffer_font.weight;
        self
    }

    pub fn with_agent_buffer_font(mut self, cx: &App) -> Self {
        let buffer_font = theme::theme_settings(cx).buffer_font(cx).clone();
        self.base_text_style.font_family = buffer_font.family.clone();
        self.base_text_style.font_fallbacks = buffer_font.fallbacks.clone();
        self.base_text_style.font_features = buffer_font.features.clone();
        self.base_text_style.font_weight = buffer_font.weight;
        self
    }

    pub fn with_muted_text(mut self, cx: &App) -> Self {
        let colors = cx.theme().colors();
        self.base_text_style.color = colors.text_muted;
        self
    }
}

/// Per-diagram view state, keyed by source offset in [`Markdown::mermaid_views`].
struct MermaidViewState {
    /// Whether the source code is shown instead of the rendered diagram.
    showing_code: bool,
    /// The display scale relative to the diagram's natural size; 1.0 is 1:1.
    zoom: f32,
    /// Whether the user zoomed out to the fit-to-width floor. While set, the
    /// zoom tracks the container width so the diagram stays fully visible
    /// when the container is resized, instead of keeping a stale absolute
    /// zoom computed against the old width.
    zoomed_to_fit: bool,
    /// Horizontal scroll position, used when the diagram overflows.
    scroll_handle: ScrollHandle,
    /// The pending debounced re-raster scheduled by the last zoom change.
    debounce_task: Option<Task<()>>,
    /// Overrides the scroll container width, which tests can't obtain from
    /// the scroll handle since its bounds are only set during layout.
    #[cfg(test)]
    container_width_for_test: Option<Pixels>,
}

impl MermaidViewState {
    /// The width of the diagram's scroll container as of the last layout,
    /// if it has been laid out.
    fn container_width(&self) -> Option<Pixels> {
        #[cfg(test)]
        if let Some(width) = self.container_width_for_test {
            return Some(width);
        }
        Some(self.scroll_handle.bounds().size.width).filter(|width| *width > px(0.))
    }
}

impl Default for MermaidViewState {
    fn default() -> Self {
        Self {
            showing_code: false,
            zoom: 1.0,
            zoomed_to_fit: false,
            scroll_handle: ScrollHandle::new(),
            debounce_task: None,
            #[cfg(test)]
            container_width_for_test: None,
        }
    }
}

pub struct Markdown {
    source: SharedString,
    selection: Selection,
    pressed_link: Option<RenderedLink>,
    pressed_footnote_ref: Option<RenderedFootnoteRef>,
    autoscroll_request: Option<usize>,
    pending_heading_scroll: Option<SharedString>,
    pending_autoscroll: Option<usize>,
    active_root_block: Option<usize>,
    parsed_markdown: ParsedMarkdown,
    images_by_source_offset: HashMap<usize, Arc<Image>>,
    should_reparse: bool,
    pending_parse: Option<Task<()>>,
    focus_handle: FocusHandle,
    language_registry: Option<Arc<LanguageRegistry>>,
    fallback_code_block_language: Option<LanguageName>,
    options: MarkdownOptions,
    mermaid_state: MermaidState,
    _mermaid_theme_subscription: Option<Subscription>,
    /// Per-diagram view state (current tab, zoom, scroll position, and pending
    /// debounced re-raster) keyed by source offset. Distinct from
    /// [`MermaidState`], which caches the rendered diagrams themselves keyed by
    /// contents. All entries are retained against the parsed diagrams on each
    /// reparse, so a single map keeps that bookkeeping in one place.
    mermaid_views: HashMap<usize, MermaidViewState>,
    copied_code_blocks: HashSet<ElementId>,
    wrapped_code_blocks: HashSet<usize>,
    code_block_scroll_handles: BTreeMap<usize, ScrollHandle>,
    context_menu_link: Option<SharedString>,
    context_menu_selected_text: Option<SharedString>,
    context_menu_selected_markdown: Option<SharedString>,
    search_highlights: Rc<[Range<usize>]>,
    active_search_highlight: Option<usize>,
}

#[derive(Clone, Copy, Default)]
pub struct MarkdownOptions {
    pub parse_links_only: bool,
    pub parse_html: bool,
    pub render_mermaid_diagrams: bool,
    pub parse_heading_slugs: bool,
    pub render_metadata_blocks: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CopyButtonVisibility {
    Hidden,
    AlwaysVisible,
    VisibleOnHover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapButtonVisibility {
    Hidden,
    AlwaysVisible,
    VisibleOnHover,
}

pub enum CodeBlockRenderer {
    Default {
        copy_button_visibility: CopyButtonVisibility,
        wrap_button_visibility: WrapButtonVisibility,
        border: bool,
    },
    Custom {
        render: CodeBlockRenderFn,
        /// A function that can modify the parent container after the code block
        /// content has been appended as a child element.
        transform: Option<CodeBlockTransformFn>,
    },
}

pub type CodeBlockRenderFn = Arc<
    dyn Fn(
        &CodeBlockKind,
        &ParsedMarkdown,
        Range<usize>,
        CodeBlockMetadata,
        &mut Window,
        &App,
    ) -> Div,
>;

pub type CodeBlockTransformFn =
    Arc<dyn Fn(AnyDiv, Range<usize>, CodeBlockMetadata, &mut Window, &App) -> AnyDiv>;

actions!(
    markdown,
    [
        /// Copies the selected text to the clipboard.
        Copy,
        /// Copies the selected text as markdown to the clipboard.
        CopyAsMarkdown
    ]
);

enum EscapeAction {
    PassThrough,
    Nbsp(usize),
    DoubleNewline,
    PrefixBackslash,
}

impl EscapeAction {
    fn output_len(&self, c: char) -> usize {
        match self {
            Self::PassThrough => c.len_utf8(),
            Self::Nbsp(count) => count * '\u{00A0}'.len_utf8(),
            Self::DoubleNewline => 2,
            Self::PrefixBackslash => '\\'.len_utf8() + c.len_utf8(),
        }
    }

    fn write_to(&self, c: char, output: &mut String) {
        match self {
            Self::PassThrough => output.push(c),
            Self::Nbsp(count) => {
                for _ in 0..*count {
                    output.push('\u{00A0}');
                }
            }
            Self::DoubleNewline => {
                output.push('\n');
                output.push('\n');
            }
            Self::PrefixBackslash => {
                // '\\' is a single backslash in Rust, e.g. '|' -> '\|'
                output.push('\\');
                output.push(c);
            }
        }
    }
}

struct MarkdownEscaper {
    in_leading_whitespace: bool,
}

impl MarkdownEscaper {
    const TAB_SIZE: usize = 4;

    fn new() -> Self {
        Self {
            in_leading_whitespace: true,
        }
    }

    fn next(&mut self, c: char) -> EscapeAction {
        let action = if self.in_leading_whitespace && c == '\t' {
            EscapeAction::Nbsp(Self::TAB_SIZE)
        } else if self.in_leading_whitespace && c == ' ' {
            EscapeAction::Nbsp(1)
        } else if c == '\n' {
            EscapeAction::DoubleNewline
        } else if c.is_ascii_punctuation() {
            EscapeAction::PrefixBackslash
        } else {
            EscapeAction::PassThrough
        };

        self.in_leading_whitespace =
            c == '\n' || (self.in_leading_whitespace && (c == ' ' || c == '\t'));
        action
    }
}

impl Markdown {
    pub fn new(
        source: SharedString,
        language_registry: Option<Arc<LanguageRegistry>>,
        fallback_code_block_language: Option<LanguageName>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_options(
            source,
            language_registry,
            fallback_code_block_language,
            MarkdownOptions::default(),
            cx,
        )
    }

    pub fn new_with_options(
        source: SharedString,
        language_registry: Option<Arc<LanguageRegistry>>,
        fallback_code_block_language: Option<LanguageName>,
        options: MarkdownOptions,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        let theme_subscription = if options.render_mermaid_diagrams {
            Some(
                cx.observe_global::<theme::GlobalTheme>(|this: &mut Self, cx| {
                    this.invalidate_mermaid_cache(cx);
                }),
            )
        } else {
            None
        };
        let mut this = Self {
            source,
            selection: Selection::default(),
            pressed_link: None,
            pressed_footnote_ref: None,
            autoscroll_request: None,
            pending_heading_scroll: None,
            pending_autoscroll: None,
            active_root_block: None,
            should_reparse: false,
            images_by_source_offset: Default::default(),
            parsed_markdown: ParsedMarkdown::default(),
            pending_parse: None,
            focus_handle,
            language_registry,
            fallback_code_block_language,
            options,
            mermaid_state: MermaidState::default(),
            _mermaid_theme_subscription: theme_subscription,
            mermaid_views: HashMap::default(),
            copied_code_blocks: HashSet::default(),
            wrapped_code_blocks: HashSet::default(),
            code_block_scroll_handles: BTreeMap::default(),
            context_menu_link: None,
            context_menu_selected_text: None,
            context_menu_selected_markdown: None,
            search_highlights: Rc::default(),
            active_search_highlight: None,
        };
        this.parse(cx);
        this
    }

    pub fn new_text(source: SharedString, cx: &mut Context<Self>) -> Self {
        Self::new_with_options(
            source,
            None,
            None,
            MarkdownOptions {
                parse_links_only: true,
                ..Default::default()
            },
            cx,
        )
    }

    fn is_code_block_wrapped(&self, id: usize) -> bool {
        self.wrapped_code_blocks.contains(&id)
    }

    fn toggle_code_block_wrap(&mut self, id: usize) {
        if !self.wrapped_code_blocks.remove(&id) {
            self.wrapped_code_blocks.insert(id);
        }
    }

    fn code_block_scroll_handle(&mut self, id: usize) -> Option<ScrollHandle> {
        (!self.is_code_block_wrapped(id)).then(|| {
            self.code_block_scroll_handles
                .entry(id)
                .or_insert_with(ScrollHandle::new)
                .clone()
        })
    }

    fn retain_code_block_scroll_handles(&mut self, ids: &HashSet<usize>) {
        self.code_block_scroll_handles
            .retain(|id, _| ids.contains(id));
    }

    pub fn invalidate_mermaid_cache(&mut self, cx: &mut Context<Self>) {
        if !self.options.render_mermaid_diagrams || self.parsed_markdown.mermaid_diagrams.is_empty()
        {
            return;
        }

        self.mermaid_state.clear(cx);
        let mermaid_views = &self.mermaid_views;
        self.mermaid_state.update(
            &self.parsed_markdown,
            |source_offset| {
                mermaid_views
                    .get(&source_offset)
                    .map_or(1.0, |view| view.zoom)
            },
            cx,
        );
        cx.notify();
    }

    pub(crate) fn is_mermaid_showing_code(&self, source_offset: usize) -> bool {
        self.mermaid_views
            .get(&source_offset)
            .is_some_and(|view| view.showing_code)
    }

    pub(crate) fn toggle_mermaid_tab(&mut self, source_offset: usize) {
        let view = self.mermaid_views.entry(source_offset).or_default();
        view.showing_code = !view.showing_code;
    }

    pub(crate) fn mermaid_zoom_level(&self, source_offset: usize) -> f32 {
        self.mermaid_views
            .get(&source_offset)
            .map_or(1.0, |view| view.zoom)
    }

    /// The smallest zoom level for a diagram: the scale that makes it span
    /// the content width, capped at 1.0 so diagrams that already fit are
    /// never zoomed out below their natural size. Falls back to 1.0 when the
    /// diagram has no raster yet or the container hasn't been laid out.
    fn mermaid_min_zoom_level(&self, source_offset: usize) -> f32 {
        let Some(diagram) = self.parsed_markdown.mermaid_diagrams.get(&source_offset) else {
            return 1.0;
        };
        let Some(natural_size) = self.mermaid_state.natural_size(&diagram.contents) else {
            return 1.0;
        };
        let Some(container_width) = self
            .mermaid_views
            .get(&source_offset)
            .and_then(|view| view.container_width())
        else {
            return 1.0;
        };
        if natural_size.width <= container_width {
            return 1.0;
        }
        container_width / natural_size.width
    }

    pub(crate) fn set_mermaid_zoom_level(
        &mut self,
        source_offset: usize,
        zoom: f32,
        cx: &mut Context<Self>,
    ) {
        let min_zoom = self.mermaid_min_zoom_level(source_offset);
        let requested_zoom = zoom;
        let mut zoom = zoom.clamp(min_zoom, MERMAID_MAX_ZOOM);
        if (zoom - 1.0).abs() <= MERMAID_ZOOM_SNAP_TOLERANCE {
            zoom = 1.0;
        }
        // The user zoomed out to (or past) the fit-to-width floor. From here
        // on the zoom tracks the container width (see
        // `effective_mermaid_zoom_level`), until the user zooms back in. A
        // zoom landing exactly at 1.0 only sticks when it was clamped, so
        // resetting to the natural size never turns tracking on.
        let zoomed_to_fit = requested_zoom < min_zoom || (zoom <= min_zoom && zoom < 1.0);

        let debounce_task = self.schedule_mermaid_rerasterize(source_offset, cx);
        let view = self.mermaid_views.entry(source_offset).or_default();
        view.zoom = zoom;
        view.zoomed_to_fit = zoomed_to_fit;
        view.debounce_task = Some(debounce_task);
        cx.notify();
    }

    /// The zoom level to display a diagram at, syncing a fit-to-width zoom
    /// with the current container width. Called at render time so that a
    /// fully zoomed-out diagram stays stuck to the container width when the
    /// container is resized, rather than keeping a stale absolute zoom.
    pub(crate) fn effective_mermaid_zoom_level(
        &mut self,
        source_offset: usize,
        cx: &mut Context<Self>,
    ) -> f32 {
        let zoom = self.mermaid_zoom_level(source_offset);
        let zoomed_to_fit = self
            .mermaid_views
            .get(&source_offset)
            .is_some_and(|view| view.zoomed_to_fit);
        if !zoomed_to_fit {
            return zoom;
        }
        let min_zoom = self.mermaid_min_zoom_level(source_offset);
        if (min_zoom - zoom).abs() < 0.001 {
            return zoom;
        }
        let debounce_task = self.schedule_mermaid_rerasterize(source_offset, cx);
        if let Some(view) = self.mermaid_views.get_mut(&source_offset) {
            view.zoom = min_zoom;
            view.debounce_task = Some(debounce_task);
        }
        min_zoom
    }

    /// Schedules a debounced re-raster of a diagram at its current zoom.
    /// Storing the returned task in `MermaidViewState::debounce_task`
    /// replaces (and thereby cancels) the previous timer, debouncing the
    /// expensive re-raster until zoom changes settle. Until then, the
    /// existing raster is displayed scaled to the new zoom.
    fn schedule_mermaid_rerasterize(
        &self,
        source_offset: usize,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(MERMAID_ZOOM_DEBOUNCE).await;
            this.update(cx, |this, cx| {
                if let Some(view) = this.mermaid_views.get_mut(&source_offset) {
                    view.debounce_task = None;
                }
                this.rerasterize_mermaid_diagram(source_offset, cx);
            })
            .ok();
        })
    }

    pub(crate) fn mermaid_scroll_handle(&mut self, source_offset: usize) -> ScrollHandle {
        self.mermaid_views
            .entry(source_offset)
            .or_default()
            .scroll_handle
            .clone()
    }

    /// Re-rasterizes a single mermaid diagram at exactly the scale it is
    /// displayed at, reusing the cached parsed SVG so that neither mermaid
    /// layout nor SVG parsing is re-run. While the new raster is pending, the
    /// previous image keeps being displayed.
    fn rerasterize_mermaid_diagram(&mut self, source_offset: usize, cx: &mut Context<Self>) {
        let Some(diagram) = self.parsed_markdown.mermaid_diagrams.get(&source_offset) else {
            return;
        };
        let contents = diagram.contents.clone();
        let zoom = self.mermaid_zoom_level(source_offset);
        self.mermaid_state.rerasterize_diagram(&contents, zoom, cx);
        cx.notify();
    }

    fn clear_code_block_scroll_handles(&mut self) {
        self.code_block_scroll_handles.clear();
    }

    fn autoscroll_code_block(&self, source_index: usize, cursor_position: Point<Pixels>) {
        let Some((_, scroll_handle)) = self
            .code_block_scroll_handles
            .range(..=source_index)
            .next_back()
        else {
            return;
        };

        let bounds = scroll_handle.bounds();
        if cursor_position.y < bounds.top() || cursor_position.y > bounds.bottom() {
            return;
        }

        let horizontal_delta = if cursor_position.x < bounds.left() {
            bounds.left() - cursor_position.x
        } else if cursor_position.x > bounds.right() {
            bounds.right() - cursor_position.x
        } else {
            return;
        };

        let offset = scroll_handle.offset();
        scroll_handle.set_offset(point(offset.x + horizontal_delta, offset.y));
    }

    pub fn is_parsing(&self) -> bool {
        self.pending_parse.is_some()
    }

    pub fn scroll_to_heading_when_parsed(&mut self, slug: SharedString, cx: &mut Context<Self>) {
        if self.pending_parse.is_some() || self.source.is_empty() {
            self.pending_heading_scroll = Some(slug);
        } else {
            self.scroll_to_heading(&slug, cx);
        }
    }

    pub fn scroll_to_heading(&mut self, slug: &str, cx: &mut Context<Self>) -> Option<usize> {
        if let Some(source_index) = self.parsed_markdown.heading_slugs.get(slug).copied() {
            self.autoscroll_request = Some(source_index);
            cx.notify();
            Some(source_index)
        } else {
            None
        }
    }

    pub fn source(&self) -> &SharedString {
        &self.source
    }

    pub fn non_rendered_source_ranges(&self) -> Vec<Range<usize>> {
        if self.source != self.parsed_markdown.source {
            return Vec::new();
        }

        self.parsed_markdown.non_rendered_source_ranges()
    }

    pub fn first_code_block_language(&self) -> Option<Arc<Language>> {
        self.parsed_markdown.events.iter().find_map(|(_, event)| {
            let MarkdownEvent::Start(MarkdownTag::CodeBlock { kind, .. }) = event else {
                return None;
            };

            self.parsed_markdown.code_block_language(kind)
        })
    }

    pub fn append(&mut self, text: &str, cx: &mut Context<Self>) {
        self.source = SharedString::new(self.source.to_string() + text);
        self.parse(cx);
    }

    pub fn replace(&mut self, source: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.source = source.into();
        self.parse(cx);
    }

    pub fn request_autoscroll_to_source_index(
        &mut self,
        source_index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.pending_parse.is_some() {
            self.pending_autoscroll = Some(source_index);
        } else {
            self.autoscroll_request = Some(source_index);
        }
        cx.refresh_windows();
    }

    fn footnote_definition_content_start(&self, label: &SharedString) -> Option<usize> {
        self.parsed_markdown
            .footnote_definitions
            .get(label)
            .copied()
    }

    pub fn set_active_root_for_source_index(
        &mut self,
        source_index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        let active_root_block =
            source_index.and_then(|index| self.parsed_markdown.root_block_for_source_index(index));
        if self.active_root_block == active_root_block {
            return;
        }

        self.active_root_block = active_root_block;
        cx.notify();
    }

    pub fn reset(&mut self, source: SharedString, cx: &mut Context<Self>) {
        if &source == self.source() {
            if self.pending_parse.is_none() {
                if let Some(slug) = self.pending_heading_scroll.take() {
                    self.pending_autoscroll = None;
                    self.scroll_to_heading(&slug, cx);
                } else if let Some(source_index) = self.pending_autoscroll.take() {
                    self.autoscroll_request = Some(source_index);
                    cx.refresh_windows();
                }
            }
            return;
        }
        if !self.source.is_empty() {
            self.pending_heading_scroll = None;
        }
        self.source = source;
        self.selection = Selection::default();
        self.autoscroll_request = None;
        self.pending_autoscroll = None;
        self.pending_parse = None;
        self.should_reparse = false;
        self.search_highlights = Rc::default();
        self.active_search_highlight = None;
        // Don't clear parsed_markdown here - keep existing content visible until new parse completes
        self.parse(cx);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn parsed_markdown(&self) -> &ParsedMarkdown {
        &self.parsed_markdown
    }

    pub fn escape(s: &str) -> Cow<'_, str> {
        let output_len: usize = {
            let mut escaper = MarkdownEscaper::new();
            s.chars().map(|c| escaper.next(c).output_len(c)).sum()
        };

        if output_len == s.len() {
            return s.into();
        }

        let mut escaper = MarkdownEscaper::new();
        let mut output = String::with_capacity(output_len);
        for c in s.chars() {
            escaper.next(c).write_to(c, &mut output);
        }
        output.into()
    }

    pub fn has_selection(&self) -> bool {
        self.selection.end > self.selection.start
    }

    pub fn selected_source(&self) -> Option<&str> {
        if self.selection.end <= self.selection.start {
            return None;
        }
        self.source.get(self.selection.start..self.selection.end)
    }

    pub fn set_search_highlights(
        &mut self,
        highlights: Vec<Range<usize>>,
        active: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        debug_assert!(
            highlights
                .windows(2)
                .all(|ranges| (ranges[0].start, ranges[0].end) <= (ranges[1].start, ranges[1].end))
        );
        self.search_highlights = highlights.into();
        self.active_search_highlight =
            active.filter(|active| *active < self.search_highlights.len());
        cx.notify();
    }

    pub fn clear_search_highlights(&mut self, cx: &mut Context<Self>) {
        if !self.search_highlights.is_empty() || self.active_search_highlight.is_some() {
            self.search_highlights = Rc::default();
            self.active_search_highlight = None;
            cx.notify();
        }
    }

    pub fn set_active_search_highlight(&mut self, active: Option<usize>, cx: &mut Context<Self>) {
        let active = active.filter(|active| *active < self.search_highlights.len());
        if self.active_search_highlight != active {
            self.active_search_highlight = active;
            cx.notify();
        }
    }

    pub fn search_highlights(&self) -> &[Range<usize>] {
        &self.search_highlights
    }

    pub fn active_search_highlight(&self) -> Option<usize> {
        self.active_search_highlight
    }

    fn copy(&self, text: &RenderedText, _: &mut Window, cx: &mut Context<Self>) {
        if self.selection.end <= self.selection.start {
            return;
        }
        let text = text.text_for_range(self.selection.start..self.selection.end);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn copy_as_markdown(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.context_menu_selected_markdown.take() {
            cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
            return;
        }
        if self.selection.end <= self.selection.start {
            return;
        }
        let text = self
            .parsed_markdown
            .rebalanced_markdown_for_selection(self.selection.start..self.selection.end);
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn capture_for_context_menu(
        &mut self,
        link: Option<SharedString>,
        rendered_text: Option<&RenderedText>,
    ) {
        let range = self.selection.start..self.selection.end;
        if range.end > range.start {
            self.context_menu_selected_markdown = Some(SharedString::new(
                self.parsed_markdown
                    .rebalanced_markdown_for_selection(range.clone()),
            ));
            self.context_menu_selected_text = rendered_text
                .map(|text| text.text_for_range(range))
                .map(SharedString::new)
                .or_else(|| self.context_menu_selected_markdown.clone());
        } else {
            self.context_menu_selected_markdown = None;
            self.context_menu_selected_text = None;
        }
        self.context_menu_link = link;
    }

    /// Returns the URL of the link that was most recently right-clicked, if any.
    /// This is set during a right-click mouse-down event and can be read by parent
    /// views to include a "Copy Link" item in their context menus.
    pub fn context_menu_link(&self) -> Option<&SharedString> {
        self.context_menu_link.as_ref()
    }

    /// Returns the rendered (plain) text that was selected when the most recent
    /// context menu invocation happened.
    pub fn context_menu_selected_text(&self) -> Option<&SharedString> {
        self.context_menu_selected_text.as_ref()
    }

    /// Returns the markdown that was selected when the most recent context
    /// menu invocation happened, rebalanced via
    /// [`ParsedMarkdown::rebalanced_markdown_for_selection`].
    pub fn context_menu_selected_markdown(&self) -> Option<&SharedString> {
        self.context_menu_selected_markdown.as_ref()
    }

    fn parse(&mut self, cx: &mut Context<Self>) {
        if self.source.is_empty() {
            self.should_reparse = false;
            self.pending_parse.take();
            self.pending_heading_scroll = None;
            self.pending_autoscroll = None;
            self.parsed_markdown = ParsedMarkdown {
                source: self.source.clone(),
                ..Default::default()
            };
            self.active_root_block = None;
            self.images_by_source_offset.clear();
            self.mermaid_state.clear(cx);
            cx.notify();
            cx.refresh_windows();
            return;
        }

        if self.pending_parse.is_some() {
            self.should_reparse = true;
            return;
        }
        self.should_reparse = false;
        self.pending_parse = Some(self.start_background_parse(cx));
    }

    fn start_background_parse(&self, cx: &Context<Self>) -> Task<()> {
        let source = self.source.clone();
        let should_parse_links_only = self.options.parse_links_only;
        let should_parse_html = self.options.parse_html;
        let should_render_mermaid_diagrams = self.options.render_mermaid_diagrams;
        let should_parse_heading_slugs = self.options.parse_heading_slugs;
        let should_parse_metadata_blocks = self.options.render_metadata_blocks;
        let language_registry = self.language_registry.clone();
        let fallback = self.fallback_code_block_language.clone();

        let parsed = cx.background_spawn(async move {
            if should_parse_links_only {
                return (
                    ParsedMarkdown {
                        events: Arc::from(parse_links_only(source.as_ref())),
                        source,
                        languages_by_name: TreeMap::default(),
                        languages_by_path: TreeMap::default(),
                        root_block_starts: Arc::default(),
                        html_blocks: BTreeMap::default(),
                        metadata_blocks: BTreeMap::default(),
                        mermaid_diagrams: BTreeMap::default(),
                        heading_slugs: HashMap::default(),
                        footnote_definitions: HashMap::default(),
                        link_definition_spans: Arc::default(),
                        fallback_code_block_language: None,
                        code_block_highlights: Arc::default(),
                    },
                    Default::default(),
                );
            }

            let parsed = parse_markdown_with_options(
                &source,
                should_parse_html,
                should_parse_heading_slugs,
                should_parse_metadata_blocks,
            );
            let events = parsed.events;
            let language_names = parsed.language_names;
            let paths = parsed.language_paths;
            let root_block_starts = parsed.root_block_starts;
            let html_blocks = parsed.html_blocks;
            let metadata_blocks = parsed.metadata_blocks;
            let heading_slugs = parsed.heading_slugs;
            let footnote_definitions = parsed.footnote_definitions;
            let link_definition_spans = parsed.link_definition_spans;
            let has_untagged_code_block = parsed.has_untagged_code_block;
            let mermaid_diagrams = if should_render_mermaid_diagrams {
                extract_mermaid_diagrams(&source, &events)
            } else {
                BTreeMap::default()
            };
            let mut images_by_source_offset = HashMap::default();
            let mut languages_by_name = TreeMap::default();
            let mut languages_by_path = TreeMap::default();
            let mut fallback_code_block_language = None;
            if let Some(registry) = language_registry.as_ref() {
                for name in language_names {
                    let mut language = registry.language_for_name_or_extension(&name).await;
                    if language.is_err()
                        && let Some((first_word, _)) = name.split_once(char::is_whitespace)
                    {
                        language = registry.language_for_name_or_extension(first_word).await;
                    }
                    if let Ok(language) = language {
                        languages_by_name.insert(name, language);
                    }
                }

                for path in paths {
                    if let Ok(language) = registry
                        .load_language_for_file_path(Path::new(path.as_ref()))
                        .await
                    {
                        languages_by_path.insert(path, language);
                    }
                }

                if has_untagged_code_block && let Some(fallback) = &fallback {
                    fallback_code_block_language =
                        registry.language_for_name(fallback.as_ref()).await.ok();
                }
            }

            for (range, event) in &events {
                if let MarkdownEvent::Start(MarkdownTag::Image { dest_url, .. }) = event
                    && let Some(data_url) = dest_url.strip_prefix("data:")
                {
                    let Some((mime_info, data)) = data_url.split_once(',') else {
                        continue;
                    };
                    let Some((mime_type, encoding)) = mime_info.split_once(';') else {
                        continue;
                    };
                    let Some(format) = ImageFormat::from_mime_type(mime_type) else {
                        continue;
                    };
                    let is_base64 = encoding == "base64";
                    if is_base64
                        && let Some(bytes) = base64::prelude::BASE64_STANDARD
                            .decode(data)
                            .log_with_level(Level::Debug)
                    {
                        let image = Arc::new(Image::from_bytes(format, bytes));
                        images_by_source_offset.insert(range.start, image);
                    }
                }
            }

            let mut parsed = ParsedMarkdown {
                source,
                events: Arc::from(events),
                languages_by_name,
                languages_by_path,
                root_block_starts: Arc::from(root_block_starts),
                html_blocks,
                metadata_blocks,
                mermaid_diagrams,
                heading_slugs,
                footnote_definitions,
                link_definition_spans: Arc::from(link_definition_spans),
                fallback_code_block_language,
                code_block_highlights: Arc::default(),
            };
            parsed.code_block_highlights = Arc::new(compute_code_block_highlights(&parsed));
            (parsed, images_by_source_offset)
        });

        cx.spawn(async move |this, cx| {
            let (parsed, images_by_source_offset) = parsed.await;

            this.update(cx, |this, cx| {
                this.parsed_markdown = parsed;
                this.images_by_source_offset = images_by_source_offset;
                if this.active_root_block.is_some_and(|block_index| {
                    block_index >= this.parsed_markdown.root_block_starts.len()
                }) {
                    this.active_root_block = None;
                }
                if this.options.render_mermaid_diagrams {
                    let parsed_markdown = this.parsed_markdown.clone();
                    this.mermaid_views
                        .retain(|offset, _| parsed_markdown.mermaid_diagrams.contains_key(offset));
                    let mermaid_views = &this.mermaid_views;
                    this.mermaid_state.update(
                        &parsed_markdown,
                        |source_offset| {
                            mermaid_views
                                .get(&source_offset)
                                .map_or(1.0, |view| view.zoom)
                        },
                        cx,
                    );
                } else {
                    this.mermaid_state.clear(cx);
                    this.mermaid_views.clear();
                }
                this.pending_parse.take();
                if this.should_reparse {
                    this.parse(cx);
                } else if let Some(slug) = this.pending_heading_scroll.take()
                    && let Some(source_index) =
                        this.parsed_markdown.heading_slugs.get(&slug).copied()
                {
                    this.pending_autoscroll = None;
                    this.autoscroll_request = Some(source_index);
                } else if let Some(source_index) = this.pending_autoscroll.take() {
                    this.autoscroll_request = Some(source_index);
                }
                cx.notify();
                cx.refresh_windows();
            })
            .ok();
        })
    }
}

impl Focusable for Markdown {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

#[derive(Debug, Default, Clone)]
enum SelectMode {
    #[default]
    Character,
    Word(Range<usize>),
    Line(Range<usize>),
    All,
}

#[derive(Clone, Default)]
struct Selection {
    start: usize,
    end: usize,
    reversed: bool,
    pending: bool,
    mode: SelectMode,
}

impl Selection {
    fn set_head(&mut self, head: usize, rendered_text: &RenderedText) {
        match &self.mode {
            SelectMode::Character => {
                if head < self.tail() {
                    if !self.reversed {
                        self.end = self.start;
                        self.reversed = true;
                    }
                    self.start = head;
                } else {
                    if self.reversed {
                        self.start = self.end;
                        self.reversed = false;
                    }
                    self.end = head;
                }
            }
            SelectMode::Word(original_range) | SelectMode::Line(original_range) => {
                let head_range = if matches!(self.mode, SelectMode::Word(_)) {
                    rendered_text.surrounding_word_range(head)
                } else {
                    rendered_text.surrounding_line_range(head)
                };

                if head < original_range.start {
                    self.start = head_range.start;
                    self.end = original_range.end;
                    self.reversed = true;
                } else if head >= original_range.end {
                    self.start = original_range.start;
                    self.end = head_range.end;
                    self.reversed = false;
                } else {
                    self.start = original_range.start;
                    self.end = original_range.end;
                    self.reversed = false;
                }
            }
            SelectMode::All => {
                self.start = 0;
                self.end = rendered_text
                    .lines
                    .last()
                    .map(|line| line.source_end)
                    .unwrap_or(0);
                self.reversed = false;
            }
        }
    }

    fn tail(&self) -> usize {
        if self.reversed { self.end } else { self.start }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParsedMarkdown {
    pub source: SharedString,
    pub events: Arc<[(Range<usize>, MarkdownEvent)]>,
    pub languages_by_name: TreeMap<SharedString, Arc<Language>>,
    pub languages_by_path: TreeMap<Arc<str>, Arc<Language>>,
    pub root_block_starts: Arc<[usize]>,
    pub(crate) html_blocks: BTreeMap<usize, html::html_parser::ParsedHtmlBlock>,
    pub(crate) metadata_blocks: BTreeMap<usize, ParsedMetadataBlock>,
    pub(crate) mermaid_diagrams: BTreeMap<usize, ParsedMarkdownMermaidDiagram>,
    pub heading_slugs: HashMap<SharedString, usize>,
    pub footnote_definitions: HashMap<SharedString, usize>,
    pub(crate) link_definition_spans: Arc<[Range<usize>]>,
    pub(crate) fallback_code_block_language: Option<Arc<Language>>,
    pub(crate) code_block_highlights: Arc<CodeBlockHighlights>,
}

pub(crate) type CodeBlockHighlights = HashMap<usize, ResolvedHighlights>;

impl ParsedMarkdown {
    pub fn source(&self) -> &SharedString {
        &self.source
    }

    pub fn events(&self) -> &Arc<[(Range<usize>, MarkdownEvent)]> {
        &self.events
    }

    pub fn root_block_starts(&self) -> &Arc<[usize]> {
        &self.root_block_starts
    }

    fn non_rendered_source_ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let mut active_link = None;
        let mut active_image: Option<Range<usize>> = None;

        for (event_range, event) in self.events.iter() {
            // An image renders as an image, so nothing between its start and end is text on
            // screen - not the alt text, and not the destination. Skipping these events also
            // keeps the alt text from advancing the enclosing link's cursor, which would
            // otherwise carve it out of the link's non-rendered span.
            if let Some(image_range) = &active_image {
                if matches!(event, MarkdownEvent::End(MarkdownTagEnd::Image))
                    && event_range.end >= image_range.end
                {
                    active_image = None;
                }
                continue;
            }

            match event {
                MarkdownEvent::Start(MarkdownTag::Image { .. }) => {
                    active_image = Some(event_range.clone());
                    if active_link.is_none() {
                        ranges.push(event_range.clone());
                    }
                }
                MarkdownEvent::Start(MarkdownTag::Link { .. }) => {
                    active_link = Some((event_range.clone(), event_range.start));
                }
                MarkdownEvent::Text
                | MarkdownEvent::SubstitutedText(_)
                | MarkdownEvent::Code
                | MarkdownEvent::SubstitutedCode(_) => {
                    let Some((link_range, cursor)) = active_link.as_mut() else {
                        continue;
                    };
                    let visible_start = event_range.start.max(link_range.start);
                    let visible_end = event_range.end.min(link_range.end);
                    if visible_start > *cursor {
                        ranges.push(*cursor..visible_start);
                    }
                    *cursor = (*cursor).max(visible_end);
                }
                MarkdownEvent::End(MarkdownTagEnd::Link) => {
                    if let Some((link_range, cursor)) = active_link.take()
                        && cursor < link_range.end
                    {
                        ranges.push(cursor..link_range.end);
                    }
                }
                _ => {}
            }
        }

        ranges.extend(self.link_definition_spans.iter().cloned());
        // Callers binary search these ranges, which requires them to be sorted and disjoint.
        ranges.sort_by_key(|range| range.start);
        ranges.dedup_by(|next, previous| {
            if next.start <= previous.end {
                previous.end = previous.end.max(next.end);
                true
            } else {
                false
            }
        });
        ranges
    }

    pub fn root_block_for_source_index(&self, source_index: usize) -> Option<usize> {
        if self.root_block_starts.is_empty() {
            return None;
        }

        let partition = self
            .root_block_starts
            .partition_point(|block_start| *block_start <= source_index);

        Some(partition.saturating_sub(1))
    }

    /// Extracts the markdown source for a selection, rebalancing inline
    /// delimiters (`**`, backticks, link syntax, etc.) so partial selections of
    /// styled spans stay well-formed.
    ///
    /// With an exception of a single inline code span, which is returned as plain
    /// text, since copying a command or identifier is the dominant use case there.
    pub fn rebalanced_markdown_for_selection(&self, selection: Range<usize>) -> String {
        selection::rebalanced_markdown_for_selection(
            &self.source,
            &self.events,
            &self.root_block_starts,
            selection,
        )
    }

    pub(crate) fn code_block_language(&self, kind: &CodeBlockKind) -> Option<Arc<Language>> {
        match kind {
            CodeBlockKind::FencedLang(name) => self.languages_by_name.get(name).cloned(),
            CodeBlockKind::FencedSrc(path_range) => {
                self.languages_by_path.get(&path_range.path).cloned()
            }
            CodeBlockKind::Fenced => self.fallback_code_block_language.clone(),
            CodeBlockKind::Indented => None,
        }
    }
}

struct PendingCodeBlock<'a> {
    language: Arc<Language>,
    texts: Vec<(Range<usize>, &'a str)>,
}

fn compute_code_block_highlights(parsed: &ParsedMarkdown) -> CodeBlockHighlights {
    let mut code_block_highlights = CodeBlockHighlights::default();
    let mut pending_block: Option<PendingCodeBlock> = None;
    for (range, event) in parsed.events.iter() {
        match event {
            MarkdownEvent::Start(MarkdownTag::CodeBlock { kind, .. }) => {
                if parsed.mermaid_diagrams.contains_key(&range.start) {
                    pending_block = None;
                    continue;
                }
                pending_block = parsed
                    .code_block_language(kind)
                    .map(|language| PendingCodeBlock {
                        language,
                        texts: Vec::new(),
                    });
            }
            MarkdownEvent::End(MarkdownTagEnd::CodeBlock) => {
                if let Some(block) = pending_block.take() {
                    highlight_code_block(block, &mut code_block_highlights);
                }
            }
            MarkdownEvent::Text => {
                if let Some(block) = &mut pending_block {
                    block
                        .texts
                        .push((range.clone(), &parsed.source[range.clone()]));
                }
            }
            MarkdownEvent::SubstitutedText(text) => {
                if let Some(block) = &mut pending_block {
                    block.texts.push((range.clone(), text.as_str()));
                }
            }
            _ => {}
        }
    }
    code_block_highlights
}

fn highlight_code_block(block: PendingCodeBlock, code_block_highlights: &mut CodeBlockHighlights) {
    let mut combined = String::new();
    let mut text_offsets = Vec::with_capacity(block.texts.len());
    for (_, text) in &block.texts {
        text_offsets.push(combined.len());
        combined.push_str(text);
    }
    let resolved = block
        .language
        .highlight_text_resolved(&Rope::from(combined.as_str()), 0..combined.len());
    if resolved.runs.is_empty() {
        return;
    }
    if let [(source_range, _)] = block.texts.as_slice() {
        code_block_highlights.insert(source_range.start, resolved);
        return;
    }
    let mut runs = resolved.runs.iter().peekable();
    for ((source_range, text), text_offset) in block.texts.iter().zip(text_offsets) {
        let text_end = text_offset + text.len();
        let mut text_runs = Vec::new();
        while let Some((run_range, highlight_id)) = runs.peek() {
            if run_range.start >= text_end {
                break;
            }
            let start = run_range.start.max(text_offset);
            let end = run_range.end.min(text_end);
            if end > start {
                text_runs.push((start - text_offset..end - text_offset, *highlight_id));
            }
            if run_range.end > text_end {
                break;
            }
            runs.next();
        }
        if !text_runs.is_empty() {
            code_block_highlights.insert(
                source_range.start,
                ResolvedHighlights {
                    sources: resolved.sources.clone(),
                    runs: text_runs.into(),
                },
            );
        }
    }
}

pub enum AutoscrollBehavior {
    /// Propagate the request up the element tree for the nearest
    /// scrollable ancestor (e.g. `List`) to handle.
    Propagate,
    /// Directly control a specific scroll handle.
    Controlled(ScrollHandle),
}

pub struct MarkdownElement {
    markdown: Entity<Markdown>,
    style: MarkdownStyle,
    code_block_renderer: CodeBlockRenderer,
    input_focus_handle: Option<FocusHandle>,
    on_url_click: Option<Rc<dyn Fn(SharedString, &mut Window, &mut App)>>,
    on_url_hover: Option<UrlHoverCallback>,
    code_span_link: Option<CodeSpanLinkCallback>,
    on_source_click: Option<SourceClickCallback>,
    on_checkbox_toggle: Option<CheckboxToggleCallback>,
    on_mermaid_zoom: Option<MermaidZoomCallback>,
    image_resolver: Option<Box<dyn Fn(&str, &App) -> Option<ImageSource>>>,
    show_root_block_markers: bool,
    autoscroll: AutoscrollBehavior,
    /// Test-only hook to observe the laid-out text when this element is
    /// rendered beneath a view, where the layout state isn't otherwise
    /// reachable.
    #[cfg(test)]
    on_render: Option<Box<dyn Fn(RenderedText)>>,
}

impl MarkdownElement {
    pub fn new(markdown: Entity<Markdown>, style: MarkdownStyle) -> Self {
        Self {
            markdown,
            style,
            code_block_renderer: CodeBlockRenderer::Default {
                copy_button_visibility: CopyButtonVisibility::VisibleOnHover,
                wrap_button_visibility: WrapButtonVisibility::Hidden,
                border: false,
            },
            input_focus_handle: None,
            on_url_click: None,
            on_url_hover: None,
            code_span_link: None,
            on_source_click: None,
            on_checkbox_toggle: None,
            on_mermaid_zoom: None,
            image_resolver: None,
            show_root_block_markers: false,
            autoscroll: AutoscrollBehavior::Propagate,
            #[cfg(test)]
            on_render: None,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn rendered_text(
        markdown: Entity<Markdown>,
        cx: &mut gpui::VisualTestContext,
        style: impl FnOnce(&Window, &App) -> MarkdownStyle,
    ) -> String {
        use gpui::size;

        let (text, _) = cx.draw(
            Default::default(),
            size(px(600.0), px(600.0)),
            |window, cx| Self::new(markdown, style(window, cx)),
        );
        text.text
            .lines
            .iter()
            .map(|line| line.layout.wrapped_text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn code_block_renderer(mut self, variant: CodeBlockRenderer) -> Self {
        self.code_block_renderer = variant;
        self
    }

    pub fn input_focus_handle(mut self, focus_handle: FocusHandle) -> Self {
        self.input_focus_handle = Some(focus_handle);
        self
    }

    /// Registers a test-only callback invoked with the laid-out text each
    /// time this element runs layout.
    #[cfg(test)]
    pub(crate) fn on_render(mut self, callback: impl Fn(RenderedText) + 'static) -> Self {
        self.on_render = Some(Box::new(callback));
        self
    }

    /// What a click on a link does, instead of opening it.
    pub fn on_url_click(
        mut self,
        handler: impl Fn(SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_url_click = Some(Rc::new(handler));
        self
    }

    pub fn on_url_hover(
        mut self,
        handler: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_url_hover = Some(Rc::new(handler));
        self
    }

    pub fn on_code_span_link(
        mut self,
        callback: impl Fn(&str, &App) -> Option<SharedString> + 'static,
    ) -> Self {
        self.code_span_link = Some(Arc::new(callback));
        self
    }

    pub fn on_source_click(
        mut self,
        handler: impl Fn(usize, usize, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.on_source_click = Some(Box::new(handler));
        self
    }

    pub fn on_checkbox_toggle(
        mut self,
        handler: impl Fn(Range<usize>, bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_checkbox_toggle = Some(Rc::new(handler));
        self
    }

    /// Registers a callback invoked when a mermaid diagram's zoom level changes.
    /// Consumers that scroll the markdown can use this to keep the diagram's
    /// position anchored while it grows or shrinks.
    pub fn on_mermaid_zoom(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_mermaid_zoom = Some(Rc::new(handler));
        self
    }

    pub fn image_resolver(
        mut self,
        resolver: impl Fn(&str, &App) -> Option<ImageSource> + 'static,
    ) -> Self {
        self.image_resolver = Some(Box::new(resolver));
        self
    }

    pub fn show_root_block_markers(mut self) -> Self {
        self.show_root_block_markers = true;
        self
    }

    pub fn scroll_handle(mut self, scroll_handle: ScrollHandle) -> Self {
        self.autoscroll = AutoscrollBehavior::Controlled(scroll_handle);
        self
    }

    fn push_markdown_code_span(
        &self,
        builder: &mut MarkdownElementBuilder,
        text: &str,
        range: Range<usize>,
        cx: &App,
    ) {
        let link_url = if builder.code_block_stack.is_empty()
            && builder.link_depth == 0
            && !self.style.prevent_mouse_interaction
        {
            self.code_span_link
                .as_ref()
                .and_then(|callback| callback(text, cx))
        } else {
            None
        };

        let mut code_style = self.style.inline_code.clone();
        let chip_background = code_style.background_color.take();

        if let Some(url) = link_url {
            builder.push_link(url.clone(), range.clone());
            let link_style = self
                .style
                .link_callback
                .as_ref()
                .and_then(|callback| callback(url.as_ref(), cx))
                .unwrap_or_else(|| self.style.link.clone());
            builder.push_text_style(code_style);
            builder.push_text_style(link_style);
            builder.push_code_chip_text(text, range, chip_background);
            builder.pop_text_style();
            builder.pop_text_style();
        } else {
            if builder.link_depth > 0 {
                code_style.color = self.style.link.color.or(code_style.color);
            }
            builder.push_text_style(code_style);
            builder.push_code_chip_text(text, range, chip_background);
            builder.pop_text_style();
        }
    }

    fn push_markdown_image(
        &self,
        builder: &mut MarkdownElementBuilder,
        range: &Range<usize>,
        source: ImageSource,
        dest_url: SharedString,
        alt_text: Option<SharedString>,
        width: Option<DefiniteLength>,
        height: Option<DefiniteLength>,
    ) {
        let enclosing_link_url = (builder.link_depth > 0)
            .then(|| builder.rendered_links.last())
            .flatten()
            .map(|link| link.destination_url.clone());
        let fallback_opens_image_url = enclosing_link_url.is_none();

        let image_element = {
            let image_start = range.start;
            let wrapper = div()
                .id(("markdown-image-link", range.start))
                .debug_selector(move || format!("markdown_image_{image_start}"))
                .min_w_0();
            let wrapper = if !self.style.prevent_mouse_interaction
                && let Some(url) = enclosing_link_url
            {
                let click_url = url.clone();
                let markdown = self.markdown.clone();
                let url_click = self.on_url_click.clone();
                let bounds = Rc::new(Cell::new(None));
                builder.push_image_link(url.clone(), bounds.clone());
                wrapper
                    .relative()
                    .cursor_pointer()
                    .child(
                        canvas(
                            move |image_bounds, _window, _cx| bounds.set(Some(image_bounds)),
                            |_, _, _, _| {},
                        )
                        .size_full()
                        .absolute()
                        .top_0()
                        .left_0(),
                    )
                    .on_click(move |_, window, cx| {
                        if let Some(ref on_url_click) = url_click {
                            on_url_click(click_url.clone(), window, cx);
                        } else {
                            cx.open_url(&click_url);
                        }
                    })
                    .capture_any_mouse_down(move |event, _window, cx| {
                        if event.button == MouseButton::Right {
                            markdown.update(cx, |md, _| {
                                md.capture_for_context_menu(Some(url.clone()), None)
                            });
                        }
                    })
            } else {
                wrapper
            };
            wrapper.child(
                img(source)
                    .id(("markdown-image", range.start))
                    .min_w_0()
                    .max_w_full()
                    .rounded_md()
                    .mr_1()
                    .mb_1()
                    .when_some(height, |this, height| this.h(height))
                    .when_some(width, |this, width| this.w(width))
                    .with_fallback(move || {
                        image_fallback_element(
                            dest_url.clone(),
                            alt_text.clone(),
                            fallback_opens_image_url,
                        )
                    }),
            )
        };

        builder.push_image_child(image_element);
    }

    fn push_markdown_paragraph(
        &self,
        builder: &mut MarkdownElementBuilder,
        range: &Range<usize>,
        markdown_end: usize,
        text_align_override: Option<TextAlign>,
    ) {
        let align = text_align_override.unwrap_or(self.style.base_text_style.text_align);
        let mut paragraph = div().when(!self.style.height_is_multiple_of_line_height, |el| {
            el.mb(self.style.paragraph_spacing)
                .line_height(self.style.paragraph_line_height)
        });

        paragraph = match align {
            TextAlign::Center => paragraph.text_center(),
            TextAlign::Left => paragraph.text_left(),
            TextAlign::Right => paragraph.text_right(),
        };

        builder.push_text_style(TextStyleRefinement {
            text_align: Some(align),
            ..Default::default()
        });
        builder.push_div(paragraph, range, markdown_end);
    }

    fn pop_markdown_paragraph(&self, builder: &mut MarkdownElementBuilder) {
        builder.pop_div();
        builder.pop_text_style();
    }

    fn push_markdown_heading(
        &self,
        builder: &mut MarkdownElementBuilder,
        level: pulldown_cmark::HeadingLevel,
        range: &Range<usize>,
        markdown_end: usize,
        text_align_override: Option<TextAlign>,
    ) {
        let align = text_align_override.unwrap_or(self.style.base_text_style.text_align);
        let mut heading = div().mt_4().mb_2();
        heading = apply_heading_style(
            heading,
            level,
            self.style.heading_level_styles.as_ref(),
            self.style.heading_border_color,
        );

        heading = match align {
            TextAlign::Center => heading.text_center(),
            TextAlign::Left => heading.text_left(),
            TextAlign::Right => heading.text_right(),
        };

        let mut heading_style = self.style.heading.clone();
        let mut heading_text_style = heading_style.text_style().clone();
        heading.style().refine(&heading_style);

        if let Some(level_style) =
            heading_level_style(level, self.style.heading_level_styles.as_ref())
        {
            heading_text_style.refine(level_style);
        }

        builder.push_text_style(TextStyleRefinement {
            text_align: Some(align),
            ..heading_text_style
        });
        builder.push_div(heading, range, markdown_end);
    }

    fn pop_markdown_heading(&self, builder: &mut MarkdownElementBuilder) {
        builder.pop_div();
        builder.pop_text_style();
    }

    fn push_markdown_block_quote(
        &self,
        builder: &mut MarkdownElementBuilder,
        kind: Option<pulldown_cmark::BlockQuoteKind>,
        range: &Range<usize>,
        markdown_end: usize,
    ) {
        let border_color = self
            .style
            .block_quote_kind_colors
            .for_kind(kind, self.style.block_quote_border_color);

        let header = kind.map(|kind| {
            let (icon_name, label) = match kind {
                BlockQuoteKind::Note => (IconName::Info, "Note"),
                BlockQuoteKind::Tip => (IconName::Sparkle, "Tip"),
                BlockQuoteKind::Important => (IconName::Chat, "Important"),
                BlockQuoteKind::Warning => (IconName::Warning, "Warning"),
                BlockQuoteKind::Caution => (IconName::Stop, "Caution"),
            };
            h_flex()
                .gap_1()
                .items_center()
                .mb_1()
                .child(
                    Icon::new(icon_name)
                        .size(IconSize::Small)
                        .color(Color::Custom(border_color)),
                )
                .child(
                    Label::new(label)
                        .color(Color::Custom(border_color))
                        .weight(FontWeight::BOLD),
                )
                .into_any_element()
        });

        let block_div = div()
            .pl_4()
            .mb(self.style.paragraph_spacing)
            .border_l_4()
            .border_color(border_color);
        let block_div = match header {
            Some(header) => block_div.child(header),
            None => block_div,
        };

        builder.push_text_style(self.style.block_quote.clone());
        builder.push_div(block_div, range, markdown_end);
    }

    fn pop_markdown_block_quote(&self, builder: &mut MarkdownElementBuilder) {
        builder.pop_div();
        builder.pop_text_style();
    }

    fn push_metadata_block(
        &self,
        builder: &mut MarkdownElementBuilder,
        source: &str,
        metadata_block: &ParsedMetadataBlock,
        markdown_end: usize,
        cx: &App,
    ) {
        let content_range = &metadata_block.content_range;
        if let Some(rows) = metadata_block.rows.as_deref() {
            builder.push_div(
                div()
                    .grid()
                    .grid_cols(2)
                    .w_full()
                    .mb_2()
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .rounded_sm()
                    .overflow_hidden(),
                content_range,
                markdown_end,
            );

            for (row_index, row) in rows.iter().enumerate() {
                self.push_metadata_cell(
                    builder,
                    source,
                    row.key.clone(),
                    content_range,
                    markdown_end,
                    MetadataCellStyle {
                        row_index,
                        is_key: true,
                    },
                    cx,
                );
                self.push_metadata_cell(
                    builder,
                    source,
                    row.value.clone(),
                    content_range,
                    markdown_end,
                    MetadataCellStyle {
                        row_index,
                        is_key: false,
                    },
                    cx,
                );
            }

            builder.pop_div();
        } else {
            let mut metadata_block = div().w_full().rounded_md();
            metadata_block.style().refine(&self.style.code_block);
            builder.push_text_style(self.style.code_block.text.to_owned());
            builder.push_code_block(None);
            builder.push_div(metadata_block, content_range, markdown_end);
            builder.push_text(&source[content_range.clone()], content_range.clone());
            builder.trim_trailing_newline();
            builder.pop_div();
            builder.pop_code_block();
            builder.pop_text_style();
        }
    }

    fn push_metadata_cell(
        &self,
        builder: &mut MarkdownElementBuilder,
        source: &str,
        text_range: Range<usize>,
        block_range: &Range<usize>,
        markdown_end: usize,
        cell_style: MetadataCellStyle,
        cx: &App,
    ) {
        builder.push_div(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .px_2()
                .py_1()
                .border_color(cx.theme().colors().border)
                .when(cell_style.row_index > 0, |this| this.border_t_1())
                .when(!cell_style.is_key, |this| this.border_l_1())
                .when(cell_style.is_key, |this| {
                    this.bg(cx.theme().colors().panel_background)
                }),
            block_range,
            markdown_end,
        );

        let text_style = if cell_style.is_key {
            TextStyleRefinement {
                color: Some(cx.theme().colors().text_muted),
                font_weight: Some(FontWeight::SEMIBOLD),
                ..Default::default()
            }
        } else {
            TextStyleRefinement::default()
        };
        builder.push_text_style(text_style);
        builder.push_text(&source[text_range.clone()], text_range);
        builder.pop_text_style();
        builder.pop_div();
    }

    fn push_markdown_list_item(
        &self,
        builder: &mut MarkdownElementBuilder,
        bullet: AnyElement,
        range: &Range<usize>,
        markdown_end: usize,
    ) {
        builder.push_div(
            div()
                .when(!self.style.height_is_multiple_of_line_height, |el| {
                    el.mb_1()
                        .gap_1()
                        .line_height(self.style.paragraph_line_height)
                })
                .h_flex()
                .items_start()
                .child(bullet),
            range,
            markdown_end,
        );
        // Without `w_0`, text doesn't wrap to the width of the container.
        builder.push_div(div().flex_1().w_0(), range, markdown_end);
    }

    fn pop_markdown_list_item(&self, builder: &mut MarkdownElementBuilder) {
        builder.pop_div();
        builder.pop_div();
    }

    fn paint_mouse_listeners(
        &mut self,
        hitbox: &Hitbox,
        rendered_text: &RenderedText,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.style.prevent_mouse_interaction {
            return;
        }

        let is_hovering_clickable = hitbox.is_hovered(window)
            && !self.markdown.read(cx).selection.pending
            && (rendered_text
                .image_link_for_position(window.mouse_position())
                .is_some()
                || rendered_text
                    .source_index_for_position(window.mouse_position())
                    .ok()
                    .is_some_and(|source_index| {
                        rendered_text.link_for_source_index(source_index).is_some()
                            || rendered_text
                                .footnote_ref_for_source_index(source_index)
                                .is_some()
                    }));

        if is_hovering_clickable {
            window.set_cursor_style(CursorStyle::PointingHand, hitbox);
        } else {
            window.set_cursor_style(CursorStyle::IBeam, hitbox);
        }

        let on_open_url = self.on_url_click.take();
        let on_url_hover = self.on_url_hover.take();
        let on_source_click = self.on_source_click.take();

        self.on_mouse_event(window, cx, {
            let hitbox = hitbox.clone();
            let rendered_text = rendered_text.clone();
            move |markdown, event: &MouseDownEvent, phase, window, _cx| {
                if phase.capture()
                    && event.button == MouseButton::Right
                    && hitbox.is_hovered(window)
                {
                    let link = rendered_text
                        .source_index_for_position(event.position)
                        .ok()
                        .and_then(|ix| rendered_text.link_for_source_index(ix))
                        .map(|link| link.destination_url.clone());
                    markdown.capture_for_context_menu(link, Some(&rendered_text));
                }
            }
        });

        self.on_mouse_event(window, cx, {
            let rendered_text = rendered_text.clone();
            let hitbox = hitbox.clone();
            move |markdown, event: &MouseDownEvent, phase, window, cx| {
                if hitbox.is_hovered(window) {
                    if phase.bubble() && event.button != MouseButton::Right {
                        let position_result =
                            rendered_text.source_index_for_position(event.position);

                        if let Ok(source_index) = position_result {
                            if let Some(footnote_ref) =
                                rendered_text.footnote_ref_for_source_index(source_index)
                            {
                                markdown.pressed_footnote_ref = Some(footnote_ref.clone());
                            } else if let Some(link) =
                                rendered_text.link_for_source_index(source_index)
                            {
                                markdown.pressed_link = Some(link.clone());
                            }
                        }

                        if markdown.pressed_footnote_ref.is_none()
                            && markdown.pressed_link.is_none()
                        {
                            let source_index = match position_result {
                                Ok(ix) | Err(ix) => ix,
                            };
                            if let Some(handler) = on_source_click.as_ref() {
                                let blocked = handler(source_index, event.click_count, window, cx);
                                if blocked {
                                    markdown.selection = Selection::default();
                                    markdown.pressed_link = None;
                                    window.prevent_default();
                                    cx.notify();
                                    return;
                                }
                            }
                            let (range, mode, reversed) = match event.click_count {
                                1 if event.modifiers.shift => {
                                    let tail = markdown.selection.tail();
                                    let reversed = source_index < tail;
                                    let range = if reversed {
                                        source_index..tail
                                    } else {
                                        tail..source_index
                                    };
                                    (range, SelectMode::Character, reversed)
                                }
                                1 => {
                                    let range = source_index..source_index;
                                    (range, SelectMode::Character, false)
                                }
                                2 => {
                                    let range = rendered_text.surrounding_word_range(source_index);
                                    (range.clone(), SelectMode::Word(range), false)
                                }
                                3 => {
                                    let range = rendered_text.surrounding_line_range(source_index);
                                    (range.clone(), SelectMode::Line(range), false)
                                }
                                _ => {
                                    let range = 0..rendered_text
                                        .lines
                                        .last()
                                        .map(|line| line.source_end)
                                        .unwrap_or(0);
                                    (range, SelectMode::All, false)
                                }
                            };
                            markdown.selection = Selection {
                                start: range.start,
                                end: range.end,
                                reversed,
                                pending: true,
                                mode,
                            };
                            window.focus(&markdown.focus_handle, cx);
                        }

                        window.prevent_default();
                        cx.notify();
                    }
                } else if phase.capture() && event.button == MouseButton::Left {
                    markdown.selection = Selection::default();
                    markdown.pressed_link = None;
                    cx.notify();
                }
            }
        });
        self.on_mouse_event(window, cx, {
            let rendered_text = rendered_text.clone();
            let hitbox = hitbox.clone();
            let was_hovering_clickable = is_hovering_clickable;
            move |markdown, event: &MouseMoveEvent, phase, window, cx| {
                if phase.capture() {
                    return;
                }

                if markdown.selection.pending {
                    let source_index = match rendered_text.source_index_for_position(event.position)
                    {
                        Ok(ix) | Err(ix) => ix,
                    };
                    markdown.selection.set_head(source_index, &rendered_text);
                    markdown.autoscroll_code_block(source_index, event.position);
                    markdown.autoscroll_request = Some(source_index);
                    cx.notify();
                } else {
                    let is_hitbox_hovered = hitbox.is_hovered(window);
                    let source_index = is_hitbox_hovered
                        .then(|| rendered_text.source_index_for_position(event.position).ok())
                        .flatten();
                    let hovered_url = is_hitbox_hovered
                        .then(|| rendered_text.image_link_for_position(event.position))
                        .flatten()
                        .map(|image| image.destination_url.clone())
                        .or_else(|| {
                            source_index
                                .and_then(|source_index| {
                                    rendered_text.link_for_source_index(source_index)
                                })
                                .map(|link| link.destination_url.clone())
                        });
                    let is_hovering_clickable = hovered_url.is_some()
                        || source_index.is_some_and(|source_index| {
                            rendered_text
                                .footnote_ref_for_source_index(source_index)
                                .is_some()
                        });
                    if let Some(on_url_hover) = on_url_hover.as_ref() {
                        on_url_hover(hovered_url, window, cx);
                    }
                    if is_hovering_clickable != was_hovering_clickable {
                        cx.notify();
                    }
                }
            }
        });
        self.on_mouse_event(window, cx, {
            let rendered_text = rendered_text.clone();
            move |markdown, event: &MouseUpEvent, phase, window, cx| {
                if phase.bubble() {
                    let source_index = rendered_text.source_index_for_position(event.position).ok();
                    if let Some(pressed_footnote_ref) = markdown.pressed_footnote_ref.take()
                        && source_index
                            .and_then(|ix| rendered_text.footnote_ref_for_source_index(ix))
                            == Some(&pressed_footnote_ref)
                    {
                        if let Some(source_index) =
                            markdown.footnote_definition_content_start(&pressed_footnote_ref.label)
                        {
                            markdown.autoscroll_request = Some(source_index);
                            cx.notify();
                        }
                    } else if let Some(pressed_link) = markdown.pressed_link.take()
                        && source_index.and_then(|ix| rendered_text.link_for_source_index(ix))
                            == Some(&pressed_link)
                    {
                        if let Some(open_url) = on_open_url.as_ref() {
                            open_url(pressed_link.destination_url, window, cx);
                        } else {
                            cx.open_url(&pressed_link.destination_url);
                        }
                    }
                } else if markdown.selection.pending {
                    markdown.selection.pending = false;
                    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
                    {
                        let text = rendered_text
                            .text_for_range(markdown.selection.start..markdown.selection.end);
                        cx.write_to_primary(ClipboardItem::new_string(text))
                    }
                    cx.notify();
                }
            }
        });
    }

    fn autoscroll(
        &self,
        rendered_text: &RenderedText,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<()> {
        let autoscroll_index = self
            .markdown
            .update(cx, |markdown, _| markdown.autoscroll_request.take())?;
        let (position, line_height) = rendered_text.position_for_source_index(autoscroll_index)?;

        match &self.autoscroll {
            AutoscrollBehavior::Controlled(scroll_handle) => {
                let viewport = scroll_handle.bounds();
                let margin = line_height * 3.;
                let top_goal = viewport.top() + margin;
                let bottom_goal = viewport.bottom() - margin;
                let current_offset = scroll_handle.offset();

                let new_offset_y = if position.y < top_goal {
                    current_offset.y + (top_goal - position.y)
                } else if position.y + line_height > bottom_goal {
                    current_offset.y + (bottom_goal - (position.y + line_height))
                } else {
                    current_offset.y
                };

                scroll_handle.set_offset(point(
                    current_offset.x,
                    new_offset_y.clamp(-scroll_handle.max_offset().y, Pixels::ZERO),
                ));
            }
            AutoscrollBehavior::Propagate => {
                let text_style = self.style.base_text_style.clone();
                let font_id = window.text_system().resolve_font(&text_style.font());
                let font_size = text_style.font_size.to_pixels(window.rem_size());
                let em_width = window.text_system().em_width(font_id, font_size).unwrap();
                window.request_autoscroll(Bounds::from_corners(
                    point(position.x - 3. * em_width, position.y - 3. * line_height),
                    point(position.x + 3. * em_width, position.y + 3. * line_height),
                ));
            }
        }
        Some(())
    }

    fn on_mouse_event<T: MouseEvent>(
        &self,
        window: &mut Window,
        _cx: &mut App,
        mut f: impl 'static
        + FnMut(&mut Markdown, &T, DispatchPhase, &mut Window, &mut Context<Markdown>),
    ) {
        window.on_mouse_event({
            let markdown = self.markdown.downgrade();
            move |event, phase, window, cx| {
                markdown
                    .update(cx, |markdown, cx| f(markdown, event, phase, window, cx))
                    .log_err();
            }
        });
    }
}

impl Styled for MarkdownElement {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style.container_style
    }
}

impl Element for MarkdownElement {
    type RequestLayoutState = RenderedMarkdown;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        let highlights = {
            let markdown = self.markdown.read(cx);
            let colors = cx.theme().colors();
            let selection = &markdown.selection;
            MarkdownHighlights {
                search_highlights: markdown.search_highlights.clone(),
                active_search_highlight: markdown.active_search_highlight,
                search_match_color: colors.search_match_background,
                active_search_match_color: colors.search_active_match_background,
                selection: (selection.start < selection.end).then(|| {
                    (
                        selection.start..selection.end,
                        self.style.selection_background_color,
                    )
                }),
                next_search_highlight_ix: 0,
            }
        };

        let (parsed_markdown, images, active_root_block, render_mermaid_diagrams, mermaid_state) = {
            let markdown = self.markdown.read(cx);
            (
                markdown.parsed_markdown.clone(),
                markdown.images_by_source_offset.clone(),
                markdown.active_root_block,
                markdown.options.render_mermaid_diagrams,
                markdown.mermaid_state.clone(),
            )
        };
        let mut builder = MarkdownElementBuilder::new(
            &self.style.container_style,
            self.style.base_text_style.clone(),
            self.style.syntax.clone(),
            highlights,
            parsed_markdown.code_block_highlights.clone(),
        );
        let markdown_end = if let Some(last) = parsed_markdown.events.last() {
            last.0.end
        } else {
            0
        };
        let mut code_block_ids = HashSet::default();

        let mut current_img_block_range: Option<Range<usize>> = None;
        let mut handled_html_block = false;
        let mut rendered_mermaid_block = false;
        let mut rendered_metadata_block = false;
        for (index, (range, event)) in parsed_markdown.events.iter().enumerate() {
            // Skip alt text for images that rendered
            if let Some(current_img_block_range) = &current_img_block_range
                && current_img_block_range.end > range.end
            {
                continue;
            }

            if handled_html_block {
                if let MarkdownEvent::End(MarkdownTagEnd::HtmlBlock) = event {
                    handled_html_block = false;
                } else {
                    continue;
                }
            }

            if rendered_mermaid_block {
                if matches!(event, MarkdownEvent::End(MarkdownTagEnd::CodeBlock)) {
                    rendered_mermaid_block = false;
                }
                continue;
            }

            if rendered_metadata_block {
                if matches!(event, MarkdownEvent::End(MarkdownTagEnd::MetadataBlock(_))) {
                    rendered_metadata_block = false;
                }
                continue;
            }

            match event {
                MarkdownEvent::RootStart => {
                    if self.show_root_block_markers {
                        builder.push_root_block(range, markdown_end);
                    }
                }
                MarkdownEvent::RootEnd(root_block_index) => {
                    if self.show_root_block_markers {
                        builder.pop_root_block(
                            active_root_block == Some(*root_block_index),
                            cx.theme().colors().border,
                            cx.theme().colors().border_variant,
                        );
                    }
                }
                MarkdownEvent::Start(tag) => {
                    match tag {
                        MarkdownTag::Image { dest_url, .. } => {
                            let alt_text = collect_image_alt_text(
                                &parsed_markdown.events[index..],
                                &parsed_markdown.source,
                            );
                            if let Some(image) = images.get(&range.start) {
                                current_img_block_range = Some(range.clone());
                                self.push_markdown_image(
                                    &mut builder,
                                    range,
                                    image.clone().into(),
                                    dest_url.clone(),
                                    alt_text,
                                    None,
                                    None,
                                );
                            } else if let Some(source) = self
                                .image_resolver
                                .as_ref()
                                .and_then(|resolve| resolve(dest_url.as_ref(), cx))
                            {
                                current_img_block_range = Some(range.clone());
                                self.push_markdown_image(
                                    &mut builder,
                                    range,
                                    source,
                                    dest_url.clone(),
                                    alt_text,
                                    None,
                                    None,
                                );
                            }
                        }
                        MarkdownTag::Paragraph => {
                            let text_align_override = builder
                                .table
                                .current_cell_alignment()
                                .and_then(alignment_to_text_align);
                            self.push_markdown_paragraph(
                                &mut builder,
                                range,
                                markdown_end,
                                text_align_override,
                            );
                        }
                        MarkdownTag::Heading { level, .. } => {
                            let text_align_override = builder
                                .table
                                .current_cell_alignment()
                                .and_then(alignment_to_text_align);
                            self.push_markdown_heading(
                                &mut builder,
                                *level,
                                range,
                                markdown_end,
                                text_align_override,
                            );
                        }
                        MarkdownTag::BlockQuote(kind) => {
                            self.push_markdown_block_quote(
                                &mut builder,
                                *kind,
                                range,
                                markdown_end,
                            );
                        }
                        MarkdownTag::CodeBlock { kind, .. } => {
                            if render_mermaid_diagrams
                                && let Some(mermaid_diagram) =
                                    parsed_markdown.mermaid_diagrams.get(&range.start)
                            {
                                let (showing_code, zoom) =
                                    self.markdown.update(cx, |markdown, cx| {
                                        (
                                            markdown.is_mermaid_showing_code(range.start),
                                            markdown.effective_mermaid_zoom_level(range.start, cx),
                                        )
                                    });
                                let copy_button_visibility = match &self.code_block_renderer {
                                    CodeBlockRenderer::Default {
                                        copy_button_visibility,
                                        ..
                                    } => *copy_button_visibility,
                                    _ => CopyButtonVisibility::VisibleOnHover,
                                };
                                builder.push_sourced_element(
                                    mermaid_diagram.content_range.clone(),
                                    render_mermaid_diagram(
                                        mermaid_diagram,
                                        &mermaid_state,
                                        &self.style,
                                        self.markdown.clone(),
                                        range.start,
                                        showing_code,
                                        zoom,
                                        copy_button_visibility,
                                        self.on_mermaid_zoom.clone(),
                                        window,
                                        cx,
                                    ),
                                );
                                rendered_mermaid_block = true;
                                continue;
                            }

                            let language = parsed_markdown.code_block_language(kind);

                            let is_indented = matches!(kind, CodeBlockKind::Indented);
                            let scroll_handle = if self.style.code_block_overflow_x_scroll {
                                self.markdown.update(cx, |markdown, _| {
                                    markdown.code_block_scroll_handle(range.start)
                                })
                            } else {
                                None
                            };
                            if scroll_handle.is_some() {
                                code_block_ids.insert(range.start);
                            }

                            match (&self.code_block_renderer, is_indented) {
                                (CodeBlockRenderer::Default { .. }, _) | (_, true) => {
                                    // This is a parent container that we can position the copy button inside.
                                    let parent_container =
                                        div().group("code_block").relative().w_full();

                                    let mut parent_container: AnyDiv = if let Some(scroll_handle) =
                                        scroll_handle.as_ref()
                                    {
                                        let scrollbars = Scrollbars::new(ScrollAxes::Horizontal)
                                            .id(("markdown-code-block-scrollbar", range.start))
                                            .tracked_scroll_handle(scroll_handle)
                                            .with_track_along(
                                                ScrollAxes::Horizontal,
                                                cx.theme().colors().editor_background,
                                            )
                                            .notify_content();

                                        parent_container
                                            .rounded_lg()
                                            .custom_scrollbars(scrollbars, window, cx)
                                            .into()
                                    } else {
                                        parent_container.into()
                                    };

                                    if let CodeBlockRenderer::Default { border: true, .. } =
                                        &self.code_block_renderer
                                    {
                                        parent_container = parent_container
                                            .rounded_md()
                                            .border_1()
                                            .border_color(cx.theme().colors().border_variant);
                                    }

                                    parent_container.style().refine(&self.style.code_block);
                                    builder.push_div(parent_container, range, markdown_end);

                                    let code_block = div()
                                        .id(("code-block", range.start))
                                        .rounded_lg()
                                        .map(|code_block| {
                                            if let Some(scroll_handle) = scroll_handle.as_ref() {
                                                code_block
                                                    .flex()
                                                    .overflow_x_scroll()
                                                    .restrict_scroll_to_axis()
                                                    .track_scroll(scroll_handle)
                                            } else {
                                                code_block.w_full()
                                            }
                                        });

                                    builder.push_text_style(self.style.code_block.text.to_owned());
                                    builder.push_code_block(language);
                                    builder.push_div(code_block, range, markdown_end);
                                }
                                (CodeBlockRenderer::Custom { .. }, _) => {}
                            }
                        }
                        MarkdownTag::HtmlBlock => {
                            builder.push_div(div(), range, markdown_end);
                            if let Some(block) = parsed_markdown.html_blocks.get(&range.start) {
                                self.render_html_block(block, &mut builder, markdown_end, cx);
                                handled_html_block = true;
                            }
                        }
                        MarkdownTag::List(bullet_index) => {
                            builder.push_list(*bullet_index);
                            let is_top_level = builder.list_stack.len() == 1;
                            builder.push_div(
                                div()
                                    .pl_2p5()
                                    .when(is_top_level, |this| this.mb(self.style.list_spacing)),
                                range,
                                markdown_end,
                            );
                        }
                        MarkdownTag::Item => {
                            let bullet = if let Some((task_range, checked)) =
                                task_list_marker_for_item(&parsed_markdown.events, index)
                            {
                                let source = &parsed_markdown.source()[range.clone()];
                                let checkbox = Checkbox::new(
                                    ElementId::Name(source.to_string().into()),
                                    ToggleState::from(checked),
                                )
                                .fill();

                                if let Some(on_toggle) = self.on_checkbox_toggle.clone() {
                                    let task_source_range = task_range.clone();
                                    checkbox
                                        .on_click(move |_state, window, cx| {
                                            on_toggle(
                                                task_source_range.clone(),
                                                !checked,
                                                window,
                                                cx,
                                            );
                                        })
                                        .into_any_element()
                                } else {
                                    checkbox.visualization_only(true).into_any_element()
                                }
                            } else if let Some(bullet_index) = builder.next_bullet_index() {
                                div().child(format!("{}.", bullet_index)).into_any_element()
                            } else {
                                div().child("•").into_any_element()
                            };
                            self.push_markdown_list_item(&mut builder, bullet, range, markdown_end);
                        }
                        MarkdownTag::Emphasis => builder.push_text_style(TextStyleRefinement {
                            font_style: Some(FontStyle::Italic),
                            ..Default::default()
                        }),
                        MarkdownTag::Strong => builder.push_text_style(TextStyleRefinement {
                            font_weight: Some(FontWeight::BOLD),
                            color: Some(cx.theme().colors().text),
                            ..Default::default()
                        }),
                        MarkdownTag::Strikethrough => {
                            builder.push_text_style(TextStyleRefinement {
                                strikethrough: Some(StrikethroughStyle {
                                    thickness: px(1.),
                                    color: None,
                                }),
                                ..Default::default()
                            })
                        }
                        MarkdownTag::Link { dest_url, .. } => {
                            if builder.code_block_stack.is_empty() {
                                builder.link_depth += 1;
                                builder.push_link(dest_url.clone(), range.clone());
                                let style = self
                                    .style
                                    .link_callback
                                    .as_ref()
                                    .and_then(|callback| callback(dest_url, cx))
                                    .unwrap_or_else(|| self.style.link.clone());
                                builder.push_text_style(style)
                            }
                        }
                        MarkdownTag::FootnoteDefinition(label) => {
                            if !builder.rendered_footnote_separator {
                                builder.rendered_footnote_separator = true;
                                builder.push_div(
                                    div()
                                        .border_t_1()
                                        .mt_2()
                                        .border_color(self.style.rule_color),
                                    range,
                                    markdown_end,
                                );
                                builder.pop_div();
                            }
                            builder.push_div(
                                div()
                                    .pt_1()
                                    .mb_1()
                                    .line_height(rems(1.3))
                                    .text_size(rems(0.85))
                                    .h_flex()
                                    .items_start()
                                    .gap_2()
                                    .child(
                                        div().text_size(rems(0.85)).child(format!("{}.", label)),
                                    ),
                                range,
                                markdown_end,
                            );
                            builder.push_div(div().flex_1().w_0(), range, markdown_end);
                        }
                        MarkdownTag::MetadataBlock(_) => {
                            if let Some(metadata_block) =
                                parsed_markdown.metadata_blocks.get(&range.start)
                            {
                                self.push_metadata_block(
                                    &mut builder,
                                    &parsed_markdown.source,
                                    metadata_block,
                                    markdown_end,
                                    cx,
                                );
                                rendered_metadata_block = true;
                            }
                        }
                        MarkdownTag::Table(alignments) => {
                            builder.table.start(alignments.clone());

                            let column_count = alignments.len();
                            builder.push_div(div().flex(), range, markdown_end);
                            builder.push_div(
                                div()
                                    .id(("table", range.start))
                                    .debug_selector(|| "markdown_table".into())
                                    .min_w_0()
                                    .grid()
                                    .when(self.style.table_columns_min_size, |this| {
                                        this.w_full().grid_cols_min_content(column_count as u16)
                                    })
                                    .when(!self.style.table_columns_min_size, |this| {
                                        this.grid_cols_max_content(column_count as u16)
                                    })
                                    .mb_2()
                                    .border(px(1.5))
                                    .border_color(cx.theme().colors().border)
                                    .rounded_sm()
                                    .restrict_scroll_to_axis()
                                    .custom_scrollbars(
                                        Scrollbars::new(ScrollAxes::Horizontal)
                                            .id(("markdown-table-scrollbar", range.start))
                                            .notify_content(),
                                        window,
                                        cx,
                                    ),
                                range,
                                markdown_end,
                            );
                        }
                        MarkdownTag::TableHead => {
                            builder.table.start_head();
                            builder.push_text_style(TextStyleRefinement {
                                font_weight: Some(FontWeight::SEMIBOLD),
                                ..Default::default()
                            });
                        }
                        MarkdownTag::TableRow => {
                            builder.table.start_row();
                        }
                        MarkdownTag::TableCell => {
                            builder.table.start_cell();
                            let is_header = builder.table.in_head;
                            let row_index = builder.table.row_index;
                            let col_index = builder.table.col_index;
                            let alignment = builder.table.current_cell_alignment();
                            let text_align = alignment
                                .and_then(alignment_to_text_align)
                                .unwrap_or(self.style.base_text_style.text_align);

                            let mut cell_div = div()
                                .debug_selector(|| {
                                    if is_header {
                                        format!("markdown_table_header_cell_{col_index}")
                                    } else {
                                        format!("markdown_table_cell_{row_index}_{col_index}")
                                    }
                                })
                                .flex()
                                .flex_col()
                                .h_full()
                                .when(col_index > 0, |this| this.border_l_1())
                                .when(row_index > 0, |this| this.border_t_1())
                                .border_color(cx.theme().colors().border)
                                .px(self.style.table_cell_padding.x)
                                .py(self.style.table_cell_padding.y)
                                .when(is_header, |this| {
                                    this.bg(cx.theme().colors().title_bar_background)
                                })
                                .when(!is_header && row_index % 2 == 1, |this| {
                                    this.bg(cx.theme().colors().panel_background)
                                });

                            cell_div = match alignment {
                                Some(Alignment::Center) => cell_div.items_center(),
                                Some(Alignment::Right) => cell_div.items_end(),
                                _ => cell_div,
                            };

                            builder.push_text_style(TextStyleRefinement {
                                text_align: Some(text_align),
                                ..Default::default()
                            });
                            builder.push_div(cell_div, range, markdown_end);
                            builder.push_div(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .w_full()
                                    .justify_center()
                                    .text_align(text_align),
                                range,
                                markdown_end,
                            );
                        }
                        _ => log::debug!("unsupported markdown tag {:?}", tag),
                    }
                }
                MarkdownEvent::End(tag) => match tag {
                    MarkdownTagEnd::Image => {
                        current_img_block_range.take();
                    }
                    MarkdownTagEnd::Paragraph => {
                        self.pop_markdown_paragraph(&mut builder);
                    }
                    MarkdownTagEnd::Heading(_) => {
                        self.pop_markdown_heading(&mut builder);
                    }
                    MarkdownTagEnd::BlockQuote(_kind) => {
                        self.pop_markdown_block_quote(&mut builder);
                    }
                    MarkdownTagEnd::CodeBlock => {
                        builder.trim_trailing_newline();

                        builder.pop_div();
                        builder.pop_code_block();
                        builder.pop_text_style();

                        if let CodeBlockRenderer::Default {
                            copy_button_visibility,
                            wrap_button_visibility,
                            ..
                        } = &self.code_block_renderer
                            && (*copy_button_visibility != CopyButtonVisibility::Hidden
                                || *wrap_button_visibility != WrapButtonVisibility::Hidden)
                        {
                            let copy_button_visibility = *copy_button_visibility;
                            let wrap_button_visibility = *wrap_button_visibility;
                            builder.modify_current_div(|el| {
                                let content_range = parser::extract_code_block_content_range(
                                    &parsed_markdown.source()[range.clone()],
                                );
                                let content_range = content_range.start + range.start
                                    ..content_range.end + range.start;

                                let code = parsed_markdown.source()[content_range].to_string();

                                let any_hover = copy_button_visibility
                                    == CopyButtonVisibility::VisibleOnHover
                                    || wrap_button_visibility
                                        == WrapButtonVisibility::VisibleOnHover;
                                let any_always = copy_button_visibility
                                    == CopyButtonVisibility::AlwaysVisible
                                    || wrap_button_visibility
                                        == WrapButtonVisibility::AlwaysVisible;
                                let use_hover = any_hover && !any_always;

                                let button_row = h_flex()
                                    .gap_0p5()
                                    .absolute()
                                    .bg(cx.theme().colors().editor_background)
                                    .when_else(
                                        use_hover,
                                        |this| {
                                            this.top_1().right_1().visible_on_hover("code_block")
                                        },
                                        |this| this.top_1p5().right_1p5(),
                                    )
                                    .when(
                                        wrap_button_visibility != WrapButtonVisibility::Hidden,
                                        |this| {
                                            let is_wrapped = self
                                                .markdown
                                                .read(cx)
                                                .is_code_block_wrapped(range.start);

                                            this.child(render_wrap_code_block_button(
                                                range.start,
                                                is_wrapped,
                                                self.markdown.clone(),
                                            ))
                                        },
                                    )
                                    .when(
                                        copy_button_visibility != CopyButtonVisibility::Hidden,
                                        |this| {
                                            this.child(render_copy_code_block_button(
                                                range.end,
                                                code,
                                                self.markdown.clone(),
                                            ))
                                        },
                                    );

                                el.child(button_row)
                            });
                        }

                        // Pop the parent container.
                        builder.pop_div();
                    }
                    MarkdownTagEnd::HtmlBlock => builder.pop_div(),
                    MarkdownTagEnd::List(_) => {
                        builder.pop_list();
                        builder.pop_div();
                    }
                    MarkdownTagEnd::Item => {
                        self.pop_markdown_list_item(&mut builder);
                    }
                    MarkdownTagEnd::Emphasis => builder.pop_text_style(),
                    MarkdownTagEnd::Strong => builder.pop_text_style(),
                    MarkdownTagEnd::Strikethrough => builder.pop_text_style(),
                    MarkdownTagEnd::Link => {
                        if builder.code_block_stack.is_empty() {
                            builder.link_depth = builder.link_depth.saturating_sub(1);
                            builder.pop_text_style()
                        }
                    }
                    MarkdownTagEnd::Table => {
                        builder.pop_div();
                        builder.pop_div();
                        builder.table.end();
                    }
                    MarkdownTagEnd::TableHead => {
                        builder.pop_text_style();
                        builder.table.end_head();
                    }
                    MarkdownTagEnd::TableRow => {
                        builder.table.end_row();
                    }
                    MarkdownTagEnd::TableCell => {
                        builder.replace_pending_checkbox(self.on_checkbox_toggle.clone());
                        builder.pop_div();
                        builder.pop_div();
                        builder.pop_text_style();
                        builder.table.end_cell();
                    }
                    MarkdownTagEnd::FootnoteDefinition => {
                        builder.pop_div();
                        builder.pop_div();
                    }
                    MarkdownTagEnd::MetadataBlock(_) => {}
                    _ => log::debug!("unsupported markdown tag end: {:?}", tag),
                },
                MarkdownEvent::Text => {
                    builder.push_text(&parsed_markdown.source[range.clone()], range.clone());
                }
                MarkdownEvent::SubstitutedText(text) => {
                    builder.push_text(text, range.clone());
                }
                MarkdownEvent::Code => {
                    self.push_markdown_code_span(
                        &mut builder,
                        &parsed_markdown.source[range.clone()],
                        range.clone(),
                        cx,
                    );
                }
                MarkdownEvent::SubstitutedCode(text) => {
                    self.push_markdown_code_span(&mut builder, text, range.clone(), cx);
                }
                MarkdownEvent::Html => {
                    let html = &parsed_markdown.source[range.clone()];
                    if html.starts_with("<!--") {
                        builder.html_comment = true;
                    }
                    if html.trim_end().ends_with("-->") {
                        builder.html_comment = false;
                        continue;
                    }
                    if builder.html_comment {
                        continue;
                    }
                    builder.push_text(html, range.clone());
                }
                MarkdownEvent::InlineHtml => {
                    let html = &parsed_markdown.source[range.clone()];
                    if let Some(code) = html
                        .strip_prefix("<code>")
                        .and_then(|html| html.strip_suffix("</code>"))
                    {
                        let code_start = range.start + "<code>".len();
                        self.push_markdown_code_span(
                            &mut builder,
                            code,
                            code_start..code_start + code.len(),
                            cx,
                        );
                        continue;
                    }
                    if html.starts_with("<code>") {
                        builder.push_text_style(self.style.inline_code.clone());
                        continue;
                    }
                    if html.trim_end().starts_with("</code>") {
                        builder.pop_text_style();
                        continue;
                    }
                    builder.push_text(&parsed_markdown.source[range.clone()], range.clone());
                }
                MarkdownEvent::Rule => {
                    builder.push_div(
                        div()
                            .border_b_1()
                            .my(self.style.paragraph_spacing)
                            .border_color(self.style.rule_color),
                        range,
                        markdown_end,
                    );
                    builder.pop_div()
                }
                MarkdownEvent::SoftBreak if !self.style.soft_break_as_hard_break => {
                    builder.push_soft_break(range.clone());
                }
                MarkdownEvent::SoftBreak | MarkdownEvent::HardBreak => {
                    builder.push_line_break(range.clone());
                }
                MarkdownEvent::TaskListMarker(_) => {
                    // handled inside the `MarkdownTag::Item` case
                }
                MarkdownEvent::FootnoteReference(label) => {
                    builder.push_footnote_ref(label.clone(), range.clone());
                    builder.push_text_style(self.style.link.clone());
                    builder.push_text(&format!("[{label}]"), range.clone());
                    builder.pop_text_style();
                }
            }
        }
        if self.style.code_block_overflow_x_scroll {
            let code_block_ids = code_block_ids;
            self.markdown.update(cx, move |markdown, _| {
                markdown.retain_code_block_scroll_handles(&code_block_ids);
            });
        } else {
            self.markdown
                .update(cx, |markdown, _| markdown.clear_code_block_scroll_handles());
        }
        let mut rendered_markdown = builder.build();
        #[cfg(test)]
        if let Some(on_render) = self.on_render.as_ref() {
            on_render(rendered_markdown.text.clone());
        }
        let child_layout_id = rendered_markdown.element.request_layout(window, cx);
        let layout_id = window.request_layout(gpui::Style::default(), [child_layout_id], cx);
        (layout_id, rendered_markdown)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        rendered_markdown: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let focus_handle = self.markdown.read(cx).focus_handle.clone();
        window.set_focus_handle(&focus_handle, cx);
        window.set_view_id(self.markdown.entity_id());

        let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
        rendered_markdown.element.prepaint(window, cx);
        self.autoscroll(&rendered_markdown.text, window, cx);
        hitbox
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        rendered_markdown: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut context = KeyContext::default();
        context.add("Markdown");
        window.set_key_context(context);

        let markdown_focus_handle = self.markdown.read(cx).focus_handle.clone();
        let input_focus_handle = if markdown_focus_handle.is_focused(window) {
            Some(markdown_focus_handle)
        } else {
            self.input_focus_handle
                .clone()
                .filter(|focus_handle| focus_handle.is_focused(window))
        };
        if let Some(input_focus_handle) = input_focus_handle {
            window.handle_input(
                &input_focus_handle,
                MarkdownInputHandler::new(self.markdown.clone(), rendered_markdown.text.clone()),
                cx,
            );
        }

        window.on_action(std::any::TypeId::of::<crate::Copy>(), {
            let entity = self.markdown.clone();
            let text = rendered_markdown.text.clone();
            move |_, phase, window, cx| {
                let text = text.clone();
                if phase == DispatchPhase::Bubble {
                    entity.update(cx, move |this, cx| this.copy(&text, window, cx))
                }
            }
        });
        window.on_action(std::any::TypeId::of::<crate::CopyAsMarkdown>(), {
            let entity = self.markdown.clone();
            move |_, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    entity.update(cx, move |this, cx| this.copy_as_markdown(window, cx))
                }
            }
        });

        self.paint_mouse_listeners(hitbox, &rendered_markdown.text, window, cx);
        rendered_markdown.element.paint(window, cx);
    }
}

fn collect_image_alt_text(
    events_from_image_start: &[(Range<usize>, MarkdownEvent)],
    source: &str,
) -> Option<SharedString> {
    let mut alt_text = String::new();
    for (range, event) in events_from_image_start.iter().skip(1) {
        match event {
            MarkdownEvent::End(MarkdownTagEnd::Image) => break,
            MarkdownEvent::Text => alt_text.push_str(&source[range.clone()]),
            _ => {}
        }
    }
    if alt_text.is_empty() {
        None
    } else {
        Some(alt_text.into())
    }
}

fn image_fallback_element(
    dest_url: SharedString,
    alt_text: Option<SharedString>,
    open_image_url_on_click: bool,
) -> AnyElement {
    let link_label = alt_text
        .filter(|alt| !alt.is_empty())
        .unwrap_or_else(|| dest_url.clone());

    let label = format!("Failed to Load: {link_label}");

    div()
        .id("image-fallback")
        .min_w_0()
        .child(Label::new(label).color(Color::Warning).underline())
        .tooltip(Tooltip::text(
            "Image failed to load. Open `zed: log` for more details.",
        ))
        .when(open_image_url_on_click, |this| {
            this.cursor_pointer()
                .on_click(move |_, _, cx| cx.open_url(&dest_url))
        })
        .into_any_element()
}

fn apply_heading_style(
    mut heading: Div,
    level: pulldown_cmark::HeadingLevel,
    custom_styles: Option<&HeadingLevelStyles>,
    border_color: Option<Hsla>,
) -> Div {
    heading = match level {
        pulldown_cmark::HeadingLevel::H1 => heading.text_3xl(),
        pulldown_cmark::HeadingLevel::H2 => heading.text_2xl(),
        pulldown_cmark::HeadingLevel::H3 => heading.text_xl(),
        pulldown_cmark::HeadingLevel::H4 => heading.text_lg(),
        pulldown_cmark::HeadingLevel::H5 => heading.text_base(),
        pulldown_cmark::HeadingLevel::H6 => heading.text_sm(),
    };

    heading = match level {
        pulldown_cmark::HeadingLevel::H1 => heading,
        _ => heading.mt_6(),
    };

    if let Some(border_color) = border_color {
        heading = match level {
            pulldown_cmark::HeadingLevel::H1 => {
                heading.pb_2().border_b_1().border_color(border_color)
            }
            pulldown_cmark::HeadingLevel::H2 => {
                heading.pb_1().border_b_1().border_color(border_color)
            }
            _ => heading,
        };
    }

    if let Some(style) = heading_level_style(level, custom_styles) {
        heading.style().text = style.clone();
    }

    heading
}

fn heading_level_style(
    level: pulldown_cmark::HeadingLevel,
    custom_styles: Option<&HeadingLevelStyles>,
) -> Option<&TextStyleRefinement> {
    let styles = custom_styles?;
    match level {
        pulldown_cmark::HeadingLevel::H1 => styles.h1.as_ref(),
        pulldown_cmark::HeadingLevel::H2 => styles.h2.as_ref(),
        pulldown_cmark::HeadingLevel::H3 => styles.h3.as_ref(),
        pulldown_cmark::HeadingLevel::H4 => styles.h4.as_ref(),
        pulldown_cmark::HeadingLevel::H5 => styles.h5.as_ref(),
        pulldown_cmark::HeadingLevel::H6 => styles.h6.as_ref(),
    }
}

fn render_wrap_code_block_button(
    id: usize,
    is_wrapped: bool,
    markdown: Entity<Markdown>,
) -> impl IntoElement {
    let (icon, tooltip) = if is_wrapped {
        (IconName::TextUnwrap, "Unwrap Content")
    } else {
        (IconName::TextWrap, "Wrap Content")
    };
    let button_id = ElementId::NamedChild(
        Arc::new(ElementId::from(("wrap-code-block", markdown.entity_id()))),
        id.to_string().into(),
    );

    IconButton::new(button_id, icon)
        .icon_size(IconSize::Small)
        .icon_color(Color::Muted)
        .tooltip(Tooltip::text(tooltip))
        .on_click(move |_event, _window, cx| {
            markdown.update(cx, |markdown, cx| {
                markdown.toggle_code_block_wrap(id);
                cx.notify();
            });
        })
}

fn render_copy_code_block_button(
    id: usize,
    code: String,
    markdown: Entity<Markdown>,
) -> impl IntoElement {
    let id = ElementId::NamedChild(
        Arc::new(ElementId::from((
            "copy-markdown-code",
            markdown.entity_id(),
        ))),
        id.to_string().into(),
    );

    CopyButton::new(id.clone(), code.clone()).custom_on_click({
        let markdown = markdown;
        move |_window, cx| {
            let id = id.clone();
            markdown.update(cx, |this, cx| {
                this.copied_code_blocks.insert(id.clone());

                cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));

                cx.spawn(async move |this, cx| {
                    cx.background_executor().timer(Duration::from_secs(2)).await;

                    cx.update(|cx| {
                        this.update(cx, |this, cx| {
                            this.copied_code_blocks.remove(&id);
                            cx.notify();
                        })
                    })
                    .ok();
                })
                .detach();
            });
        }
    })
}

impl IntoElement for MarkdownElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub enum AnyDiv {
    Div(Div),
    Stateful(Stateful<Div>),
}

impl AnyDiv {
    fn into_any_element(self) -> AnyElement {
        match self {
            Self::Div(div) => div.into_any_element(),
            Self::Stateful(div) => div.into_any_element(),
        }
    }
}

impl From<Div> for AnyDiv {
    fn from(value: Div) -> Self {
        Self::Div(value)
    }
}

impl From<Stateful<Div>> for AnyDiv {
    fn from(value: Stateful<Div>) -> Self {
        Self::Stateful(value)
    }
}

impl Styled for AnyDiv {
    fn style(&mut self) -> &mut StyleRefinement {
        match self {
            Self::Div(div) => div.style(),
            Self::Stateful(div) => div.style(),
        }
    }
}

impl ParentElement for AnyDiv {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        match self {
            Self::Div(div) => div.extend(elements),
            Self::Stateful(div) => div.extend(elements),
        }
    }
}

#[derive(Default)]
struct TableState {
    alignments: Vec<Alignment>,
    in_head: bool,
    in_cell: bool,
    row_index: usize,
    col_index: usize,
}

impl TableState {
    fn start(&mut self, alignments: Vec<Alignment>) {
        self.alignments = alignments;
        self.in_head = false;
        self.in_cell = false;
        self.row_index = 0;
        self.col_index = 0;
    }

    fn end(&mut self) {
        self.alignments.clear();
        self.in_head = false;
        self.in_cell = false;
        self.row_index = 0;
        self.col_index = 0;
    }

    fn start_head(&mut self) {
        self.in_head = true;
    }

    fn end_head(&mut self) {
        self.in_head = false;
    }

    fn start_row(&mut self) {
        self.col_index = 0;
    }

    fn end_row(&mut self) {
        self.row_index += 1;
    }

    fn start_cell(&mut self) {
        self.in_cell = true;
    }

    fn end_cell(&mut self) {
        self.in_cell = false;
        self.col_index += 1;
    }

    fn current_cell_alignment(&self) -> Option<Alignment> {
        if self.alignments.is_empty() {
            return None;
        }
        if self.in_head {
            return Some(Alignment::Center);
        }
        self.alignments.get(self.col_index).copied()
    }
}

fn alignment_to_text_align(alignment: Alignment) -> Option<TextAlign> {
    match alignment {
        Alignment::Left => Some(TextAlign::Left),
        Alignment::Center => Some(TextAlign::Center),
        Alignment::Right => Some(TextAlign::Right),
        Alignment::None => None,
    }
}

// The contents of loose list items are wrapped in a paragraph, so their task
// marker follows `Start(Paragraph)` rather than `Start(Item)`.
fn task_list_marker_for_item(
    events: &[(Range<usize>, MarkdownEvent)],
    item_index: usize,
) -> Option<(Range<usize>, bool)> {
    let next_index = item_index.checked_add(1)?;
    let marker_index = match &events.get(next_index)?.1 {
        MarkdownEvent::Start(MarkdownTag::Paragraph) => next_index.checked_add(1)?,
        MarkdownEvent::TaskListMarker(_) => next_index,
        _ => return None,
    };

    match events.get(marker_index)? {
        (range, MarkdownEvent::TaskListMarker(checked)) => Some((range.clone(), *checked)),
        _ => None,
    }
}

struct MetadataCellStyle {
    row_index: usize,
    is_key: bool,
}

struct MarkdownElementBuilder {
    div_stack: Vec<DivStackEntry>,
    rendered_lines: Vec<Rc<RenderedLine>>,
    pending_line: PendingLine,
    rendered_links: Vec<RenderedLink>,
    rendered_image_links: Vec<RenderedImageLink>,
    rendered_footnote_refs: Vec<RenderedFootnoteRef>,
    current_source_index: usize,
    html_comment: bool,
    rendered_footnote_separator: bool,
    base_text_style: TextStyle,
    text_style_stack: Vec<TextStyleRefinement>,
    code_block_stack: Vec<Option<Arc<Language>>>,
    code_block_highlights: Arc<CodeBlockHighlights>,
    link_depth: usize,
    list_stack: Vec<ListStackEntry>,
    table: TableState,
    syntax_theme: Arc<SyntaxTheme>,
    highlights: MarkdownHighlights,
}

struct MarkdownHighlights {
    /// Search highlights, sorted by range start.
    search_highlights: Rc<[Range<usize>]>,
    active_search_highlight: Option<usize>,
    search_match_color: Hsla,
    active_search_match_color: Hsla,
    selection: Option<(Range<usize>, Hsla)>,
    /// Index of the first search highlight that may intersect the next line.
    next_search_highlight_ix: usize,
}

impl MarkdownHighlights {
    /// Returns the highlighted ranges intersecting the given source range,
    /// clamped to it, in paint order.
    fn highlights_for_line(
        &mut self,
        source_range: Range<usize>,
    ) -> SmallVec<[(Range<usize>, Hsla); 1]> {
        let mut highlights = SmallVec::new();

        self.next_search_highlight_ix += self.search_highlights[self.next_search_highlight_ix..]
            .iter()
            .take_while(|range| range.end <= source_range.start)
            .count();

        for (ix, range) in self
            .search_highlights
            .iter()
            .enumerate()
            .skip(self.next_search_highlight_ix)
        {
            if range.start >= source_range.end {
                break;
            }
            let clamped = range.start.max(source_range.start)..range.end.min(source_range.end);
            if clamped.start < clamped.end {
                let color = if Some(ix) == self.active_search_highlight {
                    self.active_search_match_color
                } else {
                    self.search_match_color
                };
                highlights.push((clamped, color));
            }
        }
        if let Some((range, color)) = &self.selection {
            let clamped = range.start.max(source_range.start)..range.end.min(source_range.end);
            if clamped.start < clamped.end {
                highlights.push((clamped, *color));
            }
        }
        highlights
    }
}

struct DivStackEntry {
    div: AnyDiv,
    line_break_mode: LineBreakMode,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum LineBreakMode {
    TextLayout,
    FlexWrap,
}

impl DivStackEntry {
    fn new(div: impl Into<AnyDiv>) -> Self {
        Self {
            div: div.into(),
            line_break_mode: LineBreakMode::TextLayout,
        }
    }
}

#[derive(Default)]
struct PendingLine {
    text: String,
    runs: Vec<TextRun>,
    source_mappings: Vec<SourceMapping>,
    /// Rendered (not source) indices, so chips hug the glyphs and ignore
    /// unrendered characters like the surrounding backticks
    code_chips: Vec<(Range<usize>, Hsla)>,
}

struct ListStackEntry {
    bullet_index: Option<u64>,
}

impl MarkdownElementBuilder {
    fn new(
        container_style: &StyleRefinement,
        base_text_style: TextStyle,
        syntax_theme: Arc<SyntaxTheme>,
        highlights: MarkdownHighlights,
        code_block_highlights: Arc<CodeBlockHighlights>,
    ) -> Self {
        Self {
            div_stack: vec![{
                let mut base_div = div();
                base_div.style().refine(container_style);
                DivStackEntry::new(base_div.debug_selector(|| "inner".into()))
            }],
            rendered_lines: Vec::new(),
            pending_line: PendingLine::default(),
            rendered_links: Vec::new(),
            rendered_image_links: Vec::new(),
            rendered_footnote_refs: Vec::new(),
            current_source_index: 0,
            html_comment: false,
            rendered_footnote_separator: false,
            base_text_style,
            text_style_stack: Vec::new(),
            code_block_stack: Vec::new(),
            code_block_highlights,
            link_depth: 0,
            list_stack: Vec::new(),
            table: TableState::default(),
            syntax_theme,
            highlights,
        }
    }

    fn push_code_chip_text(
        &mut self,
        text: &str,
        source_range: Range<usize>,
        chip_background: Option<Hsla>,
    ) {
        let chip_start = self.pending_line.text.len();
        self.push_text(text, source_range);
        if let Some(background) = chip_background {
            let chip_end = self.pending_line.text.len();
            if chip_start < chip_end {
                self.pending_line
                    .code_chips
                    .push((chip_start..chip_end, background));
            }
        }
    }

    fn push_text_style(&mut self, style: TextStyleRefinement) {
        self.text_style_stack.push(style);
    }

    fn text_style(&self) -> TextStyle {
        let mut style = self.base_text_style.clone();
        for refinement in &self.text_style_stack {
            style.refine(refinement);
        }
        style
    }

    fn pop_text_style(&mut self) {
        self.text_style_stack.pop();
    }

    fn push_div(&mut self, div: impl Into<AnyDiv>, range: &Range<usize>, markdown_end: usize) {
        let mut div = div.into();
        self.flush_text();

        if range.start == 0 {
            // Remove the top margin on the first element.
            div.style().refine(&StyleRefinement {
                margin: gpui::EdgesRefinement {
                    top: Some(Length::Definite(px(0.).into())),
                    left: None,
                    right: None,
                    bottom: None,
                },
                ..Default::default()
            });
        }

        if range.end == markdown_end {
            div.style().refine(&StyleRefinement {
                margin: gpui::EdgesRefinement {
                    top: None,
                    left: None,
                    right: None,
                    bottom: Some(Length::Definite(rems(0.).into())),
                },
                ..Default::default()
            });
        }

        self.div_stack.push(DivStackEntry::new(div));
    }

    fn push_root_block(&mut self, range: &Range<usize>, markdown_end: usize) {
        self.push_div(
            div().group("markdown-root-block").relative(),
            range,
            markdown_end,
        );
        self.push_div(div().pl_4(), range, markdown_end);
    }

    fn push_image_child(&mut self, child: impl IntoElement) {
        let table_cell_alignment = self
            .table
            .in_cell
            .then(|| self.table.current_cell_alignment());
        self.modify_current_div(|el| {
            let el = el.flex().flex_row().flex_wrap();
            // Table cells center their content vertically and apply column alignment via a
            // column-direction container. Switching it to a row moves those axes, so the
            // alignment has to be restated for the row.
            match table_cell_alignment {
                Some(alignment) => {
                    let el = el.items_center().content_center();
                    match alignment {
                        Some(Alignment::Center) => el.justify_center(),
                        Some(Alignment::Right) => el.justify_end(),
                        _ => el.justify_start(),
                    }
                }
                None => el.items_start(),
            }
        });
        self.div_stack.last_mut().unwrap().line_break_mode = LineBreakMode::FlexWrap;
        self.append_child(child.into_any_element());
    }

    fn push_line_break(&mut self, source_range: Range<usize>) {
        if self.uses_flex_line_breaks() {
            self.modify_current_div(|el| el.child(div().w_full().h_0()));
        } else {
            self.push_text("\n", source_range);
        }
    }

    fn push_soft_break(&mut self, source_range: Range<usize>) {
        // A soft break right after an item in flex wrap container would otherwise
        // render as a stray leading space before the next wrapped item.
        if self.uses_flex_line_breaks() && self.pending_line.text.is_empty() {
            return;
        }
        self.push_text(" ", source_range);
    }

    fn append_child(&mut self, child: AnyElement) {
        self.div_stack.last_mut().unwrap().div.extend([child]);
    }

    fn uses_flex_line_breaks(&self) -> bool {
        self.div_stack
            .last()
            .is_some_and(|entry| entry.line_break_mode == LineBreakMode::FlexWrap)
    }

    fn modify_current_div(&mut self, f: impl FnOnce(AnyDiv) -> AnyDiv) {
        self.flush_text();
        if let Some(mut entry) = self.div_stack.pop() {
            entry.div = f(entry.div);
            self.div_stack.push(entry);
        }
    }

    fn pop_root_block(
        &mut self,
        is_active: bool,
        active_gutter_color: Hsla,
        hovered_gutter_color: Hsla,
    ) {
        self.pop_div();
        self.modify_current_div(|el| {
            el.child(
                div()
                    .h_full()
                    .w(px(4.0))
                    .when(is_active, |this| this.bg(active_gutter_color))
                    .group_hover("markdown-root-block", |this| {
                        if is_active {
                            this
                        } else {
                            this.bg(hovered_gutter_color)
                        }
                    })
                    .rounded_xs()
                    .absolute()
                    .left_0()
                    .top_0(),
            )
        });
        self.pop_div();
    }

    fn pop_div(&mut self) {
        self.flush_text();
        let div = self.div_stack.pop().unwrap().div.into_any_element();
        self.append_child(div);
    }

    fn push_list(&mut self, bullet_index: Option<u64>) {
        self.list_stack.push(ListStackEntry { bullet_index });
    }

    fn next_bullet_index(&mut self) -> Option<u64> {
        self.list_stack.last_mut().and_then(|entry| {
            let item_index = entry.bullet_index.as_mut()?;
            *item_index += 1;
            Some(*item_index - 1)
        })
    }

    fn pop_list(&mut self) {
        self.list_stack.pop();
    }

    fn push_sourced_element(&mut self, source_range: Range<usize>, element: impl Into<AnyElement>) {
        self.flush_text();
        let anchor = self.render_source_anchor(source_range);
        self.append_child(
            div()
                .relative()
                .child(anchor)
                .child(element.into())
                .into_any_element(),
        );
    }

    fn push_code_block(&mut self, language: Option<Arc<Language>>) {
        self.code_block_stack.push(language);
    }

    fn pop_code_block(&mut self) {
        self.code_block_stack.pop();
    }

    fn push_link(&mut self, destination_url: SharedString, source_range: Range<usize>) {
        self.rendered_links.push(RenderedLink {
            source_range,
            destination_url,
        });
    }

    fn push_image_link(
        &mut self,
        destination_url: SharedString,
        bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    ) {
        self.rendered_image_links.push(RenderedImageLink {
            bounds,
            destination_url,
        });
    }

    fn push_footnote_ref(&mut self, label: SharedString, source_range: Range<usize>) {
        self.rendered_footnote_refs.push(RenderedFootnoteRef {
            source_range,
            label,
        });
    }

    fn push_text(&mut self, text: &str, source_range: Range<usize>) {
        self.pending_line.source_mappings.push(SourceMapping {
            rendered_index: self.pending_line.text.len(),
            source_index: source_range.start,
        });
        self.pending_line.text.push_str(text);
        self.current_source_index = source_range.end;

        // Compute the base text style once
        let text_style = self.text_style();

        if let Some(language) = self.code_block_stack.last().and_then(Option::as_ref)
            && let Some(resolved) = self.code_block_highlights.get(&source_range.start)
        {
            let runs = if resolved.is_current() {
                resolved.runs.clone()
            } else {
                language
                    .highlight_text_resolved(&Rope::from(text), 0..text.len())
                    .runs
            };
            let mut offset = 0;
            for (run_range, highlight_id) in runs.iter() {
                if run_range.start > offset {
                    self.pending_line
                        .runs
                        .push(text_style.to_run(run_range.start - offset));
                }

                let run_len = run_range.len();
                let highlight = self.syntax_theme.get(*highlight_id).cloned();
                if let Some(highlight) = highlight {
                    self.pending_line
                        .runs
                        .push(text_style.clone().highlight(highlight).to_run(run_len));
                } else {
                    self.pending_line.runs.push(text_style.to_run(run_len));
                }
                offset = run_range.end;
            }

            if offset < text.len() {
                self.pending_line
                    .runs
                    .push(text_style.to_run(text.len() - offset));
            }
        } else {
            self.pending_line.runs.push(text_style.to_run(text.len()));
        }
    }

    fn trim_trailing_newline(&mut self) {
        if self.pending_line.text.ends_with('\n') {
            self.pending_line
                .text
                .truncate(self.pending_line.text.len() - 1);
            self.pending_line.runs.last_mut().unwrap().len -= 1;
            self.current_source_index -= 1;
        }
    }

    fn replace_pending_checkbox(&mut self, on_toggle: Option<CheckboxToggleCallback>) {
        let text = &self.pending_line.text;
        let trimmed = text.trim();
        if trimmed != "[x]" && trimmed != "[X]" && trimmed != "[ ]" {
            return;
        }
        let checked = trimmed != "[ ]";

        let leading_ws = text.len() - text.trim_start().len();
        let marker_rendered = leading_ws..leading_ws + trimmed.len();
        let marker_source = self
            .source_range_for_rendered(&marker_rendered)
            .expect("pending checkbox text must have source mappings");

        self.pending_line = PendingLine::default();

        let toggle_state = if checked {
            ToggleState::Selected
        } else {
            ToggleState::Unselected
        };
        let checkbox = Checkbox::new(
            ElementId::Name(
                format!(
                    "table_checkbox_{}_{}",
                    marker_source.start, marker_source.end
                )
                .into(),
            ),
            toggle_state,
        )
        .fill();

        let checkbox = if let Some(on_toggle) = on_toggle {
            checkbox
                .on_click(move |_state, window, cx| {
                    on_toggle(marker_source.clone(), !checked, window, cx);
                })
                .into_any_element()
        } else {
            checkbox.visualization_only(true).into_any_element()
        };

        let mut checkbox_container = h_flex().w_full();
        checkbox_container = match self.text_style().text_align {
            TextAlign::Left => checkbox_container.justify_start(),
            TextAlign::Center => checkbox_container.justify_center(),
            TextAlign::Right => checkbox_container.justify_end(),
        };

        self.append_child(checkbox_container.child(checkbox).into_any_element());
    }

    fn source_range_for_rendered(&self, rendered: &Range<usize>) -> Option<Range<usize>> {
        source_range_for_rendered(&self.pending_line.source_mappings, rendered)
    }

    fn render_source_anchor(&mut self, source_range: Range<usize>) -> AnyElement {
        let mut text_style = self.base_text_style.clone();
        text_style.color = Hsla::transparent_black();
        let text = "\u{200B}";
        let styled_text = StyledText::new(text).with_runs(vec![text_style.to_run(text.len())]);
        self.rendered_lines.push(Rc::new(RenderedLine {
            layout: styled_text.layout().clone(),
            visible_bounds: Cell::new(None),
            source_mappings: vec![SourceMapping {
                rendered_index: 0,
                source_index: source_range.start,
            }],
            source_end: source_range.end,
            language: None,
            text_align: TextAlign::Left,
            highlights: SmallVec::new(),
            code_chips: SmallVec::new(),
        }));
        div()
            .absolute()
            .top_0()
            .left_0()
            .opacity(0.)
            .child(styled_text)
            .into_any_element()
    }

    fn flush_text(&mut self) {
        let text_align = self.text_style().text_align;
        let line = mem::take(&mut self.pending_line);
        if line.text.is_empty() {
            return;
        }

        let highlights = line
            .source_mappings
            .first()
            .map(|first_mapping| {
                self.highlights
                    .highlights_for_line(first_mapping.source_index..self.current_source_index)
            })
            .unwrap_or_default();
        let text = StyledText::new(line.text).with_runs(line.runs);
        let rendered_line = Rc::new(RenderedLine {
            layout: text.layout().clone(),
            visible_bounds: Cell::new(None),
            source_mappings: line.source_mappings,
            source_end: self.current_source_index,
            language: self
                .code_block_stack
                .last()
                .and_then(|entry| entry.as_ref())
                .cloned(),
            text_align,
            highlights,
            code_chips: line.code_chips.into_iter().collect(),
        });
        self.rendered_lines.push(rendered_line.clone());
        self.append_child(
            RenderedLineElement {
                text,
                line: rendered_line,
            }
            .into_any_element(),
        );
    }

    fn build(mut self) -> RenderedMarkdown {
        debug_assert_eq!(self.div_stack.len(), 1);
        self.flush_text();
        RenderedMarkdown {
            element: self.div_stack.pop().unwrap().div.into_any_element(),
            text: RenderedText {
                lines: self.rendered_lines.into(),
                links: self.rendered_links.into(),
                image_links: self.rendered_image_links.into(),
                footnote_refs: self.rendered_footnote_refs.into(),
            },
        }
    }
}

/// Wraps a rendered line so its code chips and highlights share the glyphs'
/// ancestor content masks, and records the clipped bounds for platform text hit testing.
struct RenderedLineElement {
    text: StyledText,
    line: Rc<RenderedLine>,
}

impl Element for RenderedLineElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (gpui::LayoutId, Self::RequestLayoutState) {
        self.text.request_layout(None, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.line
            .visible_bounds
            .set(Some(bounds.intersect(&window.content_mask().bounds)));
        self.text
            .prepaint(None, inspector_id, bounds, request_layout, window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let layout = self.text.layout();
        self.line.paint_code_chips(window);
        layout.paint_background(window, cx).log_err();
        self.line.paint_highlights(window);
        layout.paint_foreground(window, cx).log_err();
    }
}

impl IntoElement for RenderedLineElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct RenderedLine {
    layout: TextLayout,
    visible_bounds: Cell<Option<Bounds<Pixels>>>,
    source_mappings: Vec<SourceMapping>,
    source_end: usize,
    language: Option<Arc<Language>>,
    text_align: TextAlign,
    /// Highlighted source ranges intersecting this line, in paint order
    highlights: SmallVec<[(Range<usize>, Hsla); 1]>,
    /// Inline code chip ranges intersecting this line, in rendered indices
    code_chips: SmallVec<[(Range<usize>, Hsla); 1]>,
}

impl RenderedLine {
    /// Painted before the glyphs so the text renders on top of the chips
    fn paint_code_chips(&self, window: &mut Window) {
        const CHIP_CORNER_RADIUS: Pixels = px(4.);

        if self.code_chips.is_empty() {
            return;
        }
        let wrapped_line_segments = self.wrapped_line_segments();
        if wrapped_line_segments.is_empty() {
            return;
        }

        for (rendered_range, color) in &self.code_chips {
            self.for_each_bounds_in_rendered_range(
                &wrapped_line_segments,
                rendered_range.clone(),
                |bounds| {
                    // Kept to a hair since the layout reserves no padding and
                    // anything wider eats the gap to neighboring words
                    let horizontal_outset = px(1.);
                    // Inset vertically so the chip hugs the glyphs like a badge
                    // instead of filling the whole line box
                    let vertical_inset = bounds.size.height * 0.1;
                    let chip_bounds = Bounds {
                        origin: point(
                            bounds.origin.x - horizontal_outset,
                            bounds.origin.y + vertical_inset,
                        ),
                        size: size(
                            bounds.size.width + horizontal_outset * 2.,
                            bounds.size.height - vertical_inset * 2.,
                        ),
                    };
                    window.paint_quad(quad(
                        chip_bounds,
                        CHIP_CORNER_RADIUS,
                        *color,
                        Edges::default(),
                        Hsla::transparent_black(),
                        BorderStyle::default(),
                    ));
                },
            );
        }
    }

    /// Painted between the text run backgrounds and the glyphs, so opaque highlight
    /// colors neither hide the text nor get hidden by run backgrounds
    fn paint_highlights(&self, window: &mut Window) {
        if self.highlights.is_empty() {
            return;
        }
        let wrapped_line_segments = self.wrapped_line_segments();
        if wrapped_line_segments.is_empty() {
            return;
        }

        for (source_range, color) in &self.highlights {
            self.for_each_bounds_in_source_range(
                &wrapped_line_segments,
                source_range.clone(),
                |bounds| {
                    window.paint_quad(quad(
                        bounds,
                        Pixels::ZERO,
                        *color,
                        Edges::default(),
                        Hsla::transparent_black(),
                        BorderStyle::default(),
                    ));
                },
            );
        }
    }

    fn wrapped_line_segments(&self) -> SmallVec<[WrappedLineSegment; 1]> {
        let layout = &self.layout;
        let line_layouts = layout.line_layouts();
        let line_height = layout.line_height();
        let mut row_top = layout.bounds().top();
        let mut wrapped_line_start = 0;
        let mut segments = SmallVec::with_capacity(line_layouts.len());

        for wrapped_line in line_layouts {
            let wrapped_line_end = wrapped_line_start + wrapped_line.len();
            let wrapped_line_height = wrapped_line.size(line_height).height;
            segments.push(WrappedLineSegment {
                start: wrapped_line_start,
                end: wrapped_line_end,
                row_top,
                layout: wrapped_line,
            });
            row_top += wrapped_line_height;
            wrapped_line_start = wrapped_line_end + 1;
        }

        segments
    }

    fn for_each_bounds_in_source_range(
        &self,
        wrapped_line_segments: &[WrappedLineSegment],
        range: Range<usize>,
        f: impl FnMut(Bounds<Pixels>),
    ) {
        if range.start >= range.end {
            return;
        }

        let rendered_start = self.rendered_index_for_source_index(range.start);
        let rendered_end = self.rendered_index_for_source_index(range.end);
        self.for_each_bounds_in_rendered_range(
            wrapped_line_segments,
            rendered_start..rendered_end,
            f,
        );
    }

    fn for_each_bounds_in_rendered_range(
        &self,
        wrapped_line_segments: &[WrappedLineSegment],
        rendered_range: Range<usize>,
        mut f: impl FnMut(Bounds<Pixels>),
    ) {
        let layout = &self.layout;
        let line_bounds = layout.bounds();
        let line_height = layout.line_height();

        let rendered_start = rendered_range.start;
        let rendered_end = rendered_range.end;

        for wrapped_line_segment in wrapped_line_segments {
            if wrapped_line_segment.start >= rendered_end {
                break;
            }
            if wrapped_line_segment.end <= rendered_start {
                continue;
            }

            let wrapped_line = &wrapped_line_segment.layout;
            let unwrapped_layout = &wrapped_line.unwrapped_layout;
            let wrapped_line_start = wrapped_line_segment.start;
            let wrapped_line_end = wrapped_line_segment.end;
            let mut row_top = wrapped_line_segment.row_top;

            let row_ends = wrapped_line
                .wrap_boundaries()
                .iter()
                .map(|wrap_boundary| {
                    let glyph =
                        &unwrapped_layout.runs[wrap_boundary.run_ix].glyphs[wrap_boundary.glyph_ix];
                    (wrapped_line_start + glyph.index, glyph.position.x)
                })
                .chain([(wrapped_line_end, unwrapped_layout.width)]);

            let mut row_start = wrapped_line_start;
            let mut row_start_x = Pixels::ZERO;

            for (row_end, row_end_x) in row_ends {
                let selection_start = rendered_start.max(row_start);
                let selection_end = rendered_end.min(row_end);

                if selection_start < selection_end {
                    let alignment_offset = self.alignment_offset_for_segment(
                        line_bounds.size.width,
                        row_start_x,
                        row_end_x,
                    );
                    let x_for_index = |index| {
                        line_bounds.left()
                            + alignment_offset
                            + unwrapped_layout.x_for_index(index - wrapped_line_start)
                            - row_start_x
                    };
                    f(Bounds::from_corners(
                        point(x_for_index(selection_start), row_top),
                        point(x_for_index(selection_end), row_top + line_height),
                    ));
                }

                row_start = row_end;
                row_start_x = row_end_x;
                row_top += line_height;
            }
        }
    }

    fn rendered_index_for_source_index(&self, source_index: usize) -> usize {
        if source_index >= self.source_end {
            return self.layout.len();
        }

        let mapping = match self
            .source_mappings
            .binary_search_by_key(&source_index, |probe| probe.source_index)
        {
            Ok(ix) => &self.source_mappings[ix],
            Err(ix) => &self.source_mappings[ix - 1],
        };
        (mapping.rendered_index + (source_index - mapping.source_index)).min(self.layout.len())
    }

    fn source_index_for_rendered_index(&self, rendered_index: usize) -> usize {
        if rendered_index >= self.layout.len() {
            return self.source_end;
        }

        let mapping = match self
            .source_mappings
            .binary_search_by_key(&rendered_index, |probe| probe.rendered_index)
        {
            Ok(ix) => &self.source_mappings[ix],
            Err(ix) => &self.source_mappings[ix - 1],
        };
        mapping.source_index + (rendered_index - mapping.rendered_index)
    }

    /// Returns the source index for use as an exclusive range end at a word/selection boundary.
    /// When the rendered index is exactly at the start of a segment with a gap from the previous
    /// segment (e.g., after stripped markdown syntax like backticks), this returns the end of the
    /// previous segment rather than the start of the current one.
    fn source_index_for_exclusive_rendered_end(&self, rendered_index: usize) -> usize {
        if rendered_index >= self.layout.len() {
            return self.source_end;
        }

        let ix = match self
            .source_mappings
            .binary_search_by_key(&rendered_index, |probe| probe.rendered_index)
        {
            Ok(ix) => ix,
            Err(ix) => {
                return self.source_mappings[ix - 1].source_index
                    + (rendered_index - self.source_mappings[ix - 1].rendered_index);
            }
        };

        // Exact match at the start of a segment. Check if there's a gap from the previous segment.
        if ix > 0 {
            let prev_mapping = &self.source_mappings[ix - 1];
            let mapping = &self.source_mappings[ix];
            let prev_segment_len = mapping.rendered_index - prev_mapping.rendered_index;
            let prev_source_end = prev_mapping.source_index + prev_segment_len;
            if prev_source_end < mapping.source_index {
                return prev_source_end;
            }
        }

        self.source_mappings[ix].source_index
    }

    fn alignment_offset_for_segment(
        &self,
        available_width: Pixels,
        segment_start_x: Pixels,
        segment_end_x: Pixels,
    ) -> Pixels {
        let segment_width = segment_end_x - segment_start_x;
        match self.text_align {
            TextAlign::Left => px(0.),
            TextAlign::Center => ((available_width - segment_width) / 2.).max(px(0.)),
            TextAlign::Right => (available_width - segment_width).max(px(0.)),
        }
    }

    fn source_index_for_position(&self, position: Point<Pixels>) -> Result<usize, usize> {
        let adjusted_position = maybe!({
            if self.text_align == TextAlign::Left {
                return None;
            }

            let Some(wrapped_line) = self.layout.line_layout_for_index(0) else {
                return None;
            };

            let bounds = self.layout.bounds();
            let line_height = self.layout.line_height();
            let relative_y = (position.y - bounds.top()).max(px(0.));
            let wrapped_row_ix = (relative_y / line_height) as usize;
            let boundaries = wrapped_line.wrap_boundaries();

            let segment_start_x = if wrapped_row_ix == 0 {
                px(0.)
            } else {
                boundaries
                    .get(wrapped_row_ix - 1)
                    .map(|b| {
                        wrapped_line.unwrapped_layout.runs[b.run_ix].glyphs[b.glyph_ix]
                            .position
                            .x
                    })
                    .unwrap_or(px(0.))
            };
            let segment_end_x = boundaries
                .get(wrapped_row_ix)
                .map(|b| {
                    wrapped_line.unwrapped_layout.runs[b.run_ix].glyphs[b.glyph_ix]
                        .position
                        .x
                })
                .unwrap_or(wrapped_line.unwrapped_layout.width);

            let alignment_offset = self.alignment_offset_for_segment(
                bounds.size.width,
                segment_start_x,
                segment_end_x,
            );
            Some(point(position.x - alignment_offset, position.y))
        })
        .unwrap_or(position);

        let line_rendered_index;
        let out_of_bounds;
        match self.layout.index_for_position(adjusted_position) {
            Ok(ix) => {
                line_rendered_index = ix;
                out_of_bounds = false;
            }
            Err(ix) => {
                line_rendered_index = ix;
                out_of_bounds = true;
            }
        };
        let source_index = self.source_index_for_rendered_index(line_rendered_index);
        if out_of_bounds {
            Err(source_index)
        } else {
            Ok(source_index)
        }
    }
}

#[derive(Copy, Clone, Debug, Default)]
struct SourceMapping {
    rendered_index: usize,
    source_index: usize,
}

fn source_range_for_rendered(
    mappings: &[SourceMapping],
    rendered: &Range<usize>,
) -> Option<Range<usize>> {
    if rendered.start >= rendered.end {
        return None;
    }
    let start = source_index_for_rendered(mappings, rendered.start)?;
    let end = source_index_for_rendered(mappings, rendered.end - 1)? + 1;
    Some(start..end)
}

fn source_index_for_rendered(mappings: &[SourceMapping], rendered_index: usize) -> Option<usize> {
    let mut last: Option<&SourceMapping> = None;
    for mapping in mappings {
        if mapping.rendered_index <= rendered_index {
            last = Some(mapping);
        } else {
            break;
        }
    }
    last.map(|m| m.source_index + (rendered_index - m.rendered_index))
}

pub struct RenderedMarkdown {
    element: AnyElement,
    text: RenderedText,
}

#[derive(Clone)]
struct RenderedText {
    lines: Rc<[Rc<RenderedLine>]>,
    links: Rc<[RenderedLink]>,
    image_links: Rc<[RenderedImageLink]>,
    footnote_refs: Rc<[RenderedFootnoteRef]>,
}

struct WrappedLineSegment {
    start: usize,
    end: usize,
    row_top: Pixels,
    layout: Arc<WrappedLineLayout>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RenderedLink {
    source_range: Range<usize>,
    destination_url: SharedString,
}

#[derive(Clone)]
struct RenderedImageLink {
    // Populated once the image's `canvas` overlay is painted; images aren't part of the
    // text layout, so their hit-test region can't be derived from a source range.
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    destination_url: SharedString,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct RenderedFootnoteRef {
    source_range: Range<usize>,
    label: SharedString,
}

impl RenderedText {
    fn bounds_for_source_range(&self, range: Range<usize>) -> Vec<Bounds<Pixels>> {
        let mut all_bounds = Vec::new();
        for line in self.lines.iter() {
            let Some(first_mapping) = line.source_mappings.first() else {
                continue;
            };
            let line_source_start = first_mapping.source_index;
            if range.end <= line_source_start {
                break;
            }
            if range.start >= line.source_end {
                continue;
            }
            let wrapped_line_segments = line.wrapped_line_segments();
            line.for_each_bounds_in_source_range(
                &wrapped_line_segments,
                range.start.max(line_source_start)..range.end.min(line.source_end),
                |bounds| all_bounds.push(bounds),
            );
        }
        all_bounds
    }

    fn utf16_index_for_source_index(&self, source_index: usize) -> usize {
        self.text_for_range(0..source_index).encode_utf16().count()
    }

    fn utf16_range_for_source_range(&self, range: Range<usize>) -> Range<usize> {
        self.utf16_index_for_source_index(range.start)..self.utf16_index_for_source_index(range.end)
    }

    fn source_index_for_utf16_index(&self, utf16_index: usize) -> usize {
        let mut line_start_utf16 = 0;
        let mut last_source_end = 0;

        for (line_index, line) in self.lines.iter().enumerate() {
            let line_text = line.layout.text();
            let line_utf16_len = line_text.encode_utf16().count();
            let line_end_utf16 = line_start_utf16 + line_utf16_len;
            last_source_end = line.source_end;

            if utf16_index <= line_end_utf16 {
                let local_utf16_index = utf16_index.saturating_sub(line_start_utf16);
                let rendered_text = Rope::from(line_text.as_str());
                let rendered_index = rendered_text.offset_utf16_to_offset(
                    rendered_text.clip_offset_utf16(OffsetUtf16(local_utf16_index), Bias::Left),
                );
                return line.source_index_for_rendered_index(rendered_index);
            }

            line_start_utf16 = line_end_utf16;
            if line_index + 1 < self.lines.len() {
                line_start_utf16 += 1;
            }
        }

        last_source_end
    }

    fn source_range_for_utf16_range(&self, range_utf16: Range<usize>) -> Range<usize> {
        self.source_index_for_utf16_index(range_utf16.start)
            ..self.source_index_for_utf16_index(range_utf16.end)
    }

    fn text_for_utf16_range(
        &self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
    ) -> String {
        let mut rendered_text = String::new();
        for (line_index, line) in self.lines.iter().enumerate() {
            if line_index > 0 {
                rendered_text.push('\n');
            }
            rendered_text.push_str(&line.layout.text());
        }

        let rendered_text = Rope::from(rendered_text);
        let start = rendered_text.clip_offset_utf16(OffsetUtf16(range_utf16.start), Bias::Left);
        let end = rendered_text.clip_offset_utf16(OffsetUtf16(range_utf16.end), Bias::Right);

        if (start.0..end.0) != range_utf16 {
            adjusted_range.replace(start.0..end.0);
        }

        rendered_text
            .chunks_in_range(
                rendered_text.offset_utf16_to_offset(start)
                    ..rendered_text.offset_utf16_to_offset(end),
            )
            .collect()
    }

    fn source_index_for_position(&self, position: Point<Pixels>) -> Result<usize, usize> {
        let mut lines = self.lines.iter().peekable();
        let mut fallback_line: Option<&Rc<RenderedLine>> = None;

        while let Some(line) = lines.next() {
            let line_bounds = line.layout.bounds();

            // Exact match: position is within bounds (handles overlapping bounds like table columns)
            if line_bounds.contains(&position) {
                return line.source_index_for_position(position);
            }

            // Track fallback for Y-coordinate based matching
            if position.y <= line_bounds.bottom() && fallback_line.is_none() {
                fallback_line = Some(line);
            }

            // Handle gap between lines
            if position.y > line_bounds.bottom() {
                if let Some(next_line) = lines.peek()
                    && position.y < next_line.layout.bounds().top()
                {
                    return Err(line.source_end);
                }
            }
        }

        // Fall back to Y-coordinate matched line
        if let Some(line) = fallback_line {
            return line.source_index_for_position(position);
        }

        Err(self.lines.last().map_or(0, |line| line.source_end))
    }

    fn source_index_for_visible_position(&self, position: Point<Pixels>) -> Option<usize> {
        self.lines.iter().find_map(|line| {
            if line.visible_bounds.get()?.contains(&position) {
                line.source_index_for_position(position).ok()
            } else {
                None
            }
        })
    }

    fn position_for_source_index(&self, source_index: usize) -> Option<(Point<Pixels>, Pixels)> {
        for line in self.lines.iter() {
            if source_index > line.source_end {
                continue;
            }
            let line_source_start = line.source_mappings.first().unwrap().source_index;
            let source_index = source_index.max(line_source_start);
            let line_height = line.layout.line_height();
            let rendered_index_within_line = line.rendered_index_for_source_index(source_index);
            let position = line.layout.position_for_index(rendered_index_within_line)?;
            return Some((position, line_height));
        }
        None
    }

    fn surrounding_word_range(&self, source_index: usize) -> Range<usize> {
        for line in self.lines.iter() {
            if source_index > line.source_end {
                continue;
            }

            let line_rendered_start = line.source_mappings.first().unwrap().rendered_index;
            let rendered_index_in_line =
                line.rendered_index_for_source_index(source_index) - line_rendered_start;
            let text = line.layout.text();

            let scope = line.language.as_ref().map(|l| l.default_scope());
            let classifier = CharClassifier::new(scope);

            let mut prev_chars = text[..rendered_index_in_line].chars().rev().peekable();
            let mut next_chars = text[rendered_index_in_line..].chars().peekable();

            let word_kind = std::cmp::max(
                prev_chars.peek().map(|&c| classifier.kind(c)),
                next_chars.peek().map(|&c| classifier.kind(c)),
            );

            let mut start = rendered_index_in_line;
            for c in prev_chars {
                if Some(classifier.kind(c)) == word_kind {
                    start -= c.len_utf8();
                } else {
                    break;
                }
            }

            let mut end = rendered_index_in_line;
            for c in next_chars {
                if Some(classifier.kind(c)) == word_kind {
                    end += c.len_utf8();
                } else {
                    break;
                }
            }

            return line.source_index_for_rendered_index(line_rendered_start + start)
                ..line.source_index_for_exclusive_rendered_end(line_rendered_start + end);
        }

        source_index..source_index
    }

    fn surrounding_line_range(&self, source_index: usize) -> Range<usize> {
        for line in self.lines.iter() {
            if source_index > line.source_end {
                continue;
            }
            let line_source_start = line.source_mappings.first().unwrap().source_index;
            return line_source_start..line.source_end;
        }

        source_index..source_index
    }

    fn text_for_range(&self, range: Range<usize>) -> String {
        let mut accumulator = String::new();

        for line in self.lines.iter() {
            if range.start > line.source_end {
                continue;
            }
            let line_source_start = line.source_mappings.first().unwrap().source_index;
            if range.end < line_source_start {
                break;
            }

            let text = line.layout.text();

            let start = if range.start < line_source_start {
                0
            } else {
                line.rendered_index_for_source_index(range.start)
            };
            let end = if range.end > line.source_end {
                line.rendered_index_for_source_index(line.source_end)
            } else {
                line.rendered_index_for_source_index(range.end)
            }
            .min(text.len());

            accumulator.push_str(&text[start..end]);
            accumulator.push('\n');
        }
        // Remove trailing newline
        accumulator.pop();
        accumulator
    }

    fn link_for_source_index(&self, source_index: usize) -> Option<&RenderedLink> {
        self.links
            .iter()
            .find(|link| link.source_range.contains(&source_index))
    }

    fn image_link_for_position(&self, position: Point<Pixels>) -> Option<&RenderedImageLink> {
        self.image_links.iter().find(|image| {
            image
                .bounds
                .get()
                .is_some_and(|bounds| bounds.contains(&position))
        })
    }

    fn footnote_ref_for_source_index(&self, source_index: usize) -> Option<&RenderedFootnoteRef> {
        self.footnote_refs
            .iter()
            .find(|fref| fref.source_range.contains(&source_index))
    }
}

struct MarkdownInputHandler {
    markdown: Entity<Markdown>,
    rendered_text: RenderedText,
}

impl MarkdownInputHandler {
    fn new(markdown: Entity<Markdown>, rendered_text: RenderedText) -> Self {
        Self {
            markdown,
            rendered_text,
        }
    }
}

impl InputHandler for MarkdownInputHandler {
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut App,
    ) -> Option<UTF16Selection> {
        self.markdown.update(cx, |markdown, _cx| {
            let range = markdown.selection.start..markdown.selection.end;
            Some(UTF16Selection {
                range: self.rendered_text.utf16_range_for_source_range(range),
                reversed: markdown.selection.reversed,
            })
        })
    }

    fn marked_text_range(&mut self, _: &mut Window, _: &mut App) -> Option<Range<usize>> {
        None
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<String> {
        if range_utf16.start > range_utf16.end {
            return None;
        }

        Some(
            self.rendered_text
                .text_for_utf16_range(range_utf16, adjusted_range),
        )
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        _: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut App) {}

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Bounds<Pixels>> {
        let source_range = self.rendered_text.source_range_for_utf16_range(range_utf16);
        self.rendered_text
            .bounds_for_source_range(source_range.clone())
            .into_iter()
            .next()
            .or_else(|| {
                self.rendered_text
                    .position_for_source_index(source_range.start)
                    .map(|(position, line_height)| Bounds {
                        origin: position,
                        size: size(px(0.), line_height),
                    })
            })
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _: &mut Window,
        _: &mut App,
    ) -> Option<usize> {
        let source_index = self
            .rendered_text
            .source_index_for_visible_position(point)?;
        Some(
            self.rendered_text
                .utf16_index_for_source_index(source_index),
        )
    }

    fn accepts_text_input(&mut self, _: &mut Window, _: &mut App) -> bool {
        false
    }
}
