mod agent_view;
mod new_thread_modal;
mod project_info;
mod project_switcher;
mod shell;
mod sidebar;

use std::io::IsTerminal as _;
use std::sync::Arc;

use assets::Assets;
use futures::FutureExt as _;
use gpui::{
    App, Bounds, Focusable as _, Font, KeyBinding, Menu, MenuItem, Pixels, TitlebarOptions,
    WindowBounds, WindowOptions, actions, point, px, size,
};
use projects::ProjectStore;
use registry::AgentRegistryStore;
use reqwest_client::ReqwestClient;
use theme::{
    DEFAULT_DARK_THEME, GlobalTheme, LoadThemes, ThemeRegistry, ThemeSettingsProvider, UiDensity,
};
use ui::prelude::*;

use crate::shell::Shell;

actions!(
    agentz,
    [
        /// Quits the application.
        Quit,
        /// Adds one or more folders as projects.
        OpenFolder,
        /// Opens the project switcher in the title bar.
        ToggleProjectSwitcher,
        /// Starts a new thread in the selected project.
        NewThread,
    ]
);

const UI_FONT_FAMILY: &str = "IBM Plex Sans";
const MONO_FONT_FAMILY: &str = "Lilex";

struct AppThemeSettings {
    ui_font: Font,
    buffer_font: Font,
}

impl ThemeSettingsProvider for AppThemeSettings {
    fn ui_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.ui_font
    }

    fn buffer_font<'a>(&'a self, _cx: &'a App) -> &'a Font {
        &self.buffer_font
    }

    fn ui_font_size(&self, _cx: &App) -> Pixels {
        px(14.)
    }

    fn buffer_font_size(&self, _cx: &App) -> Pixels {
        px(13.5)
    }

    fn ui_density(&self, _cx: &App) -> UiDensity {
        UiDensity::Default
    }
}

fn init_theme(cx: &mut App) {
    theme::init(LoadThemes::All(Box::new(Assets)), cx);
    let registry = ThemeRegistry::global(cx);
    theme_json::load_bundled_themes(&registry);
    match registry.get(DEFAULT_DARK_THEME) {
        Ok(theme) => GlobalTheme::update_theme(cx, theme),
        Err(error) => log::error!("{error:#}"),
    }
    theme::set_theme_settings_provider(
        Box::new(AppThemeSettings {
            ui_font: gpui::font(UI_FONT_FAMILY),
            buffer_font: gpui::font(MONO_FONT_FAMILY),
        }),
        cx,
    );
}

fn init_actions(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.bind_keys([
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-o", OpenFolder, None),
        KeyBinding::new("secondary-alt-o", ToggleProjectSwitcher, None),
        KeyBinding::new("secondary-n", NewThread, None),
    ]);
    cx.set_menus([
        Menu::new("agentZ").items([MenuItem::action("Quit agentZ", Quit)]),
        Menu::new("File").items([
            MenuItem::action("New Thread…", NewThread),
            MenuItem::action("Open Folder…", OpenFolder),
            MenuItem::action("Switch Project…", ToggleProjectSwitcher),
        ]),
    ]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}

/// Loads the login shell's environment (notably `PATH`) when the app was not started from a
/// terminal, so agents and `npm` resolve the same way they do in the user's shell.
fn load_shell_environment(cx: &App) -> registry::ShellEnvironmentReady {
    let started_from_terminal = std::io::stdout().is_terminal();
    cx.background_executor()
        .spawn(async move {
            #[cfg(unix)]
            if !started_from_terminal && let Err(error) = util::load_login_shell_environment().await
            {
                log::error!("failed to load the login shell environment: {error:#}");
            }
        })
        .shared()
}

fn main() {
    // The login-shell environment capture re-runs this binary with `--printenv`.
    if std::env::args().any(|argument| argument == "--printenv") {
        util::shell_env::print_env();
        return;
    }
    env_logger::init();

    let http_client = match ReqwestClient::user_agent(concat!("agentZ/", env!("CARGO_PKG_VERSION")))
    {
        Ok(client) => Arc::new(client),
        Err(error) => {
            log::error!("failed to create the HTTP client: {error:#}");
            Arc::new(ReqwestClient::new())
        }
    };

    gpui_platform::application()
        .with_assets(Assets)
        .with_http_client(http_client.clone())
        .run(move |cx: &mut App| {
            if let Err(error) = Assets.load_fonts(cx) {
                log::error!("failed to load fonts: {error:#}");
            }
            init_theme(cx);
            text_input::init(cx);
            project_switcher::init(cx);
            new_thread_modal::init(cx);
            agent_view::init(cx);
            sidebar::init(cx);
            projects::init(cx);
            let shell_environment_ready = load_shell_environment(cx);
            registry::init(http_client, shell_environment_ready, cx);
            init_actions(cx);

            let store = ProjectStore::global(cx);
            let registry = AgentRegistryStore::global(cx);
            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            let window = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: None,
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(12.), px(12.))),
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let shell = cx.new(|cx| Shell::new(store, registry, window, cx));
                    window.focus(&shell.focus_handle(cx), cx);
                    shell
                },
            );
            if let Err(error) = window {
                log::error!("failed to open window: {error:#}");
                cx.quit();
                return;
            }
            cx.activate(true);
        });
}
