//! A thread's terminals: t3code's terminal drawer (`ThreadTerminalDrawer.tsx`, and its layout
//! rules in `terminalUiStateStore.ts`). Terminals sit in groups, side by side or stacked, up to
//! four to a group; the active group fills the drawer. With more than one terminal a list on
//! the right shows the groups. The layout is this window's; the server keeps the terminals
//! running, and lists them so a restarted app finds them again.

use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::{CAPABILITY_DRAWER_TERMINALS, Request, Response};
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, MouseButton, Subscription, Window,
};
use projects::ThreadId;
use ui::{Tooltip, prelude::*};

use crate::server_client::ServerClient;
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;

/// t3code's `MAX_TERMINALS_PER_GROUP`.
const MAX_TERMINALS_PER_GROUP: usize = 4;
/// t3code's list width (`w-36`).
const LIST_WIDTH: Pixels = px(144.);

pub enum TerminalDrawerEvent {
    ToggleFullScreen,
    /// Its last terminal closed.
    Empty,
}

#[derive(Clone, Copy, PartialEq)]
enum Split {
    SideBySide,
    Stacked,
}

struct DrawerTerminal {
    number: u32,
    view: Entity<TerminalView>,
    _exit: Subscription,
}

struct Group {
    numbers: Vec<u32>,
    split: Split,
}

pub struct TerminalDrawer {
    client: Entity<ServerClient>,
    thread_id: ThreadId,
    terminals: Vec<DrawerTerminal>,
    groups: Vec<Group>,
    active: u32,
    next_number: u32,
    is_full_screen: bool,
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
    /// Opens with Terminal 1, then takes in the thread's other terminals still running on the
    /// server, each in a group of its own.
    pub fn new(client: Entity<ServerClient>, thread_id: ThreadId, cx: &mut Context<Self>) -> Self {
        let mut this = Self {
            client: client.clone(),
            thread_id,
            terminals: Vec::new(),
            groups: Vec::new(),
            active: 1,
            next_number: 1,
            is_full_screen: false,
        };
        this.add_terminal(None, cx);
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
                            this.add_terminal(None, cx);
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

    fn group_of(&self, number: u32) -> Option<usize> {
        self.groups
            .iter()
            .position(|group| group.numbers.contains(&number))
    }

    pub fn set_full_screen(&mut self, is_full_screen: bool, cx: &mut Context<Self>) {
        if self.is_full_screen != is_full_screen {
            self.is_full_screen = is_full_screen;
            cx.notify();
        }
    }

    /// Starts the next terminal: in the active group beside the active terminal when
    /// splitting, else in a group of its own. Returns its number.
    fn add_terminal(&mut self, split: Option<Split>, cx: &mut Context<Self>) -> u32 {
        let number = self.next_number;
        self.next_number += 1;
        let key = TerminalKey::drawer(self.thread_id, number);
        let terminal = Terminal::shared(&self.client, key, cx);
        // A shell that exits closes its terminal, as in t3code. One that had ended before the
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
        let active_group = self.group_of(self.active);
        match (split, active_group) {
            (Some(split), Some(index)) => {
                let group = &mut self.groups[index];
                let anchor = group
                    .numbers
                    .iter()
                    .position(|candidate| *candidate == self.active)
                    .map_or(group.numbers.len(), |position| position + 1);
                group.numbers.insert(anchor, number);
                group.split = split;
            }
            _ => self.groups.push(Group {
                numbers: vec![number],
                split: Split::SideBySide,
            }),
        }
        self.active = number;
        cx.notify();
        number
    }

    fn split(&mut self, split: Split, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_active_group_full() {
            return;
        }
        self.add_terminal(Some(split), cx);
        self.focus_active(window, cx);
    }

    fn new_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.add_terminal(None, cx);
        self.focus_active(window, cx);
    }

    fn is_active_group_full(&self) -> bool {
        self.group_of(self.active)
            .is_some_and(|index| self.groups[index].numbers.len() >= MAX_TERMINALS_PER_GROUP)
    }

    /// Ends the terminal on the server. The next one along becomes active, as in t3code.
    fn close_terminal(&mut self, number: u32, cx: &mut Context<Self>) {
        let Some(position) = self
            .terminals
            .iter()
            .position(|terminal| terminal.number == number)
        else {
            return;
        };
        self.terminals.remove(position);
        for group in &mut self.groups {
            group.numbers.retain(|candidate| *candidate != number);
        }
        self.groups.retain(|group| !group.numbers.is_empty());
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

    /// t3code's actions: split side by side, split stacked, new terminal, close the active one,
    /// and full screen.
    fn render_actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let can_split = !self.is_active_group_full() && self.has_several_terminals(cx);
        let can_add = self.has_several_terminals(cx);
        let split_tooltip = |label: &'static str| {
            if can_split {
                label.to_string()
            } else {
                format!("{label} (max {MAX_TERMINALS_PER_GROUP} per group)")
            }
        };
        let active = self.active;
        let side_by_side_tooltip = split_tooltip("Split Terminal Horizontally");
        let stacked_tooltip = split_tooltip("Split Terminal Vertically");
        h_flex()
            .child(
                IconButton::new("drawer-split", IconName::SquareSplitHorizontal)
                    .icon_size(IconSize::XSmall)
                    .disabled(!can_split)
                    .tooltip(Tooltip::text(side_by_side_tooltip))
                    .on_click(
                        cx.listener(|this, _, window, cx| {
                            this.split(Split::SideBySide, window, cx)
                        }),
                    ),
            )
            .child(
                IconButton::new("drawer-split-stacked", IconName::SquareSplitVertical)
                    .icon_size(IconSize::XSmall)
                    .disabled(!can_split)
                    .tooltip(Tooltip::text(stacked_tooltip))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.split(Split::Stacked, window, cx)),
                    ),
            )
            .child(
                IconButton::new("drawer-new-terminal", IconName::Plus)
                    .icon_size(IconSize::XSmall)
                    .disabled(!can_add)
                    .tooltip(Tooltip::text("New Terminal"))
                    .on_click(cx.listener(|this, _, window, cx| this.new_terminal(window, cx))),
            )
            .child(
                IconButton::new("drawer-close-terminal", IconName::Trash)
                    .icon_size(IconSize::XSmall)
                    .tooltip(Tooltip::text("Close Terminal"))
                    .on_click(cx.listener(move |this, _, _, cx| this.close_terminal(active, cx))),
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
                    cx.listener(|_, _, _, cx| cx.emit(TerminalDrawerEvent::ToggleFullScreen)),
                ),
            )
    }

    /// The active group's terminals, split evenly.
    fn render_group(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let Some(group) = self.group_of(self.active).map(|index| &self.groups[index]) else {
            return div().into_any_element();
        };
        let is_split = group.numbers.len() > 1;
        let stacked = group.split == Split::Stacked;
        let cells = group
            .numbers
            .iter()
            .enumerate()
            .filter_map(|(index, number)| {
                let terminal = self.terminal(*number)?;
                let number = *number;
                let is_active = number == self.active;
                Some(
                    div()
                        .id(("drawer-terminal", number as usize))
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        // A stretched cell's size isn't definite to its child, so it's set.
                        .map(|cell| {
                            if stacked {
                                cell.w_full()
                            } else {
                                cell.h_full()
                            }
                        })
                        .pt_1()
                        .when(is_split && index > 0, |cell| {
                            let border = if is_active {
                                colors.border
                            } else {
                                colors.border_variant
                            };
                            if stacked {
                                cell.border_t_1().border_color(border)
                            } else {
                                cell.border_l_1().border_color(border)
                            }
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                if this.active != number {
                                    this.active = number;
                                    cx.notify();
                                }
                            }),
                        )
                        .child(terminal.view.clone()),
                )
            });
        if stacked {
            v_flex().size_full().children(cells).into_any_element()
        } else {
            h_flex().size_full().children(cells).into_any_element()
        }
    }

    /// t3code's list: the actions, then each group (named when there's more than one, or a
    /// split) and its terminals.
    fn render_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors().clone();
        let shows_group_headers =
            self.groups.len() > 1 || self.groups.iter().any(|group| group.numbers.len() > 1);
        let groups = self
            .groups
            .iter()
            .enumerate()
            .map(|(group_index, group)| {
                let is_group_active = group.numbers.contains(&self.active);
                let first = group.numbers.first().copied().unwrap_or(self.active);
                let (label, icon) = match (group.numbers.len() > 1, group.split) {
                    (false, _) => ("Single", IconName::Square),
                    (true, Split::SideBySide) => ("Side by side", IconName::SquareSplitHorizontal),
                    (true, Split::Stacked) => ("Stacked", IconName::SquareSplitVertical),
                };
                let header =
                    shows_group_headers.then(|| {
                        h_flex()
                            .id(("drawer-group", group_index))
                            .h(px(22.))
                            .px_1p5()
                            .gap_1()
                            .rounded_sm()
                            .cursor_pointer()
                            .when(is_group_active, |row| row.bg(colors.element_selected))
                            .hover(|row| row.bg(colors.ghost_element_hover))
                            .child(Icon::new(icon).size(IconSize::XSmall).color(Color::Muted))
                            .child(
                                div().flex_1().min_w_0().child(
                                    Label::new(label)
                                        .size(LabelSize::XSmall)
                                        .color(if is_group_active {
                                            Color::Default
                                        } else {
                                            Color::Muted
                                        })
                                        .truncate(),
                                ),
                            )
                            .child(
                                Label::new(group.numbers.len().to_string())
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.activate(first, window, cx)
                            }))
                    });
                let rows =
                    group.numbers.iter().map(|number| {
                        let number = *number;
                        let is_active = number == self.active;
                        let row_group = SharedString::from(format!("drawer-row-{number}"));
                        h_flex()
                            .id(("drawer-row", number as usize))
                            .group(row_group.clone())
                            .h_6()
                            .pl_1()
                            .pr_2()
                            .gap_1()
                            .rounded_md()
                            .cursor_pointer()
                            .when(is_active, |row| row.bg(colors.element_selected))
                            .when(!is_active, |row| {
                                row.hover(|row| row.bg(colors.ghost_element_hover))
                            })
                            // The icon turns into the close button under the mouse, as in t3code.
                            .child(
                                div()
                                    .relative()
                                    .size_4()
                                    .flex_none()
                                    .child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .group_hover(row_group.clone(), |icon| icon.invisible())
                                            .child(
                                                Icon::new(IconName::Terminal)
                                                    .size(IconSize::XSmall)
                                                    .color(Color::Muted),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .visible_on_hover(row_group)
                                            .child(
                                                IconButton::new(
                                                    ("drawer-close-row", number as usize),
                                                    IconName::Close,
                                                )
                                                .icon_size(IconSize::XSmall)
                                                .tooltip(Tooltip::text(format!(
                                                    "Close Terminal {number}"
                                                )))
                                                .on_click(cx.listener(move |this, _, _, cx| {
                                                    cx.stop_propagation();
                                                    this.close_terminal(number, cx)
                                                })),
                                            ),
                                    ),
                            )
                            .child(
                                div().flex_1().min_w_0().child(
                                    Label::new(format!("Terminal {number}"))
                                        .size(LabelSize::Small)
                                        .color(if is_active {
                                            Color::Default
                                        } else {
                                            Color::Muted
                                        })
                                        .truncate(),
                                ),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.activate(number, window, cx)
                            }))
                    });
                v_flex()
                    .pb_0p5()
                    .children(header)
                    .child(v_flex().gap_0p5().children(rows))
            })
            .collect::<Vec<_>>();
        v_flex()
            .w(LIST_WIDTH)
            .flex_none()
            .h_full()
            .border_l_1()
            .border_color(colors.border_variant)
            .child(
                h_flex()
                    .h(px(22.))
                    .flex_none()
                    .justify_end()
                    .border_b_1()
                    .border_color(colors.border_variant)
                    .child(self.render_actions(cx)),
            )
            .child(
                v_flex()
                    .id("drawer-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_1()
                    .children(groups),
            )
    }
}

impl Render for TerminalDrawer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_list = self.terminals.len() > 1;
        h_flex()
            .relative()
            .size_full()
            .bg(cx.theme().colors().terminal_background)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(self.render_group(cx)),
            )
            .when(has_list, |drawer| drawer.child(self.render_list(cx)))
            // With one terminal, the actions float at its top right, as in t3code.
            .when(!has_list, |drawer| {
                drawer.child(
                    div()
                        .absolute()
                        .top_1()
                        .right_2()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().colors().border_variant)
                        .bg(cx.theme().colors().editor_background)
                        .child(self.render_actions(cx)),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::TestAppContext;

    use super::*;
    use crate::machines::MachineId;

    fn layout(drawer: &TerminalDrawer) -> Vec<(Vec<u32>, bool)> {
        drawer
            .groups
            .iter()
            .map(|group| (group.numbers.clone(), group.split == Split::Stacked))
            .collect()
    }

    #[gpui::test]
    fn terminals_split_and_close_as_t3code_s_do(cx: &mut TestAppContext) {
        let drawer = cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            cx.new(|cx| TerminalDrawer::new(client, ThreadId(1), cx))
        });
        let emptied = std::rc::Rc::new(std::cell::Cell::new(false));
        cx.update(|cx| {
            let emptied = emptied.clone();
            cx.subscribe(&drawer, move |_, event, _| {
                if matches!(event, TerminalDrawerEvent::Empty) {
                    emptied.set(true);
                }
            })
            .detach();
        });

        drawer.update(cx, |drawer, cx| {
            assert_eq!(layout(drawer), vec![(vec![1], false)]);
            // A split goes beside the active terminal, in its group.
            drawer.add_terminal(Some(Split::SideBySide), cx);
            assert_eq!(layout(drawer), vec![(vec![1, 2], false)]);
            drawer.active = 1;
            drawer.add_terminal(Some(Split::Stacked), cx);
            assert_eq!(layout(drawer), vec![(vec![1, 3, 2], true)]);
            assert_eq!(drawer.active, 3);
            drawer.add_terminal(Some(Split::Stacked), cx);
            assert!(drawer.is_active_group_full());
            // A new terminal has a group of its own.
            drawer.add_terminal(None, cx);
            assert_eq!(layout(drawer).len(), 2);
            assert_eq!(drawer.active, 5);
            // Closing the active one makes the next along active.
            drawer.active = 3;
            drawer.close_terminal(3, cx);
            assert_eq!(layout(drawer)[0].0, vec![1, 4, 2]);
            assert_eq!(drawer.active, 4);
            for number in [1, 2, 4, 5] {
                drawer.close_terminal(number, cx);
            }
            assert!(drawer.groups.is_empty());
        });
        assert!(emptied.get());
    }
}
