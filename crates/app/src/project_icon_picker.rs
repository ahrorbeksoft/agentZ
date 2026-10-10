//! t3code's project icon dialogs, opened from a project's settings. Choose icon picks one of
//! the app's icons in a color (Zed's icons, where t3code has Lucide's), an emoji, or a
//! monogram. Choose file picks an image file in the project, listed by its machine's server,
//! or, on this Mac, any file the system's file picker opens.

use std::path::PathBuf;
use std::rc::Rc;

use agentz_protocol::{Request, Response};
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding,
    ScrollHandle, Stateful, Subscription, Task, Window,
};
use projects::{ProjectIcon, ProjectId};
use strum::IntoEnumIterator as _;
use text_input::{TextInput, TextInputEvent};
use ui::{
    HighlightedLabel, ListItem, ListItemSpacing, ListSubHeader, ToggleButtonGroup,
    ToggleButtonGroupStyle, ToggleButtonSimple, Tooltip, WithScrollbar as _, prelude::*,
};
use unicode_properties::UnicodeEmoji as _;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::controls::{
    ActionButton, ActionStyle, dialog_frame, dialog_title, field_label, key_hint, text_field,
};
use crate::project_info::{
    MONOGRAM_COLORS, automatic_monogram, monogram_swatch, monogram_text, named_color,
    render_monogram,
};
use crate::project_switcher::fuzzy_match;
use crate::server_client::ServerClient;

const ICON_PICKER_KEY_CONTEXT: &str = "ProjectIconPicker";
const IMAGE_PICKER_KEY_CONTEXT: &str = "ProjectImagePicker";
/// t3code's dialog is 32rem wide.
const ICON_PICKER_WIDTH: Pixels = px(512.);
const GRID_COLUMNS: u16 = 10;
/// t3code's grids scroll past 16rem.
const GRID_MAX_HEIGHT: Pixels = px(256.);
/// t3code's search shows its first 60 icons.
const ICON_SEARCH_LIMIT: usize = 60;
/// t3code's `PROJECT_FILE_PICKER_RESULT_LIMIT`.
const IMAGE_LIMIT: usize = 200;
/// t3code's default, `folder-code`, is a folder here.
const DEFAULT_ICON: IconName = IconName::Folder;
const DEFAULT_EMOJI: &str = "💻";

/// t3code's popular icons, shown before a search, as far as Zed has them: a folder for
/// `folder-code`, a globe for `globe-2`, a box for `package`, a hammer for `wrench`, and so on.
const POPULAR_ICONS: [IconName; 18] = [
    IconName::Folder,
    IconName::Code,
    IconName::Terminal,
    IconName::ToolWeb,
    IconName::Server,
    IconName::DatabaseZap,
    IconName::Bot,
    IconName::Sparkle,
    IconName::Laptop,
    IconName::Screen,
    IconName::Cloud,
    IconName::Box,
    IconName::Book,
    IconName::Lock,
    IconName::Image,
    IconName::GitBranch,
    IconName::Blocks,
    IconName::ToolHammer,
];

/// t3code's `PROJECT_EMOJIS`.
const PROJECT_EMOJIS: [(&str, &str); 30] = [
    ("💻", "Computer"),
    ("🛠️", "Tools"),
    ("🚀", "Rocket"),
    ("🤖", "Robot"),
    ("✨", "Sparkles"),
    ("⚡", "Lightning"),
    ("🌐", "Web"),
    ("📱", "Mobile"),
    ("🖥️", "Desktop"),
    ("⌨️", "Keyboard"),
    ("⚙️", "Gear"),
    ("🗄️", "Database"),
    ("☁️", "Cloud"),
    ("📦", "Package"),
    ("📚", "Books"),
    ("🧪", "Test tube"),
    ("🔒", "Lock"),
    ("🎮", "Game"),
    ("🎵", "Music"),
    ("🎬", "Movie"),
    ("🖼️", "Picture"),
    ("🛍️", "Shopping"),
    ("🔥", "Fire"),
    ("💡", "Idea"),
    ("🧩", "Puzzle"),
    ("📊", "Chart"),
    ("🧠", "Brain"),
    ("🦄", "Unicorn"),
    ("🐙", "Octopus"),
    ("🌱", "Seedling"),
];

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", menu::Cancel, Some(ICON_PICKER_KEY_CONTEXT)),
        KeyBinding::new("up", menu::SelectPrevious, Some(IMAGE_PICKER_KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(IMAGE_PICKER_KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(IMAGE_PICKER_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(IMAGE_PICKER_KEY_CONTEXT)),
    ]);
}

/// The icons whose names have the query in them, as t3code searches Lucide's, or the popular
/// ones without a query.
fn matching_icons(query: &str) -> Vec<IconName> {
    let query = query
        .trim()
        .to_lowercase()
        .split(|character: char| character.is_whitespace() || character == '-')
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if query.is_empty() {
        return POPULAR_ICONS.to_vec();
    }
    IconName::iter()
        .filter(|icon| <&'static str>::from(*icon).contains(&query))
        .take(ICON_SEARCH_LIMIT)
        .collect()
}

/// An icon's name in words, as t3code labels one: `git_branch` is "Git Branch".
fn icon_label(icon: IconName) -> String {
    <&'static str>::from(icon)
        .split('_')
        .map(|word| {
            let mut characters = word.chars();
            characters
                .next()
                .map(|first| first.to_uppercase().chain(characters).collect::<String>())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The first emoji in what was pasted, as t3code's `firstEmoji` finds it: the first grapheme,
/// if it's a pictograph, a flag or a keycap.
fn first_emoji(value: &str) -> Option<&str> {
    let grapheme = value.trim().graphemes(true).next()?;
    let characters: Vec<char> = grapheme.chars().collect();
    let is_regional_indicator = |character: &char| ('\u{1F1E6}'..='\u{1F1FF}').contains(character);
    let is_flag = characters.len() == 2 && characters.iter().all(is_regional_indicator);
    let is_keycap = matches!(
        characters[..],
        [base, '\u{20E3}'] | [base, '\u{FE0F}', '\u{20E3}']
            if base.is_ascii_digit() || base == '#' || base == '*'
    );
    // Digits, `#` and `*` are emoji characters too, but only as keycaps.
    let is_pictograph = characters.iter().any(|character| {
        !character.is_ascii() && !is_regional_indicator(character) && character.is_emoji_char()
    });
    (is_flag || is_keycap || is_pictograph).then_some(grapheme)
}

/// What the monogram's letters are kept as, if they're one or two letters or numbers.
fn valid_monogram(letters: &str) -> Option<String> {
    let text = letters.trim().to_uppercase();
    let count = text.chars().count();
    ((1..=2).contains(&count) && text.chars().all(char::is_alphanumeric)).then_some(text)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Icons,
    Emoji,
    Monogram,
}

/// t3code's "Choose project icon" dialog.
pub struct ProjectIconPicker {
    focus_handle: FocusHandle,
    mode: Mode,
    icon: IconName,
    /// One of [`MONOGRAM_COLORS`]' names.
    color: &'static str,
    emoji: SharedString,
    automatic_monogram: String,
    search: Entity<TextInput>,
    letters: Entity<TextInput>,
    custom_emoji: Entity<TextInput>,
    icons_scroll: ScrollHandle,
    emoji_scroll: ScrollHandle,
    on_save: Rc<dyn Fn(ProjectIcon, &mut Window, &mut App)>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for ProjectIconPicker {}

impl Focusable for ProjectIconPicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl ProjectIconPicker {
    /// Opens on the project's current icon, or on the folder icon in the project's automatic
    /// color.
    pub fn new(
        current: Option<&ProjectIcon>,
        project_name: &str,
        on_save: impl Fn(ProjectIcon, &mut Window, &mut App) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (automatic_monogram, automatic_color) = automatic_monogram(project_name);
        let mode = match current {
            Some(ProjectIcon::Emoji { .. }) => Mode::Emoji,
            Some(ProjectIcon::Monogram { .. }) => Mode::Monogram,
            Some(ProjectIcon::Icon { .. } | ProjectIcon::Image { .. }) | None => Mode::Icons,
        };
        let icon = match current {
            Some(ProjectIcon::Icon { name, .. }) => name.parse().unwrap_or(DEFAULT_ICON),
            _ => DEFAULT_ICON,
        };
        let color = match current {
            Some(ProjectIcon::Icon { color, .. } | ProjectIcon::Monogram { color, .. }) => {
                MONOGRAM_COLORS
                    .iter()
                    .find(|(name, _, _)| name == color)
                    .map(|(name, _, _)| *name)
            }
            _ => None,
        }
        .unwrap_or(automatic_color);
        let emoji = match current {
            Some(ProjectIcon::Emoji { emoji }) => emoji.clone(),
            _ => DEFAULT_EMOJI.to_string(),
        };
        let letters_text = match current {
            Some(ProjectIcon::Monogram { text, .. }) => text.clone(),
            _ => automatic_monogram.clone(),
        };
        let search = cx.new(|cx| TextInput::new("Search icons", cx));
        let letters = cx.new(|cx| {
            let mut input = TextInput::new("", cx);
            input.set_text(letters_text, cx);
            input
        });
        let custom_emoji = cx.new(|cx| TextInput::new("Paste an emoji", cx));
        let subscriptions = vec![
            cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
                this.icons_scroll.scroll_to_item(0);
                cx.notify();
            }),
            cx.subscribe(&letters, |_, _, _: &TextInputEvent, cx| cx.notify()),
            cx.subscribe(&custom_emoji, |this, input, _: &TextInputEvent, cx| {
                if let Some(emoji) = first_emoji(input.read(cx).text()) {
                    this.emoji = emoji.to_string().into();
                    cx.notify();
                }
            }),
        ];
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        Self {
            focus_handle,
            mode,
            icon,
            color,
            emoji: emoji.into(),
            automatic_monogram,
            search,
            letters,
            custom_emoji,
            icons_scroll: ScrollHandle::new(),
            emoji_scroll: ScrollHandle::new(),
            on_save: Rc::new(on_save),
            _subscriptions: subscriptions,
        }
    }

    fn monogram(&self, cx: &App) -> Option<String> {
        valid_monogram(self.letters.read(cx).text())
    }

    /// What Save icon keeps, unless the monogram's letters aren't valid.
    fn chosen(&self, cx: &App) -> Option<ProjectIcon> {
        Some(match self.mode {
            Mode::Icons => ProjectIcon::Icon {
                name: <&'static str>::from(self.icon).to_string(),
                color: self.color.to_string(),
            },
            Mode::Emoji => ProjectIcon::Emoji {
                emoji: self.emoji.to_string(),
            },
            Mode::Monogram => ProjectIcon::Monogram {
                text: self.monogram(cx)?,
                color: self.color.to_string(),
            },
        })
    }

    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(icon) = self.chosen(cx) else {
            return;
        };
        cx.emit(DismissEvent);
        (self.on_save)(icon, window, cx);
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.mode = mode;
        cx.notify();
    }

    fn render_mode_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tab = |label: &'static str, mode: Mode, cx: &mut Context<Self>| {
            ToggleButtonSimple::new(
                label,
                cx.listener(move |this, _, _, cx| this.set_mode(mode, cx)),
            )
        };
        div().debug_selector(|| "project-icon-modes".into()).child(
            ToggleButtonGroup::single_row(
                "project-icon-modes",
                [
                    tab("Icons", Mode::Icons, cx),
                    tab("Emoji", Mode::Emoji, cx),
                    tab("Monogram", Mode::Monogram, cx),
                ],
            )
            .style(ToggleButtonGroupStyle::Outlined)
            .label_size(LabelSize::Small)
            .selected_index(match self.mode {
                Mode::Icons => 0,
                Mode::Emoji => 1,
                Mode::Monogram => 2,
            }),
        )
    }

    fn render_colors(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let selected_border = colors.text.opacity(0.64);
        v_flex()
            .gap_2()
            .child(field_label("Color"))
            .child(
                h_flex()
                    .flex_wrap()
                    .gap_1p5()
                    .children(MONOGRAM_COLORS.iter().map(|(name, light, dark)| {
                        let is_selected = *name == self.color;
                        div()
                            .id(SharedString::from(format!("project-icon-color-{name}")))
                            .debug_selector(move || format!("project-icon-color-{name}"))
                            .size(px(24.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_full()
                            .border_1()
                            .border_color(if is_selected {
                                selected_border
                            } else {
                                gpui::transparent_black()
                            })
                            .cursor_pointer()
                            .tooltip(Tooltip::text(capitalized(name)))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.color = name;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .size(px(16.))
                                    .rounded_full()
                                    .bg(monogram_swatch(*light, *dark, cx)),
                            )
                    })),
            )
    }

    /// A grid's tile, outlined and filled while it's the one chosen, as t3code's.
    fn tile(id: SharedString, is_selected: bool, cx: &App) -> Stateful<Div> {
        let colors = cx.theme().colors();
        let hover = colors.element_hover;
        div()
            .id(id)
            .h(px(40.))
            .flex()
            .items_center()
            .justify_center()
            .rounded_md()
            .border_1()
            .border_color(if is_selected {
                colors.border
            } else {
                gpui::transparent_black()
            })
            .when(is_selected, |tile| tile.bg(colors.element_selected))
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
    }

    /// A grid that scrolls past t3code's height, its scrollbar on a wrapper that doesn't.
    fn scrolling_grid(
        id: &'static str,
        scroll: &ScrollHandle,
        tiles: Vec<AnyElement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id(SharedString::from(format!("{id}-scroll")))
            .child(
                div()
                    .id(id)
                    .max_h(GRID_MAX_HEIGHT)
                    .overflow_y_scroll()
                    .track_scroll(scroll)
                    .child(
                        div()
                            .grid()
                            .grid_cols(GRID_COLUMNS)
                            .gap_1()
                            .p_0p5()
                            .children(tiles),
                    ),
            )
            .vertical_scrollbar_for(scroll, window, cx)
    }

    fn render_icons(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let icons = matching_icons(self.search.read(cx).text());
        let color = named_color(self.color, cx).unwrap_or(cx.theme().colors().icon);
        let tiles: Vec<AnyElement> = icons
            .iter()
            .map(|icon| {
                let icon = *icon;
                let name: &'static str = icon.into();
                Self::tile(format!("project-icon-{name}").into(), icon == self.icon, cx)
                    .debug_selector(move || format!("project-icon-{name}"))
                    .tooltip(Tooltip::text(icon_label(icon)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.icon = icon;
                        cx.notify();
                    }))
                    .child(
                        Icon::new(icon)
                            .size(IconSize::Custom(rems_from_px(20_f32)))
                            .color(Color::Custom(color)),
                    )
                    .into_any_element()
            })
            .collect();
        v_flex()
            .gap_2()
            .child(text_field(&self.search, false, window, cx))
            .when(icons.is_empty(), |this| {
                this.child(
                    h_flex().justify_center().py_8().child(
                        Label::new("No icons found.")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
                )
            })
            .when(!icons.is_empty(), |this| {
                this.child(Self::scrolling_grid(
                    "project-icons",
                    &self.icons_scroll,
                    tiles,
                    window,
                    cx,
                ))
            })
    }

    fn render_emoji(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tiles: Vec<AnyElement> = PROJECT_EMOJIS
            .iter()
            .enumerate()
            .map(|(index, (emoji, label))| {
                let emoji = *emoji;
                Self::tile(
                    format!("project-emoji-{index}").into(),
                    *self.emoji == *emoji,
                    cx,
                )
                .debug_selector(move || format!("project-emoji-{label}"))
                .tooltip(Tooltip::text(*label))
                .text_size(px(20.))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.emoji = emoji.into();
                    cx.notify();
                }))
                .child(emoji)
                .into_any_element()
            })
            .collect();
        v_flex()
            .gap_4()
            .child(Self::scrolling_grid(
                "project-emoji",
                &self.emoji_scroll,
                tiles,
                window,
                cx,
            ))
            .child(
                v_flex()
                    .gap_2()
                    .child(field_label("Or paste any emoji"))
                    .child(text_field(&self.custom_emoji, false, window, cx)),
            )
    }

    fn render_monogram(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let monogram = self.monogram(cx);
        let is_invalid = monogram.is_none();
        let preview = monogram.unwrap_or_else(|| self.automatic_monogram.clone());
        let color = named_color(self.color, cx).unwrap_or(cx.theme().colors().icon);
        let font_family = theme::theme_settings(cx).buffer_font(cx).family.clone();
        h_flex()
            .gap_4()
            .py_2()
            .child(render_monogram(
                monogram_text(&preview).into(),
                color,
                font_family,
                px(48.),
            ))
            .child(
                v_flex()
                    .flex_1()
                    .gap_2()
                    .child(Label::new("Letters").size(LabelSize::Small))
                    .child(
                        div()
                            .debug_selector(|| "project-monogram-letters".into())
                            .child(text_field(&self.letters, is_invalid, window, cx)),
                    )
                    .child(
                        Label::new("One or two letters or numbers.")
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    ),
            )
    }
}

fn capitalized(name: &str) -> String {
    let mut characters = name.chars();
    characters
        .next()
        .map(|first| first.to_uppercase().chain(characters).collect())
        .unwrap_or_default()
}

impl Render for ProjectIconPicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let border_variant = colors.border_variant;
        let can_save = self.chosen(cx).is_some();
        let body = match self.mode {
            Mode::Icons => self.render_icons(window, cx).into_any_element(),
            Mode::Emoji => self.render_emoji(window, cx).into_any_element(),
            Mode::Monogram => self.render_monogram(window, cx).into_any_element(),
        };
        dialog_frame(cx)
            .key_context(ICON_PICKER_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::cancel))
            .w(ICON_PICKER_WIDTH)
            .child(
                v_flex()
                    .p_5()
                    .gap_4()
                    .child(
                        v_flex().child(dialog_title("Choose project icon")).child(
                            Label::new("Choose an icon, emoji, or monogram.")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                    )
                    .child(self.render_mode_tabs(cx))
                    .when(self.mode != Mode::Emoji, |this| {
                        this.child(self.render_colors(cx))
                    })
                    .child(body),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .px_5()
                    .py_3()
                    .border_t_1()
                    .border_color(border_variant)
                    .bg(gpui::black().opacity(0.08))
                    .child(
                        ActionButton::new("project-icon-cancel", "Cancel")
                            .style(ActionStyle::Outline)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel(&menu::Cancel, window, cx)
                            })),
                    )
                    .child(
                        div().debug_selector(|| "project-icon-save".into()).child(
                            ActionButton::new("project-icon-save", "Save icon")
                                .style(ActionStyle::Primary)
                                .disabled(!can_save)
                                .on_click(cx.listener(|this, _, window, cx| this.save(window, cx))),
                        ),
                    ),
            )
    }
}

/// t3code's file picker for a project's icon: the image files in the project, searched by
/// path.
pub struct ProjectImagePicker {
    project_name: SharedString,
    search: Entity<TextInput>,
    /// The files' paths in the project, once its server has listed them, or why it couldn't.
    files: Option<Result<Vec<String>, SharedString>>,
    /// The files that match, by index, with where their paths matched.
    matches: Vec<(usize, Vec<usize>)>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    on_select: Rc<dyn Fn(PathBuf, &mut Window, &mut App)>,
    /// Opens this Mac's file picker, for a file outside the project.
    on_pick_external: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    _list: Task<()>,
    _subscription: Subscription,
}

impl EventEmitter<DismissEvent> for ProjectImagePicker {}

impl Focusable for ProjectImagePicker {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.focus_handle(cx)
    }
}

impl ProjectImagePicker {
    pub fn new(
        client: &Entity<ServerClient>,
        project: ProjectId,
        project_name: SharedString,
        on_select: impl Fn(PathBuf, &mut Window, &mut App) + 'static,
        on_pick_external: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| TextInput::new("Search image files…", cx));
        let subscription = cx.subscribe(&search, |this, _, _: &TextInputEvent, cx| {
            this.selected_index = 0;
            this.update_matches(cx);
        });
        window.focus(&search.focus_handle(cx), cx);
        let listing = client.read(cx).request(Request::ProjectImageFiles(project));
        let list = cx.spawn(async move |this, cx| {
            let files = match listing.await {
                Ok(Response::Files(listing)) => Ok(listing
                    .entries
                    .into_iter()
                    .filter(|entry| !entry.is_dir)
                    .map(|entry| entry.path)
                    .collect()),
                Ok(response) => Err(format!("Unexpected response: {response:?}").into()),
                Err(error) => Err(format!("{error:#}").into()),
            };
            this.update(cx, |this, cx| {
                this.files = Some(files);
                this.update_matches(cx);
            })
            .ok();
        });
        Self {
            project_name,
            search,
            files: None,
            matches: Vec::new(),
            selected_index: 0,
            scroll_handle: ScrollHandle::new(),
            on_select: Rc::new(on_select),
            on_pick_external,
            _list: list,
            _subscription: subscription,
        }
    }

    /// t3code's query: leading `@`, `.` and `/` and every space dropped.
    fn query(&self, cx: &App) -> String {
        self.search
            .read(cx)
            .text()
            .trim()
            .trim_start_matches(['@', '.', '/'])
            .split_whitespace()
            .collect::<String>()
            .to_lowercase()
    }

    fn update_matches(&mut self, cx: &mut Context<Self>) {
        let query = self.query(cx);
        self.matches = match &self.files {
            Some(Ok(files)) => files
                .iter()
                .enumerate()
                .filter_map(|(index, path)| Some((index, fuzzy_match(&query, path)?)))
                .take(IMAGE_LIMIT)
                .collect(),
            _ => Vec::new(),
        };
        self.selected_index = self
            .selected_index
            .min(self.matches.len().saturating_sub(1));
        self.scroll_handle.scroll_to_item(0);
        cx.notify();
    }

    fn path(&self, index: usize) -> Option<&str> {
        let (file, _) = self.matches.get(index)?;
        match &self.files {
            Some(Ok(files)) => files.get(*file).map(String::as_str),
            _ => None,
        }
    }

    fn select_next(&mut self, _: &menu::SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        if !self.matches.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.matches.len();
            // The project's heading is the list's first item.
            self.scroll_handle.scroll_to_item(self.selected_index + 1);
            cx.notify();
        }
    }

    fn select_previous(
        &mut self,
        _: &menu::SelectPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.matches.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.matches.len() - 1);
            self.scroll_handle.scroll_to_item(self.selected_index + 1);
            cx.notify();
        }
    }

    fn confirm(&mut self, _: &menu::Confirm, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.path(self.selected_index).map(PathBuf::from) {
            self.choose(path, window, cx);
        }
    }

    fn cancel(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
    }

    fn choose(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        cx.emit(DismissEvent);
        (self.on_select)(path, window, cx);
    }

    fn pick_external(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pick) = self.on_pick_external.clone() else {
            return;
        };
        cx.emit(DismissEvent);
        pick(window, cx);
    }

    fn empty_message(&self, cx: &App) -> SharedString {
        let has_query = !self.query(cx).is_empty();
        match &self.files {
            Some(Err(error)) => error.clone(),
            None if has_query => "Searching project files…".into(),
            None => "Indexing project files…".into(),
            Some(Ok(_)) if has_query => "No matching image files.".into(),
            Some(Ok(_)) => "No image files found.".into(),
        }
    }

    fn render_match(&self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (_, path_positions) = self.matches.get(index)?;
        let path = self.path(index)?.to_string();
        let name_start = path.rfind('/').map_or(0, |slash| slash + 1);
        let name = path[name_start..].to_string();
        let name_positions = fuzzy_match(&self.query(cx), &name).unwrap_or_default();
        let selector = format!("project-image-{path}");
        Some(
            ListItem::new(("project-image", index))
                .inset(true)
                .spacing(ListItemSpacing::Sparse)
                .toggle_state(index == self.selected_index)
                .start_slot(
                    Icon::new(IconName::Image)
                        .size(IconSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    h_flex()
                        .debug_selector(move || selector)
                        .min_w_0()
                        .gap_2()
                        .child(HighlightedLabel::new(name, name_positions))
                        .child(
                            HighlightedLabel::new(path.clone(), path_positions.clone())
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.choose(PathBuf::from(&path), window, cx)
                }))
                .into_any_element(),
        )
    }
}

impl Render for ProjectImagePicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let border_variant = cx.theme().colors().border_variant;
        let mut rows = Vec::with_capacity(self.matches.len() + 1);
        if !self.matches.is_empty() {
            rows.push(
                ListSubHeader::new(self.project_name.clone())
                    .inset(true)
                    .into_any_element(),
            );
        }
        rows.extend((0..self.matches.len()).filter_map(|index| self.render_match(index, cx)));
        let file_manager = if cfg!(target_os = "macos") {
            "Finder"
        } else {
            "Files"
        };
        let hint = |key: &'static str, label: &'static str, cx: &App| {
            h_flex()
                .gap_1()
                .child(key_hint(key, cx))
                .child(Label::new(label).size(LabelSize::Small).color(Color::Muted))
        };
        v_flex()
            .key_context(IMAGE_PICKER_KEY_CONTEXT)
            .w(rems(34.))
            .elevation_3(cx)
            .overflow_hidden()
            .on_action(cx.listener(Self::select_next))
            .on_action(cx.listener(Self::select_previous))
            .on_action(cx.listener(Self::confirm))
            .on_action(cx.listener(Self::cancel))
            .child(
                h_flex()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .border_b_1()
                    .border_color(border_variant)
                    .child(
                        Icon::new(IconName::MagnifyingGlass)
                            .size(IconSize::Small)
                            .color(Color::Muted),
                    )
                    .child(self.search.clone()),
            )
            .child(
                // The scrollbar sits on this non-scrolling wrapper so it stays put, as in Zed.
                div()
                    .id("project-images-scroll")
                    .child(
                        v_flex()
                            .id("project-images")
                            .max_h(rems(24.))
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll_handle)
                            .p_1()
                            .children(rows)
                            .when(self.matches.is_empty(), |list| {
                                list.child(
                                    div().px_2().py_1p5().child(
                                        Label::new(self.empty_message(cx)).color(Color::Muted),
                                    ),
                                )
                            }),
                    )
                    .vertical_scrollbar_for(&self.scroll_handle, window, cx),
            )
            .child(
                h_flex()
                    .px_3()
                    .py_1p5()
                    .gap_3()
                    .border_t_1()
                    .border_color(border_variant)
                    .child(hint("↵", "Select icon", cx))
                    .child(hint("esc", "Close", cx))
                    .child(div().flex_1())
                    .when(self.on_pick_external.is_some(), |footer| {
                        footer.child(
                            div()
                                .debug_selector(|| "project-image-external".into())
                                .child(
                                    Button::new(
                                        "project-image-external",
                                        format!("Open in {file_manager}"),
                                    )
                                    .style(ButtonStyle::Subtle)
                                    .label_size(LabelSize::Small)
                                    .on_click(cx.listener(
                                        |this, _, window, cx| this.pick_external(window, cx),
                                    )),
                                ),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use agentz_protocol::spaces::SpacesSnapshot;
    use agentz_protocol::{FileEntry, FileListing};
    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    use super::*;
    use crate::machines::MachineId;

    #[test]
    fn icons_are_searched_by_name() {
        assert_eq!(matching_icons(""), POPULAR_ICONS);
        assert!(matching_icons("git branch").contains(&IconName::GitBranch));
        assert!(matching_icons(" Git-Branch ").contains(&IconName::GitBranch));
        assert!(matching_icons("no such icon").is_empty());
        assert!(matching_icons("e").len() <= ICON_SEARCH_LIMIT);
        assert_eq!(icon_label(IconName::GitBranch), "Git Branch");
    }

    #[test]
    fn the_first_emoji_is_taken_from_what_is_pasted() {
        assert_eq!(first_emoji(" 🚀 launch"), Some("🚀"));
        assert_eq!(first_emoji("👩‍💻"), Some("👩‍💻"));
        assert_eq!(first_emoji("🇺🇿"), Some("🇺🇿"));
        assert_eq!(first_emoji("1️⃣"), Some("1️⃣"));
        assert_eq!(first_emoji("1"), None);
        assert_eq!(first_emoji("rocket 🚀"), None);
        assert_eq!(first_emoji(""), None);
    }

    #[test]
    fn a_monogram_is_one_or_two_letters_or_numbers() {
        assert_eq!(valid_monogram(" q7 ").as_deref(), Some("Q7"));
        assert_eq!(valid_monogram("é").as_deref(), Some("É"));
        assert_eq!(valid_monogram("abc"), None);
        assert_eq!(valid_monogram("a!"), None);
        assert_eq!(valid_monogram(""), None);
    }

    fn click(selector: &'static str, cx: &mut VisualTestContext) {
        let bounds = cx
            .debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} is shown"));
        cx.simulate_click(bounds.center(), Modifiers::none());
        cx.run_until_parked();
    }

    fn open_icon_picker(
        current: Option<ProjectIcon>,
        cx: &mut TestAppContext,
    ) -> (
        Entity<ProjectIconPicker>,
        Rc<RefCell<Vec<ProjectIcon>>>,
        &mut VisualTestContext,
    ) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
        });
        let saved = Rc::new(RefCell::new(Vec::new()));
        let (picker, cx) = cx.add_window_view({
            let saved = saved.clone();
            move |window, cx| {
                ProjectIconPicker::new(
                    current.as_ref(),
                    "shop-landing",
                    move |icon, _, _| saved.borrow_mut().push(icon),
                    window,
                    cx,
                )
            }
        });
        cx.run_until_parked();
        (picker, saved, cx)
    }

    #[gpui::test]
    fn an_icon_is_searched_for_and_saved_in_a_color(cx: &mut TestAppContext) {
        let (picker, saved, cx) = open_icon_picker(None, cx);
        // The folder icon, in the project's own color, until another is chosen.
        let (_, automatic_color) = automatic_monogram("shop-landing");
        assert_eq!(
            picker.read_with(cx, |picker, cx| picker.chosen(cx)),
            Some(ProjectIcon::Icon {
                name: "folder".into(),
                color: automatic_color.into(),
            })
        );
        assert!(cx.debug_bounds("project-icon-git_branch").is_some());
        assert!(cx.debug_bounds("project-icon-git_commit").is_none());
        picker.update(cx, |picker, cx| {
            picker
                .search
                .update(cx, |input, cx| input.set_text("git commit", cx))
        });
        cx.run_until_parked();
        click("project-icon-git_commit", cx);
        click("project-icon-color-teal", cx);
        click("project-icon-save", cx);
        assert_eq!(
            *saved.borrow(),
            [ProjectIcon::Icon {
                name: "git_commit".into(),
                color: "teal".into(),
            }]
        );
    }

    #[gpui::test]
    fn an_emoji_is_picked_or_pasted(cx: &mut TestAppContext) {
        let (picker, saved, cx) = open_icon_picker(None, cx);
        picker.update(cx, |picker, cx| picker.set_mode(Mode::Emoji, cx));
        cx.run_until_parked();
        // Emoji have no color.
        assert!(cx.debug_bounds("project-icon-color-teal").is_none());
        click("project-emoji-Rocket", cx);
        click("project-icon-save", cx);
        picker.update(cx, |picker, cx| {
            picker
                .custom_emoji
                .update(cx, |input, cx| input.set_text("🦀 crab", cx))
        });
        cx.run_until_parked();
        click("project-icon-save", cx);
        assert_eq!(
            *saved.borrow(),
            [
                ProjectIcon::Emoji {
                    emoji: "🚀".into()
                },
                ProjectIcon::Emoji {
                    emoji: "🦀".into()
                },
            ]
        );
    }

    #[gpui::test]
    fn a_monogram_opens_as_it_is_and_saves_valid_letters(cx: &mut TestAppContext) {
        let current = ProjectIcon::Monogram {
            text: "XY".into(),
            color: "rose".into(),
        };
        let (picker, saved, cx) = open_icon_picker(Some(current.clone()), cx);
        assert_eq!(
            picker.read_with(cx, |picker, cx| picker.chosen(cx)),
            Some(current)
        );
        assert!(cx.debug_bounds("project-monogram-letters").is_some());
        let set_letters = |letters: &'static str, cx: &mut VisualTestContext| {
            picker.update(cx, |picker, cx| {
                picker
                    .letters
                    .update(cx, |input, cx| input.set_text(letters, cx))
            });
            cx.run_until_parked();
        };
        set_letters("abc", cx);
        click("project-icon-save", cx);
        assert!(saved.borrow().is_empty());
        set_letters("q7", cx);
        click("project-icon-save", cx);
        assert_eq!(
            *saved.borrow(),
            [ProjectIcon::Monogram {
                text: "Q7".into(),
                color: "rose".into(),
            }]
        );
    }

    #[gpui::test]
    fn an_image_file_is_picked_from_the_project_s_listing(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            init(cx);
        });
        let client = cx.update(|cx| {
            let client = ServerClient::new_for_test(
                MachineId::Remote(1),
                "".into(),
                SpacesSnapshot::default(),
                cx,
            );
            client.update(cx, |client, _| {
                client.answer_for_test(|request| match request {
                    Request::ProjectImageFiles(ProjectId(1)) => {
                        Some(Response::Files(FileListing {
                            root: "/srv/demo".into(),
                            entries: ["assets/logo.png", "public/favicon.svg"]
                                .map(|path| FileEntry {
                                    path: path.into(),
                                    is_dir: false,
                                })
                                .into(),
                        }))
                    }
                    _ => None,
                })
            });
            client
        });
        let chosen = Rc::new(RefCell::new(Vec::new()));
        let (picker, cx) = cx.add_window_view({
            let chosen = chosen.clone();
            move |window, cx| {
                ProjectImagePicker::new(
                    &client,
                    ProjectId(1),
                    "demo".into(),
                    move |path, _, _| chosen.borrow_mut().push(path),
                    None,
                    window,
                    cx,
                )
            }
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("project-image-assets/logo.png").is_some());
        // Files outside the project are only for this Mac's copies.
        assert!(cx.debug_bounds("project-image-external").is_none());
        cx.simulate_input("fav");
        cx.run_until_parked();
        assert!(cx.debug_bounds("project-image-assets/logo.png").is_none());
        assert!(
            cx.debug_bounds("project-image-public/favicon.svg")
                .is_some()
        );
        cx.simulate_keystrokes("enter");
        assert_eq!(*chosen.borrow(), [PathBuf::from("public/favicon.svg")]);
        picker.read_with(cx, |picker, _| assert!(picker.files.is_some()));
    }
}
