//! Loads theme JSON files (the format used by `assets/themes`) into the [`ThemeRegistry`].
//!
//! Ported from Zed's `settings_content::theme` and `theme_settings` crates without the
//! settings store, so themes can be loaded on their own.

mod content;
mod schema;

use std::sync::Arc;

use anyhow::{Context as _, Result};
use gpui::{
    FontFeatures, FontStyle, FontWeight, HighlightStyle, Refineable, WindowBackgroundAppearance,
};
use serde::Serializer;
use theme::{
    AccentColors, Appearance, AppearanceContent, PlayerColor, PlayerColors, StatusColors,
    SyntaxTheme, SystemColors, Theme, ThemeColors, ThemeFamily, ThemeRegistry, ThemeStyles,
    default_color_scales, try_parse_color,
};
use util::ResultExt as _;

pub use crate::content::*;
pub use crate::schema::{
    ThemeContent, ThemeFamilyContent, status_colors_refinement, syntax_overrides,
    theme_colors_refinement,
};

/// Converts theme content types into their GPUI equivalents.
pub trait IntoGpui {
    type Output;
    fn into_gpui(self) -> Self::Output;
}

impl IntoGpui for FontStyleContent {
    type Output = FontStyle;

    fn into_gpui(self) -> Self::Output {
        match self {
            FontStyleContent::Normal => FontStyle::Normal,
            FontStyleContent::Italic => FontStyle::Italic,
            FontStyleContent::Oblique => FontStyle::Oblique,
        }
    }
}

impl IntoGpui for FontWeightContent {
    type Output = FontWeight;

    fn into_gpui(self) -> Self::Output {
        FontWeight(self.0.clamp(100., 950.))
    }
}

impl IntoGpui for FontFeaturesContent {
    type Output = FontFeatures;

    fn into_gpui(self) -> Self::Output {
        FontFeatures(Arc::new(self.0.into_iter().collect()))
    }
}

impl IntoGpui for WindowBackgroundContent {
    type Output = WindowBackgroundAppearance;

    fn into_gpui(self) -> Self::Output {
        match self {
            WindowBackgroundContent::Opaque => WindowBackgroundAppearance::Opaque,
            WindowBackgroundContent::Transparent => WindowBackgroundAppearance::Transparent,
            WindowBackgroundContent::Blurred => WindowBackgroundAppearance::Blurred,
            WindowBackgroundContent::MicaBackdrop => WindowBackgroundAppearance::MicaBackdrop,
            WindowBackgroundContent::MicaAltBackdrop => WindowBackgroundAppearance::MicaAltBackdrop,
        }
    }
}

pub(crate) mod fallible_options {
    use serde::Deserialize as _;

    /// Used by `#[with_fallible_options]`: a malformed optional value falls back to `None`
    /// instead of failing the whole theme.
    pub(crate) fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
    where
        D: serde::Deserializer<'de>,
        T: serde::de::DeserializeOwned + Default,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        match serde_json::from_value(value) {
            Ok(parsed) => Ok(parsed),
            Err(error) => {
                log::warn!("ignoring invalid theme value: {error}");
                Ok(T::default())
            }
        }
    }
}

pub fn serialize_f32_with_two_decimal_places<S>(
    value: &f32,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let rounded = (value * 100.0).round() / 100.0;
    let formatted = format!("{:.2}", rounded);
    let clean_value: f64 = formatted.parse().unwrap_or(rounded as f64);
    serializer.serialize_f64(clean_value)
}

/// Loads the themes bundled with the app's assets into the registry.
pub fn load_bundled_themes(registry: &ThemeRegistry) {
    let theme_paths = registry
        .assets()
        .list("themes/")
        .expect("failed to list theme assets")
        .into_iter()
        .filter(|path| path.ends_with(".json"));

    for path in theme_paths {
        let Some(theme) = registry.assets().load(&path).log_err().flatten() else {
            continue;
        };

        let Some(theme_family) = serde_json::from_slice(&theme)
            .with_context(|| format!("failed to parse theme at path \"{path}\""))
            .log_err()
        else {
            continue;
        };

        let refined = refine_theme_family(theme_family);
        registry.insert_theme_families([refined]);
    }
}

/// Loads a user theme from the given bytes into the registry.
pub fn load_user_theme(registry: &ThemeRegistry, bytes: &[u8]) -> Result<()> {
    let theme = deserialize_user_theme(bytes)?;
    let refined = refine_theme_family(theme);
    registry.insert_theme_families([refined]);
    Ok(())
}

/// Deserializes a user theme from the given bytes.
pub fn deserialize_user_theme(bytes: &[u8]) -> Result<ThemeFamilyContent> {
    let theme_family: ThemeFamilyContent = serde_json_lenient::from_slice(bytes)?;

    for theme in &theme_family.themes {
        if theme
            .style
            .colors
            .deprecated_scrollbar_thumb_background
            .is_some()
        {
            log::warn!(
                r#"Theme "{theme_name}" is using a deprecated style property: scrollbar_thumb.background. Use `scrollbar.thumb.background` instead."#,
                theme_name = theme.name
            )
        }
    }

    Ok(theme_family)
}

/// Refines a [`ThemeFamilyContent`] and its [`ThemeContent`]s into a [`ThemeFamily`].
pub fn refine_theme_family(theme_family_content: ThemeFamilyContent) -> ThemeFamily {
    let id = uuid::Uuid::new_v4().to_string();
    let name = theme_family_content.name.clone();
    let author = theme_family_content.author.clone();

    let themes: Vec<Theme> = theme_family_content
        .themes
        .iter()
        .map(|theme_content| refine_theme(theme_content))
        .collect();

    ThemeFamily {
        id,
        name: name.into(),
        author: author.into(),
        themes,
        scales: default_color_scales(),
    }
}

/// Refines a [`ThemeContent`] into a [`Theme`].
pub fn refine_theme(theme: &ThemeContent) -> Theme {
    let appearance = match theme.appearance {
        AppearanceContent::Light => Appearance::Light,
        AppearanceContent::Dark => Appearance::Dark,
    };

    let mut refined_status_colors = match theme.appearance {
        AppearanceContent::Light => StatusColors::light(),
        AppearanceContent::Dark => StatusColors::dark(),
    };
    let mut status_colors_refinement = status_colors_refinement(&theme.style.status);
    theme::apply_status_color_defaults(&mut status_colors_refinement);
    refined_status_colors.refine(&status_colors_refinement);

    let mut refined_player_colors = match theme.appearance {
        AppearanceContent::Light => PlayerColors::light(),
        AppearanceContent::Dark => PlayerColors::dark(),
    };
    merge_player_colors(&mut refined_player_colors, &theme.style.players);

    let mut refined_theme_colors = match theme.appearance {
        AppearanceContent::Light => ThemeColors::light(),
        AppearanceContent::Dark => ThemeColors::dark(),
    };
    let mut theme_colors_refinement = theme_colors_refinement(
        &theme.style.colors,
        &status_colors_refinement,
        theme.appearance == AppearanceContent::Light,
    );
    theme::apply_theme_color_defaults(&mut theme_colors_refinement, &refined_player_colors);
    refined_theme_colors.refine(&theme_colors_refinement);

    let mut refined_accent_colors = match theme.appearance {
        AppearanceContent::Light => AccentColors::light(),
        AppearanceContent::Dark => AccentColors::dark(),
    };
    merge_accent_colors(&mut refined_accent_colors, &theme.style.accents);

    let syntax_highlights = theme.style.syntax.iter().map(|(syntax_token, highlight)| {
        (
            syntax_token.clone(),
            HighlightStyle {
                color: highlight
                    .color
                    .as_ref()
                    .and_then(|color| try_parse_color(color).ok()),
                background_color: highlight
                    .background_color
                    .as_ref()
                    .and_then(|color| try_parse_color(color).ok()),
                font_style: highlight.font_style.map(|s| s.into_gpui()),
                font_weight: highlight.font_weight.map(|w| w.into_gpui()),
                ..Default::default()
            },
        )
    });
    let syntax_theme = Arc::new(SyntaxTheme::new(syntax_highlights));

    let window_background_appearance = theme
        .style
        .window_background_appearance
        .map(|w| w.into_gpui())
        .unwrap_or_default();

    Theme {
        id: uuid::Uuid::new_v4().to_string(),
        name: theme.name.clone().into(),
        appearance,
        styles: ThemeStyles {
            system: SystemColors::default(),
            window_background_appearance,
            accents: refined_accent_colors,
            colors: refined_theme_colors,
            status: refined_status_colors,
            player: refined_player_colors,
            syntax: syntax_theme,
        },
    }
}

/// Merges player color overrides into the given [`PlayerColors`].
pub fn merge_player_colors(
    player_colors: &mut PlayerColors,
    user_player_colors: &[crate::content::PlayerColorContent],
) {
    if user_player_colors.is_empty() {
        return;
    }

    for (idx, player) in user_player_colors.iter().enumerate() {
        let cursor = player
            .cursor
            .as_ref()
            .and_then(|color| try_parse_color(color).ok());
        let background = player
            .background
            .as_ref()
            .and_then(|color| try_parse_color(color).ok());
        let selection = player
            .selection
            .as_ref()
            .and_then(|color| try_parse_color(color).ok());

        if let Some(player_color) = player_colors.0.get_mut(idx) {
            *player_color = PlayerColor {
                cursor: cursor.unwrap_or(player_color.cursor),
                background: background.unwrap_or(player_color.background),
                selection: selection.unwrap_or(player_color.selection),
            };
        } else {
            player_colors.0.push(PlayerColor {
                cursor: cursor.unwrap_or_default(),
                background: background.unwrap_or_default(),
                selection: selection.unwrap_or_default(),
            });
        }
    }
}

/// Merges accent color overrides into the given [`AccentColors`].
pub fn merge_accent_colors(
    accent_colors: &mut AccentColors,
    user_accent_colors: &[crate::content::AccentContent],
) {
    if user_accent_colors.is_empty() {
        return;
    }

    let colors = user_accent_colors
        .iter()
        .filter_map(|accent_color| {
            accent_color
                .0
                .as_ref()
                .and_then(|color| try_parse_color(color).ok())
        })
        .collect::<Vec<_>>();

    if !colors.is_empty() {
        accent_colors.0 = Arc::from(colors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_themes_parse() {
        let themes_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/themes");
        let mut theme_names = Vec::new();
        for entry in std::fs::read_dir(&themes_dir).expect("themes directory exists") {
            let family_dir = entry.expect("readable entry").path();
            let Some(family_name) = family_dir.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let json_path = family_dir.join(format!("{family_name}.json"));
            if !json_path.exists() {
                continue;
            }
            let bytes = std::fs::read(&json_path).expect("readable theme file");
            let family = refine_theme_family(
                deserialize_user_theme(&bytes)
                    .unwrap_or_else(|error| panic!("{}: {error:#}", json_path.display())),
            );
            theme_names.extend(
                family
                    .themes
                    .into_iter()
                    .map(|theme| theme.name.to_string()),
            );
        }
        assert!(theme_names.iter().any(|name| name == "One Dark"));
        assert!(theme_names.iter().any(|name| name == "Gruvbox Dark"));
        assert!(theme_names.len() >= 10, "{theme_names:?}");
    }
}
