//! App-wide preferences, saved as JSON in the data directory: the theme, picked the way Zed
//! picks it (a mode, plus one theme for light appearance and one for dark), the saved machines,
//! and which projects the sidebar shows.
//!
//! Agent settings belong to each machine's server; its client keeps the app's copy.

use std::path::PathBuf;
use std::sync::Arc;

use agentz_protocol::spaces::LayoutNode;
use anyhow::{Context as _, Result};
use gpui::{App, AppContext as _, Context, Entity, Global, Task, WindowAppearance};
use serde::{Deserialize, Serialize};
use theme::{ActiveTheme as _, DEFAULT_DARK_THEME, GlobalTheme, ThemeRegistry};
use util::ResultExt as _;

use crate::machines::{GroupKey, ProjectGroupingMode, Scope};

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

/// Zed's `PlaySoundWhenAgentDone`, set for each sound, with a choice for another app in front.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaySound {
    Never,
    /// While the user can't see the agent: another thread or Settings is open, or another app
    /// is in front. Zed's `when_hidden`, which older settings files still say.
    #[serde(alias = "when_hidden")]
    WhenInAnotherThread,
    /// Only while another app is in front.
    WhenInAnotherApp,
    Always,
}

impl PlaySound {
    pub const ALL: [Self; 4] = [
        Self::Never,
        Self::WhenInAnotherThread,
        Self::WhenInAnotherApp,
        Self::Always,
    ];

    /// `is_visible` is whether the user can see the agent, and `is_window_active` whether
    /// agentZ is in front.
    pub fn should_play(self, is_visible: bool, is_window_active: bool) -> bool {
        match self {
            Self::Never => false,
            Self::WhenInAnotherThread => !is_visible,
            Self::WhenInAnotherApp => !is_window_active,
            Self::Always => true,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Never => "Never",
            Self::WhenInAnotherThread => "When in another thread",
            Self::WhenInAnotherApp => "When in another app",
            Self::Always => "Always",
        }
    }
}

/// A machine reached over SSH (herdr's saved endpoints). No secrets: authentication stays with
/// OpenSSH.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineProfile {
    pub id: u64,
    /// The name shown for it. Empty shows the target.
    #[serde(default)]
    pub label: String,
    /// What to give `ssh`: a host, an alias from `~/.ssh/config`, `user@host`, or
    /// `ssh://user@host:port`.
    pub target: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl MachineProfile {
    pub fn display_label(&self) -> String {
        if self.label.trim().is_empty() {
            self.target.clone()
        } else {
            self.label.trim().to_string()
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub theme_mode: ThemeMode,
    pub light_theme: String,
    pub dark_theme: String,
    pub machines: Vec<MachineProfile>,
    pub scope: Scope,
    /// How checkouts of one repository are combined in the projects list.
    pub project_grouping: ProjectGroupingMode,
    /// Per project (by [`GroupKey::of_project`]), in place of `project_grouping`.
    pub project_grouping_overrides: std::collections::BTreeMap<GroupKey, ProjectGroupingMode>,
    /// The combining mode General's switch turns back on.
    pub last_combined_grouping: ProjectGroupingMode,
    /// Terminals' font size, as Cmd-+ and Cmd-- in a terminal left it. `None` is the
    /// default.
    pub terminal_font_size: Option<f32>,
    /// The sidebar was hidden with Cmd-B, in Agents and Workspaces alike.
    pub is_sidebar_hidden: bool,
    /// Tabs saved with Save Layout…, which open in any workspace on any machine.
    pub saved_layouts: Vec<SavedLayout>,
    /// Zed's `agent.use_modifier_to_send`: Cmd-Enter sends and Enter makes a new line.
    pub use_modifier_to_send: bool,
    /// Thoughts show open in threads: Zed's `thinking_display` as `always_expanded`, where
    /// otherwise it's `always_collapsed`.
    pub show_thinking: bool,
    /// Chats show in the sidebar, New Chat and search. Off, they're hidden and kept.
    pub chats: bool,
    pub play_sound_when_finished: PlaySound,
    /// When a permission request or a question arrives. Always by default, as herdr plays its
    /// request sound: it needs an answer either way.
    pub play_sound_when_input_needed: PlaySound,
    /// Both sounds' volume, from 0 (silent) to 1 (full), on top of the system's, as macOS's
    /// Alert volume.
    pub sound_volume: f32,
    /// System notifications show only while another app is in front, as t3code's do.
    pub notify_when_unfocused: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedLayout {
    pub name: String,
    pub layout: LayoutNode,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::default(),
            light_theme: DEFAULT_LIGHT_THEME.to_string(),
            dark_theme: DEFAULT_DARK_THEME.to_string(),
            machines: Vec::new(),
            scope: Scope::default(),
            project_grouping: ProjectGroupingMode::default(),
            project_grouping_overrides: Default::default(),
            last_combined_grouping: ProjectGroupingMode::default(),
            terminal_font_size: None,
            is_sidebar_hidden: false,
            use_modifier_to_send: false,
            show_thinking: false,
            chats: true,
            play_sound_when_finished: PlaySound::WhenInAnotherThread,
            play_sound_when_input_needed: PlaySound::Always,
            sound_volume: 1.,
            notify_when_unfocused: true,
            saved_layouts: Vec::new(),
        }
    }
}

impl AppSettings {
    /// Saves a new machine and returns its id.
    pub fn add_machine(&mut self, label: String, target: String) -> u64 {
        let id = self
            .machines
            .iter()
            .map(|machine| machine.id)
            .max()
            .map_or(1, |id| id + 1);
        self.machines.push(MachineProfile {
            id,
            label,
            target,
            enabled: true,
        });
        id
    }

    /// Saves a layout, in place of one with the same name.
    pub fn save_layout(&mut self, layout: SavedLayout) {
        match self
            .saved_layouts
            .iter_mut()
            .find(|saved| saved.name == layout.name)
        {
            Some(saved) => *saved = layout,
            None => self.saved_layouts.push(layout),
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

/// Default settings, never read from or saved to the user's file.
#[cfg(test)]
pub fn init_for_test(cx: &mut App) {
    let settings = AppSettings::default();
    apply_theme(&settings, cx);
    let store = cx.new(|_| AppSettingsStore {
        settings,
        path: std::env::temp_dir().join("agentz-test-settings.json"),
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

/// Whether Cmd-B hid the sidebar.
pub fn is_sidebar_hidden(cx: &App) -> bool {
    AppSettingsStore::global(cx)
        .read(cx)
        .settings()
        .is_sidebar_hidden
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;
    use theme::ThemeRegistry;

    use super::{AppSettings, PlaySound};

    /// A file from before the in-another-app choice and the volume keeps its sounds' choices,
    /// at full volume.
    #[test]
    fn older_sound_settings_load() {
        let settings: AppSettings = serde_json::from_value(serde_json::json!({
            "play_sound_when_finished": "when_hidden",
            "play_sound_when_input_needed": "never",
        }))
        .expect("the settings parse");
        assert_eq!(
            settings.play_sound_when_finished,
            PlaySound::WhenInAnotherThread
        );
        assert_eq!(settings.play_sound_when_input_needed, PlaySound::Never);
        assert_eq!(settings.sound_volume, 1.);

        let saved = serde_json::to_value(&settings).expect("the settings serialize");
        assert_eq!(saved["play_sound_when_finished"], "when_in_another_thread");
        assert_eq!(
            serde_json::to_value(PlaySound::WhenInAnotherApp).expect("a value"),
            "when_in_another_app"
        );
    }

    #[test]
    fn sounds_play_where_their_choice_says() {
        // (on the thread, in another thread, in another app)
        let places = |when: PlaySound| {
            (
                when.should_play(true, true),
                when.should_play(false, true),
                when.should_play(false, false),
            )
        };
        assert_eq!(places(PlaySound::Never), (false, false, false));
        assert_eq!(places(PlaySound::WhenInAnotherThread), (false, true, true));
        assert_eq!(places(PlaySound::WhenInAnotherApp), (false, false, true));
        assert_eq!(places(PlaySound::Always), (true, true, true));
    }

    /// A bundled theme that doesn't parse is only logged, and left out of the list.
    #[gpui::test]
    fn bundled_themes_load(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let registry = ThemeRegistry::global(cx);
            for name in [
                "One Dark",
                "One Light",
                "Ayu Dark",
                "Gruvbox Dark",
                "JetBrains Dark",
                "JetBrains Light",
                "Catppuccin Latte",
                "Catppuccin Frappé",
                "Catppuccin Macchiato",
                "Catppuccin Mocha",
            ] {
                assert!(registry.get(name).is_ok(), "{name} isn't loaded");
            }
        });
    }
}
