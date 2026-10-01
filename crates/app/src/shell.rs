use gpui::{
    App, Context, Entity, FocusHandle, Focusable, MouseButton, PathPromptOptions, Subscription,
    Window, WindowControlArea,
};
use projects::{ProjectScope, ProjectStore};
use ui::{ButtonLike, PopoverMenu, PopoverMenuHandle, Tooltip, prelude::*};

use crate::project_switcher::ProjectSwitcher;
use crate::sidebar::{SIDEBAR_WIDTH, Sidebar};
use crate::{OpenFolder, ToggleProjectSwitcher};

const TITLE_BAR_HEIGHT: Pixels = px(40.);
/// Leaves room for the macOS traffic lights.
const TRAFFIC_LIGHTS_WIDTH: Pixels = px(80.);

pub struct Shell {
    focus_handle: FocusHandle,
    store: Entity<ProjectStore>,
    sidebar: Entity<Sidebar>,
    switcher_handle: PopoverMenuHandle<ProjectSwitcher>,
    should_move_window: bool,
    _subscription: Subscription,
}

impl Shell {
    pub fn new(store: Entity<ProjectStore>, cx: &mut Context<Self>) -> Self {
        let sidebar = cx.new(|cx| Sidebar::new(store.clone(), cx));
        let subscription = cx.observe(&store, |_, _, cx| cx.notify());
        Self {
            focus_handle: cx.focus_handle(),
            store,
            sidebar,
            switcher_handle: PopoverMenuHandle::default(),
            should_move_window: false,
            _subscription: subscription,
        }
    }

    fn open_folder(&mut self, _: &OpenFolder, _: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: true,
            prompt: Some("Open".into()),
        });
        let store = self.store.clone();
        cx.spawn(async move |_, cx| {
            let paths = match paths.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(error)) => {
                    log::error!("failed to pick folders: {error:#}");
                    return;
                }
            };
            store.update(cx, |store, cx| {
                let mut last_added = None;
                for path in paths {
                    last_added = Some(store.add_project(path, cx));
                }
                // In "All projects" the new project simply appears; otherwise switch to it so
                // it doesn't get added out of sight.
                if let Some(id) = last_added
                    && store.scope() != ProjectScope::All
                {
                    store.set_scope(ProjectScope::Project(id), cx);
                }
            });
        })
        .detach();
    }

    fn toggle_project_switcher(
        &mut self,
        _: &ToggleProjectSwitcher,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.switcher_handle.toggle(window, cx);
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let store = self.store.read(cx);
        let scope_label: SharedString = match store.scope() {
            ProjectScope::All => "All projects".into(),
            ProjectScope::Project(id) => store
                .project(id)
                .map(|project| project.name())
                .unwrap_or_else(|| "All projects".into()),
        };
        let switcher_store = self.store.clone();

        h_flex()
            .id("title-bar")
            .window_control_area(WindowControlArea::Drag)
            .h(TITLE_BAR_HEIGHT)
            .flex_none()
            .w_full()
            .pl(TRAFFIC_LIGHTS_WIDTH)
            .pr_3()
            .gap_2()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.title_bar_background)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.should_move_window = true),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.should_move_window = false),
            )
            .on_mouse_down_out(cx.listener(|this, _, _, _| this.should_move_window = false))
            .on_mouse_move(cx.listener(|this, _, window, _| {
                if this.should_move_window {
                    this.should_move_window = false;
                    window.start_window_move();
                }
            }))
            .on_click(|event, window, _| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
            .child(
                // Keeps a press on the switcher from starting a window drag.
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        PopoverMenu::new("project-switcher")
                            .with_handle(self.switcher_handle.clone())
                            .menu(move |window, cx| {
                                let store = switcher_store.clone();
                                Some(cx.new(|cx| ProjectSwitcher::new(store, window, cx)))
                            })
                            .trigger_with_tooltip(
                                ButtonLike::new("project-switcher-trigger")
                                    .style(ButtonStyle::Outlined)
                                    .child(
                                        h_flex()
                                            .px_1()
                                            .gap_1p5()
                                            .child(
                                                Icon::new(IconName::Folder)
                                                    .size(IconSize::Small)
                                                    .color(Color::Muted),
                                            )
                                            .child(Label::new(scope_label))
                                            .child(
                                                Icon::new(IconName::ChevronDown)
                                                    .size(IconSize::XSmall)
                                                    .color(Color::Muted),
                                            ),
                                    ),
                                |_, cx| {
                                    Tooltip::for_action(
                                        "Switch Project",
                                        &ToggleProjectSwitcher,
                                        cx,
                                    )
                                },
                            )
                            .anchor(gpui::Anchor::TopLeft)
                            .offset(gpui::point(px(0.), px(4.))),
                    ),
            )
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().colors().background;
        let text_color = cx.theme().colors().text;
        let main_background = cx.theme().colors().editor_background;

        v_flex()
            .key_context("Shell")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::open_folder))
            .on_action(cx.listener(Self::toggle_project_switcher))
            .size_full()
            .bg(background)
            .text_color(text_color)
            .font_ui(cx)
            .text_ui(cx)
            .child(self.render_title_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar.clone())
                    .child(
                        div()
                            .flex_1()
                            .min_w(SIDEBAR_WIDTH)
                            .h_full()
                            .bg(main_background),
                    ),
            )
    }
}
