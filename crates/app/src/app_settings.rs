//! App-wide preferences, saved as JSON in the data directory. For now that's the theme, picked
//! the way Zed picks it: a mode, plus one theme for light appearance and one for dark.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context as _, Result};
use gpui::{App, AppContext as _, Context, Entity, Global, Task, WindowAppearance};
use serde::{Deserialize, Serialize};
use theme::{ActiveTheme as _, DEFAULT_DARK_THEME, GlobalTheme, ThemeRegistry};
use util::ResultExt as _;

const DEFAULT_LIGHT_THEME: &str = "One Light";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    /// Follow macOS's light or dark appearance.
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub theme_mode: ThemeMode,
    pub light_theme: String,
    pub dark_theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::default(),
            light_theme: DEFAULT_LIGHT_THEME.to_string(),
            dark_theme: DEFAULT_DARK_THEME.to_string(),
        }
    }
}

pub struct AppSettingsStore {
    settings: AppSettings,
    path: PathBuf,
    _save: Option<Task<()>>,
}

struct GlobalAppSettings(Entity<AppSettingsStore>);

impl Global for GlobalAppSettings {}

/// Loads the settings and applies the theme. Call after the themes are registered.
pub fn init(cx: &mut App) {
    let path = paths::settings_file();
    let settings = read_settings(&path).log_err().flatten().unwrap_or_default();
    apply_theme(&settings, cx);
    let store = cx.new(|_| AppSettingsStore {
        settings,
        path,
        _save: None,
    });
    cx.set_global(GlobalAppSettings(store));
}

impl AppSettingsStore {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalAppSettings>().0.clone()
    }

    pub fn settings(&self) -> &AppSettings {
        &self.settings
    }

    pub fn update(&mut self, change: impl FnOnce(&mut AppSettings), cx: &mut Context<Self>) {
        let previous = self.settings.clone();
        change(&mut self.settings);
        if self.settings == previous {
            return;
        }
        apply_theme(&self.settings, cx);
        let path = self.path.clone();
        let settings = self.settings.clone();
        self._save = Some(cx.background_spawn(async move {
            write_settings(&path, &settings).log_err();
        }));
        cx.notify();
    }

    /// Re-applies the theme, for when macOS switches between light and dark.
    pub fn reapply_theme(&self, cx: &mut App) {
        apply_theme(&self.settings, cx);
    }
}

fn apply_theme(settings: &AppSettings, cx: &mut App) {
    let is_dark = match settings.theme_mode {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::System => matches!(
            cx.window_appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    };
    let (name, fallback) = if is_dark {
        (&settings.dark_theme, DEFAULT_DARK_THEME)
    } else {
        (&settings.light_theme, DEFAULT_LIGHT_THEME)
    };
    let registry = ThemeRegistry::global(cx);
    match registry.get(name).or_else(|_| registry.get(fallback)) {
        Ok(theme) => {
            // Compared by identity rather than name: at startup the active theme is the
            // built-in fallback, which shares its name with the bundled "One Dark".
            if !Arc::ptr_eq(cx.theme(), &theme) {
                GlobalTheme::update_theme(cx, theme);
                cx.refresh_windows();
            }
        }
        Err(error) => log::error!("{error:#}"),
    }
}

fn read_settings(path: &std::path::Path) -> Result<Option<AppSettings>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let settings =
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(settings))
}

fn write_settings(path: &std::path::Path, settings: &AppSettings) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let json = serde_json::to_vec_pretty(settings)?;
    std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))
}
