//! A picture of the user's behind the window (`design/appearance`). agentZ draws it under
//! everything and makes the theme's colors partly transparent over it, so it looks the same on
//! every desktop, as Warp's and iTerm2's background images do.
//!
//! Painting a translucent color twice doubles its opacity, and much of agentZ paints a surface's
//! color on a surface of the same color (cards on the thread). So only one layer covers the
//! picture: the editor's color at the opacity the user picked. Every other surface color becomes
//! the least opaque color that shows as it over the editor's, so a card in the editor's color
//! adds nothing, and a sidebar or a selected row shows as it would on a solid window.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use gpui::{
    App, Decorations, Global, Hsla, ImageSource, ObjectFit, Rgba, StyledImage as _, img, prelude::*,
};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder as _, ImageReader};
use theme::Theme;
use ui::prelude::*;
use util::ResultExt as _;

use crate::app_settings::{AppSettings, WindowBackground};
use crate::window_decorations::RoundedTopCorners as _;

/// How much of the theme's color covers the picture until the user moves the slider: a hint of
/// the picture, mostly the theme (the board's "A little").
pub const DEFAULT_OPACITY: f32 = 0.88;
/// The most of the picture the slider lets show through. Past about 30%, text over the picture
/// gets hard to read, and people keep it at 10–20%.
pub const MAX_SHOWS_THROUGH: f32 = 0.3;

/// The copy's longest side: a full-screen window on a 4K display. Larger pictures only cost
/// memory, since the window draws the copy whole.
const PICTURE_SIZE: u32 = 3840;
/// The softened copy's longest side. It's blurred, so the window scaling it up doesn't show.
const SOFT_PICTURE_SIZE: u32 = 1280;
/// The blur's radius at that size: about what the board's mock showed.
const SOFT_PICTURE_BLUR: f32 = 28.;
const JPEG_QUALITY: u8 = 90;

/// Where Choose… keeps its copies of the user's picture, so moving or deleting the original
/// doesn't change the window.
fn pictures_dir() -> PathBuf {
    paths::data_dir().join("backgrounds")
}

/// The copy of the picture named `name`, sharp or softened.
pub fn picture_path(name: &str, soften: bool) -> PathBuf {
    picture_path_in(&pictures_dir(), name, soften)
}

fn picture_path_in(directory: &Path, name: &str, soften: bool) -> PathBuf {
    let file = if soften {
        format!("{name}-soft.jpg")
    } else {
        format!("{name}.jpg")
    };
    directory.join(file)
}

/// Copies a picture for the window: turned as its camera says, scaled down to what a window
/// shows, and once more small and blurred for Soften picture. Returns the copies' name, new for
/// each picture so the window's image cache never shows an old one. Slow; run it in the
/// background.
pub async fn import_picture(source: &Path) -> Result<String> {
    import_picture_into(source, &pictures_dir()).await
}

async fn import_picture_into(source: &Path, directory: &Path) -> Result<String> {
    let picture = match read_picture(source) {
        Ok(picture) => picture,
        #[cfg(target_os = "macos")]
        Err(error) => read_with_sips(source).await.map_err(|sips_error| {
            log::info!("sips couldn't convert {}: {sips_error:#}", source.display());
            error
        })?,
        #[cfg(not(target_os = "macos"))]
        Err(error) => return Err(error),
    };

    let sharp = scaled_down(picture, PICTURE_SIZE, FilterType::CatmullRom);
    let soft = scaled_down(sharp.clone(), SOFT_PICTURE_SIZE, FilterType::Triangle)
        .fast_blur(SOFT_PICTURE_BLUR);

    let name = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    std::fs::create_dir_all(directory)
        .with_context(|| format!("creating {}", directory.display()))?;
    write_jpeg(&sharp, &picture_path_in(directory, &name, false))?;
    write_jpeg(&soft, &picture_path_in(directory, &name, true))?;
    Ok(name)
}

/// The picture at `path`, turned as its camera says.
fn read_picture(path: &Path) -> Result<DynamicImage> {
    let mut decoder = ImageReader::open(path)
        .with_context(|| format!("opening {}", path.display()))?
        .with_guessed_format()
        .with_context(|| format!("reading {}", path.display()))?
        .into_decoder()
        .context("it isn't a picture agentZ can read (PNG, JPEG, WebP, GIF, TIFF or BMP)")?;
    let orientation = decoder.orientation()?;
    let mut picture = DynamicImage::from_decoder(decoder).context("decoding the picture")?;
    picture.apply_orientation(orientation);
    Ok(picture)
}

/// macOS's own pictures, its wallpapers and an iPhone's photos, are HEIC, which `image` can't
/// read. `sips`, which comes with macOS, converts them to a JPEG, keeping how they're turned.
#[cfg(target_os = "macos")]
async fn read_with_sips(source: &Path) -> Result<DynamicImage> {
    let converted = std::env::temp_dir().join(format!(
        "agentz-picture-{}-{}.jpg",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let output = smol::process::Command::new("/usr/bin/sips")
        .args(["-s", "format", "jpeg", "-s", "formatOptions", "best"])
        .arg(source)
        .arg("--out")
        .arg(&converted)
        .output()
        .await
        .context("running sips")?;
    let picture = if output.status.success() {
        read_picture(&converted)
    } else {
        Err(anyhow::anyhow!(
            "sips failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    };
    if converted.exists() {
        std::fs::remove_file(&converted).log_err();
    }
    picture
}

fn scaled_down(picture: DynamicImage, longest_side: u32, filter: FilterType) -> DynamicImage {
    if picture.width().max(picture.height()) <= longest_side {
        picture
    } else {
        picture.resize(longest_side, longest_side, filter)
    }
}

fn write_jpeg(picture: &DynamicImage, path: &Path) -> Result<()> {
    let file =
        std::fs::File::create(path).with_context(|| format!("creating {}", path.display()))?;
    JpegEncoder::new_with_quality(std::io::BufWriter::new(file), JPEG_QUALITY)
        .encode_image(&picture.to_rgb8())
        .with_context(|| format!("writing {}", path.display()))
}

/// Deletes the copies of every picture but `keep`'s.
pub fn remove_other_pictures(keep: Option<&str>) -> Result<()> {
    remove_other_pictures_in(&pictures_dir(), keep)
}

fn remove_other_pictures_in(directory: &Path, keep: Option<&str>) -> Result<()> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("reading {}", directory.display()));
        }
    };
    for entry in entries {
        let path = entry?.path();
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if keep != Some(stem.trim_end_matches("-soft")) {
            std::fs::remove_file(&path).with_context(|| format!("deleting {}", path.display()))?;
        }
    }
    Ok(())
}

/// Lets go of a copy the window no longer shows, which its image cache would keep in memory.
pub fn forget_picture(path: PathBuf, cx: &mut App) {
    ImageSource::from(path).remove_asset(cx);
}

/// The copy the window shows for these settings, if it shows one and it's there.
fn picture_to_show(settings: &AppSettings) -> Option<PathBuf> {
    if settings.window_background != WindowBackground::Picture {
        return None;
    }
    let path = picture_path(
        settings.background_picture.as_deref()?,
        settings.soften_background_picture,
    );
    path.exists().then_some(path)
}

/// The picture behind the window, and the layer of the theme's color that covers it.
#[derive(Clone, Debug, PartialEq)]
pub struct ShownPicture {
    path: PathBuf,
    /// The editor's color at the user's opacity.
    tint: Hsla,
    /// The editor's color, shown while the picture loads or if it can't be read.
    solid: Hsla,
}

impl ShownPicture {
    /// The copy's file name in the data directory's `backgrounds`, which the server serves the
    /// artifacts pages from.
    pub fn file_name(&self) -> Option<String> {
        Some(self.path.file_name()?.to_string_lossy().into_owned())
    }

    /// The layer of the editor's color over the picture.
    pub fn tint(&self) -> Hsla {
        self.tint
    }
}

#[derive(Default)]
struct WindowPicture {
    shown: Option<ShownPicture>,
    /// The last see-through theme made, so applying the settings again keeps the same one.
    see_through: Option<SeeThroughTheme>,
}

impl Global for WindowPicture {}

struct SeeThroughTheme {
    source: Arc<Theme>,
    opacity: f32,
    theme: Arc<Theme>,
}

/// The theme to show for these settings: `theme` itself, or with a picture behind the window,
/// its see-through copy. Remembers the picture for the shell, and says whether it changed.
pub fn apply(theme: Arc<Theme>, settings: &AppSettings, cx: &mut App) -> (Arc<Theme>, bool) {
    let picture = picture_to_show(settings);
    let state = cx.default_global::<WindowPicture>();
    let (theme, shown) = match picture {
        None => (theme, None),
        Some(path) => {
            let opacity = settings
                .background_picture_opacity
                .clamp(1. - MAX_SHOWS_THROUGH, 1.);
            let see_through = match &state.see_through {
                Some(cached)
                    if Arc::ptr_eq(&cached.source, &theme) && cached.opacity == opacity =>
                {
                    cached.theme.clone()
                }
                _ => {
                    let see_through = Arc::new(see_through(&theme, opacity));
                    state.see_through = Some(SeeThroughTheme {
                        source: theme.clone(),
                        opacity,
                        theme: see_through.clone(),
                    });
                    see_through
                }
            };
            let editor = theme.colors().editor_background;
            let shown = ShownPicture {
                path,
                tint: Hsla {
                    a: opacity,
                    ..editor
                },
                solid: Hsla { a: 1., ..editor },
            };
            (see_through, Some(shown))
        }
    };
    let changed = state.shown != shown;
    state.shown = shown;
    (theme, changed)
}

/// The picture the window shows behind everything, if any.
pub fn shown_picture(cx: &App) -> Option<ShownPicture> {
    cx.try_global::<WindowPicture>()?.shown.clone()
}

/// The theme the window's see-through one was made from, while a picture shows, for colors
/// that must stay solid over it.
pub fn solid_theme(cx: &App) -> Option<Arc<Theme>> {
    let state = cx.try_global::<WindowPicture>()?;
    state.shown.as_ref()?;
    Some(state.see_through.as_ref()?.source.clone())
}

/// The editor's color for a mark drawn in it on a filled shape (Send's arrow, a badge's count):
/// solid while a picture shows, as the see-through theme's is clear and the mark would vanish.
pub fn solid_editor_background(cx: &App) -> Hsla {
    solid_theme(cx).map_or_else(
        || cx.theme().colors().editor_background,
        |theme| theme.colors().editor_background,
    )
}

/// What to paint a surface's color over where it must hide what's under it, as a cover over the
/// end of a long title does: while a picture shows, the editor's color, solid, which the
/// surfaces' colors are drawn to show over; otherwise nothing, as they're solid themselves.
pub fn solid_backdrop(cx: &App) -> Hsla {
    cx.try_global::<WindowPicture>()
        .and_then(|state| state.shown.as_ref())
        .map_or(gpui::transparent_black(), |picture| picture.solid)
}

/// A solid color mixed from the theme's (over [`solid_backdrop`]), made see-through as the
/// theme's own surfaces are while a picture shows.
pub fn see_through_color(color: Hsla, cx: &App) -> Hsla {
    match cx
        .try_global::<WindowPicture>()
        .and_then(|state| state.shown.as_ref())
    {
        Some(picture) => overlay(color, picture.solid.to_rgb()),
        None => color,
    }
}

/// The picture filling the window, under the theme's color, for the shell to put first.
pub fn render_picture(picture: ShownPicture, decorations: Decorations) -> impl IntoElement {
    let ShownPicture { path, tint, solid } = picture;
    let rounded = move |element: Div| match decorations {
        Decorations::Client { tiling } => element.rounded_top_corners(tiling),
        Decorations::Server => element,
    };
    let solid_fill = move || rounded(div().size_full().bg(solid)).into_any_element();
    div()
        .absolute()
        .inset_0()
        .child(
            img(path)
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Cover)
                .map(|picture| match decorations {
                    Decorations::Client { tiling } => picture.rounded_top_corners(tiling),
                    Decorations::Server => picture,
                })
                .with_loading(solid_fill)
                .with_fallback(solid_fill),
        )
        .child(rounded(div().absolute().inset_0().bg(tint)))
}

/// `theme` with its surfaces partly transparent, for a window whose only cover over the picture
/// is the editor's color at `opacity` (see the module's comment). Menus and popovers open over
/// the window's content rather than its background, so they're their own color at `opacity`.
fn see_through(theme: &Theme, opacity: f32) -> Theme {
    let mut theme = theme.clone();
    let base = Hsla {
        a: 1.,
        ..theme.styles.colors.editor_background
    }
    .to_rgb();
    let colors = &mut theme.styles.colors;
    for color in [
        &mut colors.background,
        &mut colors.surface_background,
        &mut colors.element_background,
        &mut colors.element_hover,
        &mut colors.element_active,
        &mut colors.element_selected,
        &mut colors.element_selection_background,
        &mut colors.element_disabled,
        &mut colors.drop_target_background,
        &mut colors.ghost_element_background,
        &mut colors.ghost_element_hover,
        &mut colors.ghost_element_active,
        &mut colors.ghost_element_selected,
        &mut colors.ghost_element_disabled,
        &mut colors.status_bar_background,
        &mut colors.title_bar_background,
        &mut colors.title_bar_inactive_background,
        &mut colors.toolbar_background,
        &mut colors.tab_bar_background,
        &mut colors.tab_inactive_background,
        &mut colors.tab_active_background,
        &mut colors.panel_background,
        &mut colors.panel_overlay_background,
        &mut colors.panel_overlay_hover,
        &mut colors.scrollbar_track_background,
        &mut colors.editor_background,
        &mut colors.editor_gutter_background,
        &mut colors.editor_subheader_background,
        &mut colors.editor_active_line_background,
        &mut colors.editor_highlighted_line_background,
        &mut colors.editor_diff_hunk_added_background,
        &mut colors.editor_diff_hunk_deleted_background,
        &mut colors.terminal_background,
        &mut colors.terminal_ansi_background,
    ] {
        *color = overlay(*color, base);
    }
    colors.elevated_surface_background = colors.elevated_surface_background.opacity(opacity);
    let status = &mut theme.styles.status;
    for color in [
        &mut status.conflict_background,
        &mut status.created_background,
        &mut status.deleted_background,
        &mut status.error_background,
        &mut status.hidden_background,
        &mut status.hint_background,
        &mut status.ignored_background,
        &mut status.info_background,
        &mut status.modified_background,
        &mut status.predictive_background,
        &mut status.renamed_background,
        &mut status.success_background,
        &mut status.unreachable_background,
        &mut status.warning_background,
    ] {
        *color = overlay(*color, base);
    }
    theme
}

/// The least opaque color that, painted over `base`, shows as `color`: GIMP's color to alpha. A
/// color that's already partly transparent is kept, as it was made to be painted over others.
fn overlay(color: Hsla, base: Rgba) -> Hsla {
    if color.a < 1. {
        return color;
    }
    let target = color.to_rgb();
    let channels = [(target.r, base.r), (target.g, base.g), (target.b, base.b)];
    // Each channel needs at least this much of the overlay to reach the target from the base,
    // with the overlay's channel at 1 (to lighten) or 0 (to darken).
    let alpha = channels
        .iter()
        .map(|&(target, base)| {
            if target > base {
                (target - base) / (1. - base)
            } else if target < base {
                (base - target) / base
            } else {
                0.
            }
        })
        .fold(0., f32::max);
    if alpha <= f32::EPSILON {
        return gpui::transparent_black();
    }
    let channel = |target: f32, base: f32| ((target - base * (1. - alpha)) / alpha).clamp(0., 1.);
    Rgba {
        r: channel(target.r, base.r),
        g: channel(target.g, base.g),
        b: channel(target.b, base.b),
        a: alpha,
    }
    .into()
}

/// Copies the picture at `source` and shows it behind the window, deleting the last one's
/// copies. Done when it's shown, or with why it couldn't be.
pub fn choose_picture(source: PathBuf, cx: &mut App) -> gpui::Task<Result<()>> {
    let app_settings = crate::app_settings::AppSettingsStore::global(cx);
    cx.spawn(async move |cx| {
        let name = cx
            .background_spawn(async move { import_picture(&source).await })
            .await?;
        let previous = app_settings.update(cx, |store, cx| {
            let previous = store.settings().background_picture.clone();
            store.update(
                |settings| {
                    settings.window_background = WindowBackground::Picture;
                    settings.background_picture = Some(name.clone());
                },
                cx,
            );
            previous
        });
        if let Some(previous) = previous {
            cx.update(|cx| {
                forget_picture(picture_path(&previous, false), cx);
                forget_picture(picture_path(&previous, true), cx);
            });
        }
        cx.background_spawn(async move { remove_other_pictures(Some(&name)).log_err() })
            .detach();
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use gpui::{Hsla, Rgba, rgb};

    use super::{
        PICTURE_SIZE, SOFT_PICTURE_SIZE, import_picture_into, overlay, picture_path_in,
        remove_other_pictures_in,
    };

    /// A picture larger than a window is copied scaled down, once sharp and once small for
    /// Soften picture, and choosing another deletes the first one's copies.
    #[test]
    fn pictures_are_copied_scaled_down() {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let source = directory.path().join("wide.png");
        image::RgbImage::from_fn(5000, 1000, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 90])
        })
        .save(&source)
        .expect("the picture saves");
        let pictures = directory.path().join("backgrounds");

        let first =
            smol::block_on(import_picture_into(&source, &pictures)).expect("the picture is copied");
        let sharp = image::open(picture_path_in(&pictures, &first, false)).expect("a sharp copy");
        assert_eq!((sharp.width(), sharp.height()), (PICTURE_SIZE, 768));
        let soft = image::open(picture_path_in(&pictures, &first, true)).expect("a soft copy");
        assert_eq!(soft.width(), SOFT_PICTURE_SIZE);

        std::thread::sleep(std::time::Duration::from_millis(2));
        let second =
            smol::block_on(import_picture_into(&source, &pictures)).expect("the picture is copied");
        assert_ne!(first, second);
        remove_other_pictures_in(&pictures, Some(&second)).expect("the old copies are deleted");
        assert!(!picture_path_in(&pictures, &first, false).exists());
        assert!(!picture_path_in(&pictures, &first, true).exists());
        assert!(picture_path_in(&pictures, &second, false).exists());
        assert!(picture_path_in(&pictures, &second, true).exists());

        let text = directory.path().join("notes.txt");
        std::fs::write(&text, "not a picture").expect("the file saves");
        assert!(smol::block_on(import_picture_into(&text, &pictures)).is_err());
    }

    /// `top` painted over `base`, as the renderer blends them.
    fn over(top: Hsla, base: Rgba) -> Rgba {
        let top = top.to_rgb();
        let mix =
            |top_channel: f32, base_channel: f32| top_channel * top.a + base_channel * (1. - top.a);
        Rgba {
            r: mix(top.r, base.r),
            g: mix(top.g, base.g),
            b: mix(top.b, base.b),
            a: 1.,
        }
    }

    /// Painted over the base, a surface's overlay shows as the surface's own color, and the
    /// base's own color adds nothing.
    #[test]
    fn overlays_show_as_their_color_over_the_editor() {
        // JetBrains Dark's editor, under its panel, a selected row, itself, black, white and its
        // accent.
        let editor = rgb(0x1e1f22);
        for color in [0x26282b, 0x43454a, 0x1e1f22, 0x000000, 0xffffff, 0x3474f0] {
            let shown = over(overlay(Hsla::from(rgb(color)), editor), editor);
            let expected = rgb(color);
            for (shown, expected) in [
                (shown.r, expected.r),
                (shown.g, expected.g),
                (shown.b, expected.b),
            ] {
                assert!(
                    (shown - expected).abs() < 0.002,
                    "{color:06x} shows as {shown} for {expected}"
                );
            }
        }
        assert_eq!(overlay(Hsla::from(editor), editor).a, 0.);
        // A color made to be painted over others is kept.
        let hover = Hsla::from(rgb(0xffffff)).opacity(0.1);
        assert_eq!(overlay(hover, editor), hover);
    }
}
