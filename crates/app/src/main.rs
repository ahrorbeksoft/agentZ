mod add_project_modal;
mod agent_icons;
mod agent_login;
mod agent_view;
mod app_settings;
mod command_palette;
mod confirm_dialog;
mod controls;
mod diff_panel;
mod elicitation_card;
mod go_to_picker;
mod login_item;
mod machine_icon_picker;
mod machine_modal;
mod machines;
mod mention_menu;
mod new_space_picker;
mod new_thread_modal;
mod project_info;
mod project_store;
mod project_switcher;
mod registry_store;
mod save_layout_modal;
mod server_client;
mod settings_page;
mod shell;
mod shortcut_sheet;
mod sidebar;
mod slide_drag;
mod sound;
mod spaces_view;
mod terminal_drawer;
mod terminal_element;
mod terminal_entity;
mod terminal_mouse;
mod terminal_thread_view;
mod terminal_view;
mod thread_entity;
mod welcome;
mod worktree_modal;

use std::sync::Arc;

use assets::Assets;
use gpui::{
    App, Bounds, Focusable as _, Font, KeyBinding, Menu, MenuItem, Pixels, TitlebarOptions,
    WindowBounds, WindowOptions, actions, point, px, size,
};
use reqwest_client::ReqwestClient;
use theme::{LoadThemes, ThemeRegistry, ThemeSettingsProvider, UiDensity};
use ui::prelude::*;
use util::ResultExt as _;

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
        /// Opens the settings page.
        OpenSettings,
        /// Shows or hides the open thread's changes.
        ToggleDiff,
        /// Shows or hides the sidebar.
        ToggleSidebar,
        /// Opens or closes the terminal under the open thread.
        ToggleTerminalDrawer,
        /// Lists the shortcuts for what's focused, and the app's.
        ShowShortcuts,
        /// Opens the command palette, to run what applies to what's focused.
        ToggleCommandPalette,
        /// Goes to a workspace, a tab, a pane or a thread.
        GoTo,
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
    theme::set_theme_settings_provider(
        Box::new(AppThemeSettings {
            ui_font: gpui::font(UI_FONT_FAMILY),
            buffer_font: gpui::font(MONO_FONT_FAMILY),
        }),
        cx,
    );
    app_settings::init(cx);
}

/// What the views need in a headless test, without the user's settings or servers.
#[cfg(test)]
fn init_for_test(cx: &mut App) {
    theme::init(LoadThemes::All(Box::new(Assets)), cx);
    theme_json::load_bundled_themes(&ThemeRegistry::global(cx));
    theme::set_theme_settings_provider(
        Box::new(AppThemeSettings {
            ui_font: gpui::font(UI_FONT_FAMILY),
            buffer_font: gpui::font(MONO_FONT_FAMILY),
        }),
        cx,
    );
    app_settings::init_for_test(cx);
    agent_icons::init(cx);
    text_input::init(cx);
    agent_view::init(cx);
    terminal_view::init(cx);
    terminal_thread_view::init(cx);
    spaces_view::init(cx);
    bind_keys(cx);
}

/// The shell's keys are bound in its context, not globally: GPUI ranks a binding without a
/// context above any view's own, so a global Cmd-D would take the key from Workspaces' Split
/// Right.
fn bind_keys(cx: &mut App) {
    let context = Some(shell::KEY_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-o", OpenFolder, context),
        KeyBinding::new("secondary-alt-o", ToggleProjectSwitcher, context),
        KeyBinding::new("secondary-n", NewThread, context),
        KeyBinding::new("secondary-,", OpenSettings, context),
        KeyBinding::new("secondary-d", ToggleDiff, context),
        // Zed's key for its left dock.
        KeyBinding::new("secondary-b", ToggleSidebar, context),
        KeyBinding::new("secondary-j", ToggleTerminalDrawer, context),
        // herdr's help is prefix-?; Cmd-? is macOS's Help menu search.
        KeyBinding::new("secondary-/", ShowShortcuts, context),
        // Zed's keys: its command palette, and its file finder for places.
        KeyBinding::new("secondary-shift-p", ToggleCommandPalette, context),
        KeyBinding::new("secondary-p", GoTo, context),
    ]);
}

fn init_actions(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| cx.quit());
    bind_keys(cx);
    cx.set_menus([
        Menu::new("agentZ").items([
            MenuItem::action("Settings…", OpenSettings),
            MenuItem::separator(),
            MenuItem::action("Quit agentZ", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Thread…", NewThread),
            MenuItem::action("Open Folder…", OpenFolder),
            MenuItem::action("Switch Project…", ToggleProjectSwitcher),
        ]),
        Menu::new("View").items([
            MenuItem::action("Sidebar", ToggleSidebar),
            MenuItem::action("Changes", ToggleDiff),
            MenuItem::action("Terminal", ToggleTerminalDrawer),
        ]),
    ]);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}

fn main() {
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
        .with_http_client(http_client)
        .run(|cx: &mut App| {
            if let Err(error) = Assets.load_fonts(cx) {
                log::error!("failed to load fonts: {error:#}");
            }
            init_theme(cx);
            text_input::init(cx);
            project_switcher::init(cx);
            new_thread_modal::init(cx);
            new_space_picker::init(cx);
            spaces_view::init(cx);
            add_project_modal::init(cx);
            worktree_modal::init(cx);
            machine_modal::init(cx);
            confirm_dialog::init(cx);
            shortcut_sheet::init(cx);
            command_palette::init(cx);
            go_to_picker::init(cx);
            save_layout_modal::init(cx);
            agent_view::init(cx);
            agent_login::init(cx);
            elicitation_card::init(cx);
            sidebar::init(cx);
            terminal_view::init(cx);
            terminal_thread_view::init(cx);
            settings_page::init(cx);
            agent_icons::init(cx);
            machines::init(cx);
            project_info::init(cx);
            init_actions(cx);
            cx.background_spawn(async { login_item::refresh().log_err() })
                .detach();

            let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
            let window = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: None,
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(12.), px(12.))),
                    }),
                    // The shell draws its own title bar and handles its drag and double-click,
                    // as Zed's does. Left to AppKit too, macOS 27 zooms the window on the
                    // double-click as well, so it zooms and immediately unzooms.
                    app_owns_titlebar_drag: true,
                    ..Default::default()
                },
                |window, cx| {
                    let shell = cx.new(|cx| Shell::new(window, cx));
                    window.focus(&shell.focus_handle(cx), cx);
                    shell
                },
            );
            if let Err(error) = window {
                log::error!("failed to open window: {error:#}");
                cx.quit();
                return;
            }
            // Launch Services brings a bundled app forward itself, except when it's opened in
            // the background (`open -g`). From a terminal, nothing else would.
            if !runs_from_bundle() {
                cx.activate(true);
            }
        });
}

fn runs_from_bundle() -> bool {
    std::env::current_exe().is_ok_and(|executable| {
        executable
            .parent()
            .is_some_and(|directory| directory.ends_with("Contents/MacOS"))
    })
}
