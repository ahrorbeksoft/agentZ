//! A terminal thread, or the agent CLI in a Workspaces pane shown full screen in Agents: a
//! toolbar like an agent thread's over its terminal.

use agentz_protocol::terminal::{TerminalCommand, TerminalKey};
use gpui::{App, Context, Entity, FocusHandle, Focusable, Subscription, Window};
use ui::{Tooltip, prelude::*};

use crate::ToggleDiff;
use crate::server_client::ServerClient;
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;

pub struct TerminalThreadView {
    title: SharedString,
    command: TerminalCommand,
    /// A thread's terminal can restart and has changes; a pane's ends with its pane.
    is_thread: bool,
    is_diff_open: bool,
    terminal: Entity<Terminal>,
    view: Entity<TerminalView>,
    _subscriptions: Vec<Subscription>,
}

impl TerminalThreadView {
    pub fn new(
        client: &Entity<ServerClient>,
        key: TerminalKey,
        title: SharedString,
        command: TerminalCommand,
        cx: &mut Context<Self>,
    ) -> Self {
        let is_thread = matches!(key, TerminalKey::Thread(_));
        let terminal = Terminal::shared(client, key, cx);
        let view = cx.new(|cx| TerminalView::new(terminal.clone(), TerminalMode::Scrollable, cx));
        let subscriptions = vec![cx.observe(&terminal, |_, _, cx| cx.notify())];
        Self {
            title,
            command,
            is_thread,
            is_diff_open: false,
            terminal,
            view,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_title(&mut self, title: SharedString, cx: &mut Context<Self>) {
        if self.title != title {
            self.title = title;
            cx.notify();
        }
    }

    pub fn set_diff_open(&mut self, is_diff_open: bool, cx: &mut Context<Self>) {
        if self.is_diff_open != is_diff_open {
            self.is_diff_open = is_diff_open;
            cx.notify();
        }
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let terminal = self.terminal.read(cx);
        let exit = terminal.frame().and_then(|frame| frame.exited.clone());
        let window_title = terminal
            .frame()
            .and_then(|frame| frame.title.clone())
            .filter(|title| !title.is_empty() && *title != *self.title);
        let command = self
            .command
            .command
            .clone()
            .unwrap_or_else(|| "login shell".to_string());
        let status = exit.map(|exit| match (exit.code, exit.signal) {
            (_, Some(signal)) => format!("Ended by {signal}"),
            (Some(0), None) => "Exited".to_string(),
            (Some(code), None) => format!("Exited with code {code}"),
            (None, None) => "Exited".to_string(),
        });
        let has_exited = status.is_some();
        let is_thread = self.is_thread;
        h_flex()
            .h(px(36.))
            .flex_none()
            .px_2()
            .gap_1p5()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .child(
                Icon::new(IconName::Terminal)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Label::new(self.title.clone())
                    .size(LabelSize::Small)
                    .truncate(),
            )
            .child(
                Label::new(window_title.unwrap_or(command))
                    .size(LabelSize::Small)
                    .color(Color::Muted)
                    .truncate(),
            )
            .child(div().flex_1())
            .children(status.map(|status| {
                Label::new(status)
                    .size(LabelSize::Small)
                    .color(Color::Muted)
            }))
            .when(is_thread, |toolbar| {
                toolbar
                    .child(
                        Button::new("restart-terminal", "Restart")
                            .label_size(LabelSize::Small)
                            .start_icon(Icon::new(IconName::RotateCw).size(IconSize::XSmall))
                            .when(!has_exited, |button| button.style(ButtonStyle::Subtle))
                            .when(has_exited, |button| button.style(ButtonStyle::Outlined))
                            .tooltip(Tooltip::text("Run the command again"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.terminal
                                    .update(cx, |terminal, cx| terminal.restart(cx));
                                window.focus(&this.view.focus_handle(cx), cx);
                            })),
                    )
                    .child(
                        IconButton::new("toggle-diff", IconName::Diff)
                            .icon_size(IconSize::Small)
                            .toggle_state(self.is_diff_open)
                            .tooltip(|_, cx| Tooltip::for_action("Show Changes", &ToggleDiff, cx))
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(ToggleDiff), cx)
                            }),
                    )
            })
    }
}

impl Focusable for TerminalThreadView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.view.focus_handle(cx)
    }
}

impl Render for TerminalThreadView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().colors().terminal_background)
            .child(self.render_toolbar(cx))
            .child(div().flex_1().min_h_0().py_1().child(self.view.clone()))
    }
}
