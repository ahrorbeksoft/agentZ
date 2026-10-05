//! A terminal thread (herdr's panes): a toolbar like an agent thread's over its terminal.

use std::time::SystemTime;

use agentz_protocol::CAPABILITY_THREAD_DIFF;
use agentz_protocol::terminal::{TerminalCommand, TerminalKey};
use gpui::{App, Context, Entity, FocusHandle, Focusable, Subscription, Task, Window};
use projects::ThreadId;
use ui::{Tooltip, prelude::*};

use crate::agent_view::{ChangeStat, TOOLBAR_HEIGHT, load_change_stat, render_changes_button};
use crate::server_client::ServerClient;
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;

pub struct TerminalThreadView {
    title: SharedString,
    command: TerminalCommand,
    /// An agent CLI runs in it, whose session a restart would end.
    has_agent: bool,
    is_diff_open: bool,
    client: Entity<ServerClient>,
    thread_id: ThreadId,
    /// What its agent CLIs changed, for the changes button, as an agent thread's header shows.
    changes: ChangeStat,
    /// The turn the changes were last asked for after, by when it finished.
    changes_asked_for: Option<Option<SystemTime>>,
    _changes_load: Task<()>,
    terminal: Entity<Terminal>,
    view: Entity<TerminalView>,
    _subscriptions: Vec<Subscription>,
}

impl TerminalThreadView {
    pub fn new(
        client: &Entity<ServerClient>,
        thread_id: ThreadId,
        title: SharedString,
        command: TerminalCommand,
        cx: &mut Context<Self>,
    ) -> Self {
        let terminal = Terminal::shared(client, TerminalKey::Thread(thread_id), cx);
        let view = cx.new(|cx| TerminalView::new(terminal.clone(), TerminalMode::Scrollable, cx));
        let store = client.read(cx).projects().clone();
        let subscriptions = vec![
            cx.observe(&terminal, |_, _, cx| cx.notify()),
            cx.observe(&store, |this, _, cx| this.load_changes(cx)),
        ];
        let mut this = Self {
            title,
            command,
            has_agent: false,
            is_diff_open: false,
            client: client.clone(),
            thread_id,
            changes: ChangeStat::default(),
            changes_asked_for: None,
            _changes_load: Task::ready(()),
            terminal,
            view,
            _subscriptions: subscriptions,
        };
        this.load_changes(cx);
        this
    }

    /// Asks again what the thread changed whenever one of its turns finishes.
    fn load_changes(&mut self, cx: &mut Context<Self>) {
        let completed_at = self
            .client
            .read(cx)
            .projects()
            .read(cx)
            .thread(self.thread_id)
            .and_then(|thread| thread.completed_at);
        if self.changes_asked_for == Some(completed_at) {
            return;
        }
        let client = self.client.read(cx);
        if client.connection().is_none() || !client.has_capability(CAPABILITY_THREAD_DIFF) {
            return;
        }
        self.changes_asked_for = Some(completed_at);
        let request = load_change_stat(client, self.thread_id);
        self._changes_load = cx.spawn(async move |this, cx| {
            let Some(changes) = request.await else {
                return;
            };
            this.update(cx, |this, cx| {
                if this.changes != changes {
                    this.changes = changes;
                    cx.notify();
                }
            })
            .ok();
        });
    }

    pub fn set_title(&mut self, title: SharedString, cx: &mut Context<Self>) {
        if self.title != title {
            self.title = title;
            cx.notify();
        }
    }

    pub fn set_has_agent(&mut self, has_agent: bool, cx: &mut Context<Self>) {
        if self.has_agent != has_agent {
            self.has_agent = has_agent;
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
        h_flex()
            .h(TOOLBAR_HEIGHT)
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
            // Not while an agent CLI runs: one click would end its session.
            .when(!self.has_agent, |toolbar| {
                toolbar.child(
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
            })
            .child(render_changes_button(self.changes, self.is_diff_open))
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
