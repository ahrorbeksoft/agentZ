//! A thread's terminals: t3code's terminal drawer (`ThreadTerminalDrawer.tsx`) with Zed's
//! terminal panel's tabs in place of its groups and splits, as picked in
//! `design/thread-terminals/`. A strip across the top has a tab for each shell, named by what
//! runs in it, then New Terminal and Full Screen; one shell shows at a time. The tabs are this
//! window's; the server keeps the terminals running, and lists them so a restarted app finds
//! them again.

use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::{CAPABILITY_DRAWER_TERMINALS, Request, Response};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, PromptLevel, Subscription, Window,
};
use projects::ThreadId;
use ui::{Tooltip, prelude::*};

use crate::server_client::ServerClient;
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;

/// The strip's height, as in the design's mock.
const STRIP_HEIGHT: Pixels = px(30.);
/// Past this a tab's name is cut short: a long command line would push the others away.
const MAX_TAB_NAME_WIDTH: Pixels = px(160.);

pub enum TerminalDrawerEvent {
    ToggleFullScreen,
    /// Its last terminal closed.
    Empty,
}

struct DrawerTerminal {
    number: u32,
    view: Entity<TerminalView>,
    _exit: Subscription,
}

pub struct TerminalDrawer {
    client: Entity<ServerClient>,
    thread_id: ThreadId,
    terminals: Vec<DrawerTerminal>,
    active: u32,
    next_number: u32,
    is_full_screen: bool,
    /// The tabs name what runs in each shell, which the server reports in the projects.
    _programs: Subscription,
}

impl EventEmitter<TerminalDrawerEvent> for TerminalDrawer {}

impl Focusable for TerminalDrawer {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        match self.terminal(self.active) {
            Some(terminal) => terminal.view.focus_handle(cx),
            None => cx.focus_handle(),
        }
    }
}

impl TerminalDrawer {
    /// Opens with the first shell, then takes in the thread's other terminals still running on
    /// the server, each as a tab.
    pub fn new(client: Entity<ServerClient>, thread_id: ThreadId, cx: &mut Context<Self>) -> Self {
        let projects = client.read(cx).projects().clone();
        let mut this = Self {
            client: client.clone(),
            thread_id,
            terminals: Vec::new(),
            active: 1,
            next_number: 1,
            is_full_screen: false,
            _programs: cx.observe(&projects, |_, _, cx| cx.notify()),
        };
        this.add_terminal(cx);
        if this.has_several_terminals(cx) {
            let running = client.read(cx).request(Request::DrawerTerminals(thread_id));
            cx.spawn(async move |this, cx| {
                let Ok(Response::DrawerTerminals(numbers)) = running.await else {
                    return;
                };
                this.update(cx, |this, cx| {
                    let active = this.active;
                    for number in numbers {
                        if this.terminal(number).is_none() {
                            this.next_number = this.next_number.max(number);
                            this.add_terminal(cx);
                        }
                    }
                    this.active = active;
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        this
    }

    /// Whether the server can run more than the drawer's first terminal.
    fn has_several_terminals(&self, cx: &App) -> bool {
        self.client
            .read(cx)
            .has_capability(CAPABILITY_DRAWER_TERMINALS)
    }

    fn terminal(&self, number: u32) -> Option<&DrawerTerminal> {
        self.terminals
            .iter()
            .find(|terminal| terminal.number == number)
    }

    pub fn set_full_screen(&mut self, is_full_screen: bool, cx: &mut Context<Self>) {
        if self.is_full_screen != is_full_screen {
            self.is_full_screen = is_full_screen;
            cx.notify();
        }
    }

    /// Starts the next shell as the last tab and shows it. Returns its number.
    fn add_terminal(&mut self, cx: &mut Context<Self>) -> u32 {
        let number = self.next_number;
        self.next_number += 1;
        let key = TerminalKey::drawer(self.thread_id, number);
        let terminal = Terminal::shared(&self.client, key, cx);
        // A shell that exits closes its tab, as in t3code. One that had ended before the
        // drawer opened starts again instead, so opening the drawer always shows a shell.
        let mut was_running = false;
        let exit = cx.observe(&terminal, move |this, terminal, cx| {
            let Some(frame) = terminal.read(cx).frame() else {
                return;
            };
            match (frame.exited.is_some(), was_running) {
                (false, _) => was_running = true,
                (true, true) => this.close_terminal(number, cx),
                (true, false) => {
                    was_running = true;
                    terminal.update(cx, |terminal, cx| terminal.restart(cx));
                }
            }
        });
        let view = cx.new(|cx| TerminalView::new(terminal, TerminalMode::Scrollable, cx));
        self.terminals.push(DrawerTerminal {
            number,
            view,
            _exit: exit,
        });
        self.active = number;
        cx.notify();
        number
    }

    fn new_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_terminal(cx);
        self.focus_active(window, cx);
    }

    /// What runs in front of the shell, as the server reports it.
    fn running_program(&self, number: u32, cx: &App) -> Option<String> {
        self.client
            .read(cx)
            .projects()
            .read(cx)
            .drawer_commands(self.thread_id)
            .find(|(candidate, _)| *candidate == number)
            .map(|(_, command)| command.to_string())
    }

    /// A tab's × ends an idle shell at once, and asks first while a program runs in it, as
    /// closing a Workspaces pane does.
    fn request_close(&mut self, number: u32, window: &mut Window, cx: &mut Context<Self>) {
        let Some(program) = self.running_program(number, cx) else {
            self.close_terminal(number, cx);
            return;
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            &format!("Close “{program}”?"),
            Some("It's still running, and closing the terminal ends it."),
            &["Close", "Cancel"],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if answer.await == Ok(0) {
                this.update(cx, |this, cx| this.close_terminal(number, cx))
                    .ok();
            }
        })
        .detach();
    }

    /// Ends the terminal on the server. The next tab along shows, as in t3code.
    fn close_terminal(&mut self, number: u32, cx: &mut Context<Self>) {
        let Some(position) = self
            .terminals
            .iter()
            .position(|terminal| terminal.number == number)
        else {
            return;
        };
        self.terminals.remove(position);
        self.client.read(cx).send(
            Request::CloseTerminal(TerminalKey::drawer(self.thread_id, number)),
            cx,
        );
        if self.active == number {
            let next = position.min(self.terminals.len().saturating_sub(1));
            if let Some(terminal) = self.terminals.get(next) {
                self.active = terminal.number;
            }
        }
        if self.terminals.is_empty() {
            cx.emit(TerminalDrawerEvent::Empty);
        }
        cx.notify();
    }

    fn activate(&mut self, number: u32, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminal(number).is_some() {
            self.active = number;
            self.focus_active(window, cx);
            cx.notify();
        }
    }

    fn focus_active(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(terminal) = self.terminal(self.active) {
            window.focus(&terminal.view.focus_handle(cx), cx);
        }
    }

    /// A shell's tab: the terminal icon, what runs in it (as a Workspaces pane names it), and
    /// its ×, which shows on the tab in front and under the mouse, as Zed's do.
    fn render_tab(&self, number: u32, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let is_active = number == self.active;
        let color = if is_active {
            Color::Default
        } else {
            Color::Muted
        };
        let name = self
            .running_program(number, cx)
            .unwrap_or_else(|| "Shell".to_string());
        let group = SharedString::from(format!("drawer-tab-{number}"));
        h_flex()
            .id(("drawer-tab", number as usize))
            .debug_selector(|| format!("drawer-tab-{number}"))
            .group(group.clone())
            .h_full()
            .flex_none()
            .pl_2p5()
            .pr_1()
            .gap_1p5()
            .border_r_1()
            .border_color(colors.border_variant)
            .map(|tab| {
                if is_active {
                    tab.bg(colors.terminal_background)
                } else {
                    tab.border_b_1()
                        .cursor_pointer()
                        .hover(|tab| tab.bg(colors.ghost_element_hover))
                }
            })
            .child(
                Icon::new(IconName::Terminal)
                    .size(IconSize::XSmall)
                    .color(color),
            )
            .child(
                div().max_w(MAX_TAB_NAME_WIDTH).child(
                    Label::new(name)
                        .size(LabelSize::Small)
                        .color(color)
                        .truncate(),
                ),
            )
            .child(
                div()
                    .when(!is_active, |close| close.visible_on_hover(group))
                    .child(
                        IconButton::new(("drawer-close-tab", number as usize), IconName::Close)
                            .icon_size(IconSize::XSmall)
                            .icon_color(Color::Muted)
                            .tooltip(Tooltip::text("Close Terminal"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                cx.stop_propagation();
                                this.request_close(number, window, cx)
                            })),
                    ),
            )
            .on_click(cx.listener(move |this, _, window, cx| this.activate(number, window, cx)))
    }

    /// Zed's terminal panel's strip: the tabs, then New Terminal and Full Screen at its end.
    fn render_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_add = self.has_several_terminals(cx);
        let tabs = self
            .terminals
            .iter()
            .map(|terminal| self.render_tab(terminal.number, cx).into_any_element())
            .collect::<Vec<_>>();
        let colors = cx.theme().colors();
        h_flex()
            .h(STRIP_HEIGHT)
            .flex_none()
            .w_full()
            .bg(colors.panel_background)
            .child(
                h_flex()
                    .id("drawer-tabs")
                    .h_full()
                    .min_w_0()
                    .overflow_x_scroll()
                    .children(tabs),
            )
            // The tab in front has no bottom border, so it runs into its shell.
            .child(
                h_flex()
                    .flex_1()
                    .h_full()
                    .justify_end()
                    .gap_0p5()
                    .px_1p5()
                    .border_b_1()
                    .border_color(colors.border_variant)
                    .child(
                        IconButton::new("drawer-new-terminal", IconName::Plus)
                            .icon_size(IconSize::XSmall)
                            .disabled(!can_add)
                            .tooltip(Tooltip::text("New Terminal"))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.new_terminal(window, cx)),
                            ),
                    )
                    .child(
                        IconButton::new(
                            "drawer-full-screen",
                            if self.is_full_screen {
                                IconName::Minimize
                            } else {
                                IconName::Maximize
                            },
                        )
                        .icon_size(IconSize::XSmall)
                        .tooltip(Tooltip::text(if self.is_full_screen {
                            "Exit Full Screen"
                        } else {
                            "Full Screen"
                        }))
                        .on_click(
                            cx.listener(|_, _, _, cx| {
                                cx.emit(TerminalDrawerEvent::ToggleFullScreen)
                            }),
                        ),
                    ),
            )
    }
}

impl Render for TerminalDrawer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self
            .terminal(self.active)
            .map(|terminal| (terminal.number, terminal.view.clone()));
        v_flex()
            .size_full()
            .bg(cx.theme().colors().terminal_background)
            .child(self.render_strip(cx))
            .children(active.map(|(number, view)| {
                div()
                    .id(("drawer-terminal", number as usize))
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .pt_1()
                    .child(view)
            }))
    }
}

#[cfg(test)]
mod tests {
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::{TestAppContext, VisualTestContext};
    use projects::ProjectsSnapshot;

    use super::*;
    use crate::machines::MachineId;

    fn open(
        cx: &mut TestAppContext,
    ) -> (
        Entity<ServerClient>,
        Entity<TerminalDrawer>,
        &mut VisualTestContext,
    ) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            )
        });
        let (drawer, cx) = cx.add_window_view({
            let client = client.clone();
            |_, cx| TerminalDrawer::new(client, ThreadId(1), cx)
        });
        (client, drawer, cx)
    }

    /// What the server says runs in front of each shell.
    fn set_running(client: &Entity<ServerClient>, running: &[(u32, &str)], cx: &mut App) {
        let projects = client.read(cx).projects().clone();
        projects.update(cx, |store, cx| {
            store.set_snapshot(
                ProjectsSnapshot {
                    drawer_commands: running
                        .iter()
                        .map(|(number, command)| (ThreadId(1), *number, command.to_string()))
                        .collect(),
                    ..Default::default()
                },
                cx,
            )
        });
    }

    fn numbers(drawer: &TerminalDrawer) -> Vec<u32> {
        drawer
            .terminals
            .iter()
            .map(|terminal| terminal.number)
            .collect()
    }

    fn closes(client: &Entity<ServerClient>, cx: &mut VisualTestContext) -> usize {
        client.read_with(cx, |client, _| {
            client
                .sent_for_test()
                .into_iter()
                .filter(|request| matches!(request, Request::CloseTerminal(_)))
                .count()
        })
    }

    /// A new shell is the last tab and shows; closing the one in front shows the next along,
    /// and closing the last empties the drawer.
    #[gpui::test]
    fn shells_are_tabs(cx: &mut TestAppContext) {
        let (_, drawer, cx) = open(cx);
        let emptied = std::rc::Rc::new(std::cell::Cell::new(false));
        cx.update(|_, cx| {
            let emptied = emptied.clone();
            cx.subscribe(&drawer, move |_, event, _| {
                if matches!(event, TerminalDrawerEvent::Empty) {
                    emptied.set(true);
                }
            })
            .detach();
        });
        drawer.update(cx, |drawer, cx| {
            assert_eq!(numbers(drawer), vec![1]);
            drawer.add_terminal(cx);
            drawer.add_terminal(cx);
            assert_eq!(numbers(drawer), vec![1, 2, 3]);
            assert_eq!(drawer.active, 3);
            drawer.active = 2;
            drawer.close_terminal(2, cx);
            assert_eq!(numbers(drawer), vec![1, 3]);
            assert_eq!(drawer.active, 3);
            drawer.close_terminal(3, cx);
            assert_eq!(drawer.active, 1);
        });
        assert!(!emptied.get());
        drawer.update(cx, |drawer, cx| drawer.close_terminal(1, cx));
        assert!(emptied.get());
    }

    /// Each tab names what runs in its shell, and a click on one shows it.
    #[gpui::test]
    fn tabs_name_what_runs_and_switch_shells(cx: &mut TestAppContext) {
        let (client, drawer, cx) = open(cx);
        drawer.update(cx, |drawer, cx| {
            drawer.add_terminal(cx);
        });
        cx.update(|_, cx| set_running(&client, &[(2, "npm run dev")], cx));
        cx.run_until_parked();
        let tab = |cx: &mut VisualTestContext, number: u32| {
            cx.debug_bounds(&*format!("drawer-tab-{number}").leak())
                .expect("a tab")
        };
        assert!(tab(cx, 1).size.width < tab(cx, 2).size.width);
        let names = drawer.read_with(cx, |drawer, cx| {
            [1, 2].map(|number| drawer.running_program(number, cx))
        });
        assert_eq!(names, [None, Some("npm run dev".to_string())]);

        let first = tab(cx, 1);
        cx.simulate_click(first.center(), gpui::Modifiers::none());
        assert_eq!(drawer.read_with(cx, |drawer, _| drawer.active), 1);
    }

    /// An idle shell's × ends it at once; while a program runs, it asks first.
    #[gpui::test]
    fn closing_a_busy_shell_asks_first(cx: &mut TestAppContext) {
        let (client, drawer, cx) = open(cx);
        drawer.update(cx, |drawer, cx| {
            drawer.add_terminal(cx);
            drawer.add_terminal(cx);
        });
        cx.update(|_, cx| set_running(&client, &[(2, "npm run dev")], cx));
        cx.run_until_parked();

        cx.update(|window, cx| drawer.update(cx, |drawer, cx| drawer.request_close(3, window, cx)));
        cx.run_until_parked();
        assert!(!cx.has_pending_prompt());
        assert_eq!(closes(&client, cx), 1);

        cx.update(|window, cx| drawer.update(cx, |drawer, cx| drawer.request_close(2, window, cx)));
        assert_eq!(
            cx.pending_prompt(),
            Some((
                "Close “npm run dev”?".to_string(),
                "It's still running, and closing the terminal ends it.".to_string()
            ))
        );
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        assert_eq!(
            drawer.read_with(cx, |drawer, _| numbers(drawer)),
            vec![1, 2]
        );
        cx.update(|window, cx| drawer.update(cx, |drawer, cx| drawer.request_close(2, window, cx)));
        cx.simulate_prompt_answer("Close");
        cx.run_until_parked();
        assert_eq!(drawer.read_with(cx, |drawer, _| numbers(drawer)), vec![1]);
        assert_eq!(closes(&client, cx), 2);
    }
}
