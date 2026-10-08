//! Logging in to an agent with the methods it offers. On the agent's settings page each method
//! is a row with its own Log In button, and a method that takes an API key or a gateway opens
//! its form in place. In a thread, the methods are full-width buttons in the middle of it. A
//! login in progress shows what's left to do: finishing in the browser, or entering a one-time
//! code on the page the agent gave. Terminal methods run in a terminal on the agent's machine.
//! In Add Account's dialog, the methods are a list, and the one picked shows its progress there.

use std::time::Duration;

use agentz_client::ssh::{HeldForward, LocalForward, Ssh};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::AgentId;
use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::thread::{
    Elicitation, LoginInput, ThreadView, api_key_meta, gateway_meta, login_code, login_input,
    logs_in_through_terminal,
};
use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, KeyBinding, SharedString, Subscription, Task,
    Window, div,
};
use markdown::{Markdown, MarkdownElement, MarkdownFont, MarkdownStyle};
use text_input::{TextInput, TextInputEvent};
use ui::{Tooltip, prelude::*};

use crate::agent_icons::agent_icon;
use crate::controls::{
    ActionButton, ActionSize, ActionStyle, code_boxes, copy_to_clipboard, field_label, icon_tile,
    link_host, spinner, text_field,
};
use crate::server_client::Transport;
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;
use crate::thread_entity::AgentThread;

const KEY_CONTEXT: &str = "AgentLoginForm";
/// Room for a login command's prompts, a URL and a pasted code.
const LOGIN_TERMINAL_HEIGHT: Pixels = px(240.);
/// The width of a thread's login buttons.
const CENTERED_WIDTH: Pixels = px(300.);
const COPIED_FOR: Duration = Duration::from_secs(2);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoginLayout {
    /// Rows for the account card on the agent's settings page, under its status row.
    Rows,
    /// A panel in the middle of a thread.
    Centered,
    /// Add Account's dialog: the methods as a list, then the picked one's progress, with the
    /// dialog's own buttons for Back and Cancel.
    Dialog,
}

/// Where a login in Add Account's dialog stands, for the dialog's buttons.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LoginStep {
    Choosing,
    /// Typing in an API key or a gateway, which Log In sends.
    Entering {
        can_submit: bool,
    },
    InProgress,
    Failed,
}

/// What was just copied, to say so on its button for a moment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Copied {
    Link,
    Code,
}

pub struct AgentLogin {
    thread: Entity<AgentThread>,
    layout: LoginLayout,
    /// For the agent's icon in a thread's panel.
    agent_id: Option<AgentId>,
    /// Where a terminal login method runs, on the agent's machine, with the method.
    login_terminal: Option<(acp::AuthMethodId, Entity<TerminalView>)>,
    /// The API-key or gateway method whose details are being typed in.
    entering: Option<acp::AuthMethod>,
    /// The method last picked, which the dialog shows the progress of until Back, and tries
    /// again after it fails.
    chosen: Option<acp::AuthMethod>,
    /// Whether the picked method's login was seen under way, and the failure there was as it
    /// was picked: until either changes, that failure is an earlier login's.
    chosen_started: bool,
    failure_when_chosen: Option<SharedString>,
    api_key: Entity<TextInput>,
    base_url: Entity<TextInput>,
    headers: Vec<HeaderRow>,
    input_error: Option<SharedString>,
    /// What the agent said when it asked for a login, as markdown (Zed renders it so).
    description: Option<(SharedString, Entity<Markdown>)>,
    copied: Option<Copied>,
    _copied_reset: Task<()>,
    /// The page from another machine ([`ThreadView::login_page`]) last opened here, so the
    /// panel stops asking to open it.
    opened_page: Option<SharedString>,
    /// Ports forwarded from the agent's machine for the login's page, until it's over.
    forwards: Vec<HeldForward>,
    forward_error: Option<SharedString>,
    _forwarding: Task<()>,
    _subscriptions: Vec<Subscription>,
}

/// One HTTP header a gateway login sends.
struct HeaderRow {
    name: Entity<TextInput>,
    value: Entity<TextInput>,
    _subscriptions: [Subscription; 2],
}

/// The page and code a login in progress waits on.
struct LoginWait {
    /// The page to finish on: the one the agent asked to open, or one it printed.
    url: Option<SharedString>,
    code: Option<String>,
    /// The agent's own request to open the page, which opening it answers.
    elicitation: Option<Elicitation>,
    /// The agent opened it on its machine, and it hasn't been opened here yet.
    must_open: bool,
}

impl AgentLogin {
    pub fn new(
        thread: Entity<AgentThread>,
        layout: LoginLayout,
        agent_id: Option<AgentId>,
        cx: &mut Context<Self>,
    ) -> Self {
        let api_key = cx.new(|cx| TextInput::new("sk-…", cx).masked());
        let base_url = cx.new(|cx| TextInput::new("https://", cx));
        let subscriptions = vec![
            cx.observe(&thread, |this, thread, cx| {
                this.sync_description(cx);
                // A login started elsewhere (another window) takes over from typing one in.
                if thread.read(cx).is_authenticating() && this.entering.is_some() {
                    this.stop_entering(cx);
                }
                if thread.read(cx).is_authenticating() && this.chosen.is_some() {
                    this.chosen_started = true;
                }
                // The login's page and its forwarded ports are done with once it's over.
                let thread = thread.read(cx);
                if thread.login_page().is_none() {
                    this.opened_page = None;
                }
                if !thread.is_authenticating()
                    && thread.login_page().is_none()
                    && this.running_terminal_login(cx).is_none()
                {
                    this.forwards.clear();
                    this.forward_error = None;
                }
                cx.notify();
            }),
            cx.subscribe(&api_key, |this, _, _: &TextInputEvent, cx| {
                this.input_changed(cx)
            }),
            cx.subscribe(&base_url, |this, _, _: &TextInputEvent, cx| {
                this.input_changed(cx)
            }),
        ];
        let mut this = Self {
            thread,
            layout,
            agent_id,
            login_terminal: None,
            entering: None,
            chosen: None,
            chosen_started: false,
            failure_when_chosen: None,
            api_key,
            base_url,
            headers: Vec::new(),
            input_error: None,
            description: None,
            copied: None,
            _copied_reset: Task::ready(()),
            opened_page: None,
            forwards: Vec::new(),
            forward_error: None,
            _forwarding: Task::ready(()),
            _subscriptions: subscriptions,
        };
        this.sync_description(cx);
        this
    }

    fn sync_description(&mut self, cx: &mut Context<Self>) {
        let text = self.thread.read(cx).auth_description().cloned();
        match (text, &self.description) {
            (Some(text), Some((current, _))) if text == *current => {}
            (Some(text), _) => {
                let markdown = cx.new(|cx| Markdown::new(text.clone(), None, None, cx));
                self.description = Some((text, markdown));
            }
            (None, _) => self.description = None,
        }
    }

    /// Log In is enabled by what's typed, and typing clears the last complaint about it.
    fn input_changed(&mut self, cx: &mut Context<Self>) {
        self.input_error = None;
        cx.notify();
    }

    /// Agent methods log in through the agent, terminal methods in a terminal on its machine,
    /// and methods that take an API key or a gateway ask for it first.
    pub(crate) fn choose(
        &mut self,
        method: acp::AuthMethod,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.failure_when_chosen = self.failure(cx);
        self.chosen = Some(method.clone());
        self.chosen_started = false;
        if logs_in_through_terminal(&method) {
            self.stop_entering(cx);
            self.start_terminal_login(method.id().clone(), window, cx);
            return;
        }
        match login_input(&method) {
            LoginInput::Nothing => {
                self.stop_entering(cx);
                let method_id = method.id().clone();
                self.thread
                    .update(cx, |thread, cx| thread.authenticate(method_id, None, cx));
            }
            LoginInput::ApiKey => {
                self.stop_entering(cx);
                self.entering = Some(method);
                window.focus(&self.api_key.focus_handle(cx), cx);
            }
            LoginInput::Gateway => {
                self.stop_entering(cx);
                self.entering = Some(method);
                self.add_header(cx);
                window.focus(&self.base_url.focus_handle(cx), cx);
            }
        }
        cx.notify();
    }

    fn can_submit(&self, cx: &App) -> bool {
        match self.entering.as_ref().map(login_input) {
            Some(LoginInput::ApiKey) => !self.api_key.read(cx).text().trim().is_empty(),
            Some(LoginInput::Gateway) => !self.base_url.read(cx).text().trim().is_empty(),
            _ => false,
        }
    }

    pub(crate) fn submit(&mut self, cx: &mut Context<Self>) {
        let Some(method) = self.entering.clone() else {
            return;
        };
        if !self.can_submit(cx) {
            return;
        }
        let meta = match login_input(&method) {
            LoginInput::ApiKey => api_key_meta(self.api_key.read(cx).text().trim()),
            LoginInput::Gateway => {
                let base_url = self.base_url.read(cx).text().trim().to_string();
                if !url::Url::parse(&base_url)
                    .is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
                {
                    self.input_error =
                        Some("The base URL must start with http:// or https://.".into());
                    cx.notify();
                    return;
                }
                let headers: Vec<(String, String)> = self
                    .headers
                    .iter()
                    .filter_map(|row| {
                        let name = row.name.read(cx).text().trim().to_string();
                        (!name.is_empty())
                            .then(|| (name, row.value.read(cx).text().trim().to_string()))
                    })
                    .collect();
                gateway_meta(&base_url, &headers)
            }
            LoginInput::Nothing => return,
        };
        let method_id = method.id().clone();
        self.thread.update(cx, |thread, cx| {
            thread.authenticate(method_id, Some(meta), cx)
        });
        self.stop_entering(cx);
    }

    /// Forgets what was typed: keys and headers are secrets.
    fn stop_entering(&mut self, cx: &mut Context<Self>) {
        self.entering = None;
        self.input_error = None;
        self.api_key.update(cx, |input, cx| {
            input.set_text(String::new(), cx);
            input.set_masked(true, cx);
        });
        self.headers.clear();
        cx.notify();
    }

    fn add_header(&mut self, cx: &mut Context<Self>) {
        let name = cx.new(|cx| TextInput::new("Header", cx));
        let value = cx.new(|cx| TextInput::new("Value", cx).masked());
        let subscriptions = [
            cx.subscribe(&name, |this, _, _: &TextInputEvent, cx| {
                this.input_changed(cx)
            }),
            cx.subscribe(&value, |this, _, _: &TextInputEvent, cx| {
                this.input_changed(cx)
            }),
        ];
        self.headers.push(HeaderRow {
            name,
            value,
            _subscriptions: subscriptions,
        });
        cx.notify();
    }

    /// Runs a terminal login method on the agent's machine and shows its terminal, reusing
    /// the view from an earlier attempt.
    fn start_terminal_login(
        &mut self,
        method_id: acp::AuthMethodId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(connection) = self.thread.read(cx).connection() else {
            return;
        };
        self.thread.update(cx, |thread, cx| {
            thread.terminal_login(method_id.clone(), cx)
        });
        let view = match self.login_terminal.take() {
            Some((_, view)) => {
                // The server started the login over in a new terminal.
                let terminal = view.read(cx).terminal().clone();
                terminal.update(cx, |terminal, cx| terminal.reconnected(cx));
                view
            }
            None => {
                let client = self.thread.read(cx).client().clone();
                let terminal = Terminal::shared(&client, TerminalKey::Login(connection), cx);
                cx.new(|cx| TerminalView::new(terminal, TerminalMode::Scrollable, cx))
            }
        };
        window.focus(&view.focus_handle(cx), cx);
        self.login_terminal = Some((method_id, view));
        self.chosen_started = true;
        cx.notify();
    }

    /// The method whose terminal login runs. The server closes the terminal once the login
    /// succeeds; a failed one stays to show why.
    fn running_terminal_login(
        &self,
        cx: &App,
    ) -> Option<(&acp::AuthMethodId, Entity<TerminalView>)> {
        let (method_id, view) = self.login_terminal.as_ref()?;
        if view.read(cx).terminal().read(cx).error().is_some() {
            return None;
        }
        Some((method_id, view.clone()))
    }

    fn copy(&mut self, copied: Copied, text: &str, cx: &mut Context<Self>) {
        copy_to_clipboard(text, cx);
        self.copied = Some(copied);
        self._copied_reset = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(COPIED_FOR).await;
            this.update(cx, |this, cx| {
                this.copied = None;
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }

    /// Opens the login's page here. Opening the page the agent asked for answers it. A page from
    /// an agent on another machine sends the browser back to `localhost` there, so those ports
    /// are forwarded from it first, over its SSH connection.
    fn open(&mut self, wait: &LoginWait, cx: &mut Context<Self>) {
        let Some(url) = wait.url.clone() else {
            return;
        };
        let thread = self.thread.read(cx);
        if thread.login_page() == Some(&url) {
            self.opened_page = Some(url.clone());
        }
        let forwards = match thread.client().read(cx).transport() {
            Transport::Ssh(target) => Some(target.clone()).zip(Some(loopback_forwards(&url))),
            Transport::Local => None,
        }
        .filter(|(_, forwards)| !forwards.is_empty());
        match forwards {
            None => cx.open_url(&url),
            Some((target, forwards)) => {
                self.forward_error = None;
                let runtime = reqwest_client::runtime().handle().clone();
                let forwarding = runtime.spawn({
                    let runtime = runtime.clone();
                    async move {
                        let ssh = Ssh::new(&target)?;
                        let mut held = Vec::new();
                        for forward in forwards {
                            held.push(ssh.hold_forward(forward, runtime.clone()).await?);
                        }
                        anyhow::Ok(held)
                    }
                });
                self._forwarding = cx.spawn(async move |this, cx| {
                    let held = forwarding
                        .await
                        .map_err(anyhow::Error::from)
                        .and_then(|held| held);
                    this.update(cx, |this, cx| {
                        match held {
                            Ok(held) => {
                                this.forwards.extend(held);
                                cx.open_url(&url);
                            }
                            Err(error) => {
                                log::error!("couldn't forward the login page's port: {error:#}");
                                this.forward_error = Some(format!("{error:#}").into());
                            }
                        }
                        cx.notify();
                    })
                    .ok();
                });
            }
        }
        if let Some(elicitation) = wait
            .elicitation
            .as_ref()
            .filter(|elicitation| !elicitation.opened)
        {
            let id = elicitation.id;
            self.thread.update(cx, |thread, cx| {
                thread.respond_to_elicitation(
                    id,
                    acp::ElicitationAction::Accept(acp::ElicitationAcceptAction::new()),
                    cx,
                )
            });
        }
        // The page from another machine is opened now.
        cx.notify();
    }

    fn cancel_login(&mut self, cx: &mut Context<Self>) {
        self.thread
            .update(cx, |thread, cx| thread.cancel_authentication(cx));
    }

    fn render_description(&self, window: &Window, cx: &App) -> Option<AnyElement> {
        let (_, markdown) = self.description.as_ref()?;
        let mut style = MarkdownStyle::themed(MarkdownFont::Agent, window, cx);
        style.base_text_style.font_size = rems_from_px(12_f32).into();
        style.base_text_style.color = cx.theme().colors().text_muted;
        Some(MarkdownElement::new(markdown.clone(), style).into_any_element())
    }

    /// One method's row: its icon, name and description, and its Log In button, or the form
    /// or terminal it opened.
    fn render_method_row(
        &self,
        index: usize,
        method: &acp::AuthMethod,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let is_entering = self
            .entering
            .as_ref()
            .is_some_and(|entering| entering.id() == method.id());
        let terminal = self
            .running_terminal_login(cx)
            .filter(|(method_id, _)| *method_id == method.id())
            .map(|(_, view)| view);
        let is_open = is_entering || terminal.is_some();
        let is_primary = index == 0 && self.entering.is_none();
        let heading = v_flex()
            .flex_1()
            .min_w_0()
            .gap_0p5()
            .child(Label::new(method.name().to_string()))
            .children(
                method_description(method, self.thread.read(cx).agent_name()).map(|description| {
                    Label::new(description)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                }),
            );
        let row = h_flex()
            .px_4()
            .py_3()
            .gap_3()
            .border_t_1()
            .border_color(colors.border_variant)
            .child(icon_tile(
                Icon::new(method_icon(method)).color(Color::Muted),
                px(28.),
                cx,
            ));
        if !is_open {
            let method = method.clone();
            return row
                .child(heading)
                .child(
                    ActionButton::new(
                        SharedString::from(format!("login-{}", method.id().0)),
                        "Log In",
                    )
                    .style(if is_primary {
                        ActionStyle::Primary
                    } else {
                        ActionStyle::Outline
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose(method.clone(), window, cx)
                    })),
                )
                .into_any_element();
        }
        let body = match terminal {
            Some(terminal) => v_flex()
                .gap_2()
                .child(heading)
                .child(self.render_terminal(terminal, cx)),
            None => v_flex()
                .gap_4()
                .child(heading)
                .child(self.render_entry_fields(method, window, cx))
                .child(self.render_entry_actions(cx)),
        };
        row.items_start()
            .bg(gpui::black().opacity(0.08))
            .child(body.flex_1().min_w_0())
            .into_any_element()
    }

    fn render_terminal(
        &self,
        terminal: Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let agent_name = self.thread.read(cx).agent_name().clone();
        let page = self.render_terminal_page(cx);
        v_flex()
            .w_full()
            .gap_2()
            .child(
                div()
                    .w_full()
                    .h(LOGIN_TERMINAL_HEIGHT)
                    .rounded(px(6.))
                    .border_1()
                    .border_color(cx.theme().colors().border)
                    .overflow_hidden()
                    .child(terminal),
            )
            .children(page)
            .child(
                Label::new(format!(
                    "Finish in the terminal. {agent_name} starts again logged in once it's done."
                ))
                .size(LabelSize::Small)
                .color(Color::Muted),
            )
            .into_any_element()
    }

    /// The API key, or the gateway's address and headers, as the method asks.
    /// The page the command in the login terminal opened on the agent's machine (see
    /// [`ThreadView::login_page`]), to open here: the same buttons as a login in progress.
    fn render_terminal_page(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let thread = self.thread.read(cx);
        let agent_name = thread.agent_name().clone();
        thread.login_page()?;
        let wait = self.login_wait(cx);
        let url = wait.url.clone()?;
        let host = link_host(&url);
        let must_open = wait.must_open;
        let wait = std::rc::Rc::new(wait);
        let copy_link = ActionButton::new(
            "login-terminal-copy-link",
            if self.copied == Some(Copied::Link) {
                "Copied"
            } else {
                "Copy Link"
            },
        )
        .start_icon(Icon::new(IconName::Copy).size(IconSize::XSmall))
        .on_click(cx.listener(move |this, _, _, cx| this.copy(Copied::Link, &url, cx)));
        let open_label = match (&host, must_open) {
            (_, false) => "Open Again".to_string(),
            (Some(host), true) => format!("Open {host}"),
            (None, true) => "Open Page".to_string(),
        };
        let open = ActionButton::new("login-terminal-open", open_label)
            .style(if must_open {
                ActionStyle::Primary
            } else {
                ActionStyle::Outline
            })
            .end_icon(Icon::new(IconName::ArrowUpRight).size(IconSize::XSmall))
            .on_click(cx.listener(move |this, _, _, cx| this.open(&wait, cx)));
        let message = match &host {
            Some(host) => format!("{agent_name} needs you to log in at {host}."),
            None => format!("{agent_name} needs you to log in in your browser."),
        };
        Some(
            v_flex()
                .gap_1()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            div().flex_1().min_w_0().child(
                                Label::new(message)
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            ),
                        )
                        .child(copy_link)
                        .child(open),
                )
                .children(
                    self.forward_error
                        .clone()
                        .map(|error| Label::new(error).size(LabelSize::Small).color(Color::Error)),
                )
                .into_any_element(),
        )
    }

    fn render_entry_fields(
        &self,
        method: &acp::AuthMethod,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let agent_name = self.thread.read(cx).agent_name().clone();
        let has_error = self.input_error.is_some();
        let error = self
            .input_error
            .clone()
            .map(|error| Label::new(error).size(LabelSize::Small).color(Color::Error));
        match login_input(method) {
            LoginInput::ApiKey => {
                let is_masked = self.api_key.read(cx).is_masked();
                v_flex()
                    .gap_1p5()
                    .child(field_label("API key"))
                    .child(
                        text_field(&self.api_key, has_error, window, cx)
                            .font_buffer(cx)
                            .text_size(rems_from_px(12_f32))
                            .pr_1()
                            .child(
                                IconButton::new(
                                    "login-reveal-key",
                                    if is_masked {
                                        IconName::Eye
                                    } else {
                                        IconName::EyeOff
                                    },
                                )
                                .icon_size(IconSize::XSmall)
                                .icon_color(Color::Muted)
                                .tooltip(Tooltip::text(if is_masked {
                                    "Show Key"
                                } else {
                                    "Hide Key"
                                }))
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.api_key.update(cx, |input, cx| {
                                            input.set_masked(!is_masked, cx)
                                        })
                                    },
                                )),
                            ),
                    )
                    .children(error)
                    .child(
                        Label::new(format!(
                            "{agent_name} keeps the key. agentZ doesn't store it."
                        ))
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                    )
                    .into_any_element()
            }
            LoginInput::Gateway => v_flex()
                .gap_4()
                .child(
                    v_flex()
                        .gap_1p5()
                        .child(field_label("Base URL"))
                        .child(
                            text_field(&self.base_url, has_error, window, cx)
                                .font_buffer(cx)
                                .text_size(rems_from_px(12_f32)),
                        )
                        .children(error),
                )
                .child(
                    v_flex()
                        .gap_1p5()
                        .child(field_label("Headers"))
                        .children(self.headers.iter().enumerate().map(|(index, row)| {
                            h_flex()
                                .gap_2()
                                .child(
                                    div().w(px(132.)).flex_none().child(
                                        text_field(&row.name, false, window, cx)
                                            .font_buffer(cx)
                                            .text_size(rems_from_px(12_f32)),
                                    ),
                                )
                                .child(
                                    div().flex_1().min_w_0().child(
                                        text_field(&row.value, false, window, cx)
                                            .font_buffer(cx)
                                            .text_size(rems_from_px(12_f32)),
                                    ),
                                )
                                .child(
                                    IconButton::new(("remove-header", index), IconName::Close)
                                        .icon_size(IconSize::Small)
                                        .icon_color(Color::Muted)
                                        .tooltip(Tooltip::text("Remove Header"))
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if index < this.headers.len() {
                                                this.headers.remove(index);
                                            }
                                            cx.notify();
                                        })),
                                )
                        }))
                        .child(
                            h_flex().child(
                                Button::new("add-header", "Add Header")
                                    .style(ButtonStyle::Subtle)
                                    .label_size(LabelSize::Small)
                                    .color(Color::Muted)
                                    .start_icon(
                                        Icon::new(IconName::Plus)
                                            .size(IconSize::XSmall)
                                            .color(Color::Muted),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| this.add_header(cx))),
                            ),
                        ),
                )
                .child(
                    Label::new(format!(
                        "{agent_name} keeps these. agentZ doesn't store them."
                    ))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
                )
                .into_any_element(),
            LoginInput::Nothing => div().into_any_element(),
        }
    }

    fn render_entry_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .justify_end()
            .gap_2()
            .child(
                ActionButton::new("login-entry-cancel", "Cancel")
                    .style(ActionStyle::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| this.stop_entering(cx))),
            )
            .child(
                ActionButton::new("login-entry-submit", "Log In")
                    .style(ActionStyle::Primary)
                    .disabled(!self.can_submit(cx))
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
            .into_any_element()
    }

    /// What the login in progress waits on: the agent's request to open a page, or the links
    /// and code it printed (a remote machine can't open a browser here), or the code in what it
    /// said when it asked for the login (Factory Droid's pairing code).
    fn login_wait(&self, cx: &App) -> LoginWait {
        let thread = self.thread.read(cx);
        let elicitation = login_elicitation(thread).cloned();
        let url = elicitation
            .as_ref()
            .and_then(|elicitation| elicitation.url())
            .map(|url| SharedString::from(url.to_string()))
            .or_else(|| thread.login_page().cloned())
            .or_else(|| thread.auth_links().first().cloned());
        let code = elicitation
            .as_ref()
            .and_then(|elicitation| login_code(&elicitation.request.message))
            .or_else(|| thread.auth_code().map(|code| code.to_string()))
            .or_else(|| thread.auth_description().and_then(|text| login_code(text)));
        let must_open = match &elicitation {
            Some(elicitation) => !elicitation.opened,
            None => thread.login_page().is_some_and(|page| {
                url.as_ref() == Some(page) && self.opened_page.as_ref() != Some(page)
            }),
        };
        LoginWait {
            url,
            code,
            elicitation,
            must_open,
        }
    }

    /// The login in progress, centered: a one-time code to enter, the browser to finish in, or
    /// only that it's logging in.
    fn render_in_progress(&self, method: &acp::AuthMethod, cx: &mut Context<Self>) -> AnyElement {
        let agent_name = self.thread.read(cx).agent_name().clone();
        let wait = self.login_wait(cx);
        let host = wait.url.as_deref().and_then(link_host);
        let wait = std::rc::Rc::new(wait);
        let cancel = |label: &'static str, cx: &mut Context<Self>| {
            ActionButton::new("login-cancel", label)
                .style(ActionStyle::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.cancel_login(cx)))
        };
        let open_button = |label: String, style: ActionStyle, cx: &mut Context<Self>| {
            let wait = wait.clone();
            ActionButton::new("login-open", label)
                .style(style)
                .end_icon(Icon::new(IconName::ArrowUpRight).size(IconSize::XSmall))
                .on_click(cx.listener(move |this, _, _, cx| this.open(&wait, cx)))
        };
        let panel = v_flex().items_center().gap_3().text_center();
        // The dialog has its own Cancel.
        let in_dialog = self.layout == LoginLayout::Dialog;

        if let Some(code) = wait.code.clone() {
            let copy_code = {
                let code = code.clone();
                ActionButton::new(
                    "login-copy-code",
                    if self.copied == Some(Copied::Code) {
                        "Copied"
                    } else {
                        "Copy Code"
                    },
                )
                .start_icon(Icon::new(IconName::Copy).size(IconSize::XSmall))
                .on_click(cx.listener(move |this, _, _, cx| this.copy(Copied::Code, &code, cx)))
            };
            let instruction = match &host {
                Some(host) => h_flex()
                    .gap_1()
                    .child(
                        Label::new("Enter this code at")
                            .size(LabelSize::Custom(rems_from_px(13_f32)))
                            .color(Color::Muted),
                    )
                    .child(Label::new(host.clone()).size(LabelSize::Custom(rems_from_px(13_f32)))),
                None => h_flex().child(
                    Label::new(format!("Enter this code where {agent_name} asks for it"))
                        .size(LabelSize::Custom(rems_from_px(13_f32)))
                        .color(Color::Muted),
                ),
            };
            return panel
                .child(instruction)
                .child(code_boxes(&code, cx))
                .child(
                    h_flex()
                        .mt_1p5()
                        .gap_2()
                        .child(copy_code)
                        .when_some(host, |row, host| {
                            row.child(open_button(
                                format!("Open {host}"),
                                ActionStyle::Primary,
                                cx,
                            ))
                        }),
                )
                .child(
                    h_flex()
                        .mt_1()
                        .gap_1p5()
                        .child(spinner(Color::Muted))
                        .child(
                            Label::new(if in_dialog {
                                "Waiting for you to finish"
                            } else {
                                "Waiting for you to finish ·"
                            })
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                        )
                        .when(!in_dialog, |row| {
                            row.child(
                                Button::new("login-cancel", "Cancel")
                                    .style(ButtonStyle::Transparent)
                                    .label_size(LabelSize::Small)
                                    .color(Color::Muted)
                                    .on_click(cx.listener(|this, _, _, cx| this.cancel_login(cx))),
                            )
                        }),
                )
                .into_any_element();
        }

        if login_input(method) != LoginInput::Nothing {
            return panel
                .child(spinner(Color::Accent))
                .child(Label::new(format!("Logging in to {agent_name}…")))
                .when(!in_dialog, |panel| panel.child(cancel("Cancel", cx)))
                .into_any_element();
        }

        // A page the agent asked to open that hasn't been yet waits on the user to open it.
        let must_open = wait.must_open;
        let (title, message): (SharedString, SharedString) = if must_open {
            (
                "Continue in your browser".into(),
                match (&wait.elicitation, &host) {
                    (Some(elicitation), _) if !elicitation.request.message.trim().is_empty() => {
                        elicitation.request.message.clone().into()
                    }
                    (_, Some(host)) => {
                        format!("{agent_name} needs you to log in at {host}.").into()
                    }
                    _ => format!("{agent_name} needs you to log in in your browser.").into(),
                },
            )
        } else {
            (
                "Finish logging in in your browser".into(),
                match &host {
                    Some(host) => format!(
                        "{agent_name} opened {host}. This updates by itself once you're done."
                    )
                    .into(),
                    None => format!(
                        "{agent_name} opens its login page in your browser. This updates by \
                         itself once you're done."
                    )
                    .into(),
                },
            )
        };
        let copy_link = wait.url.clone().map(|url| {
            ActionButton::new(
                "login-copy-link",
                if self.copied == Some(Copied::Link) {
                    "Copied"
                } else {
                    "Copy Link"
                },
            )
            .start_icon(Icon::new(IconName::Copy).size(IconSize::XSmall))
            .on_click(cx.listener(move |this, _, _, cx| this.copy(Copied::Link, &url, cx)))
        });
        let open = wait.url.as_ref().map(|_| {
            if must_open {
                open_button(
                    host.as_ref()
                        .map_or("Open Page".to_string(), |host| format!("Open {host}")),
                    ActionStyle::Primary,
                    cx,
                )
            } else if in_dialog {
                open_button("Open the Page Again".to_string(), ActionStyle::Outline, cx)
            } else {
                open_button("Open Again".to_string(), ActionStyle::Outline, cx)
            }
        });
        panel
            .child(spinner(Color::Accent))
            .child(Label::new(title).size(LabelSize::Custom(rems_from_px(16_f32))))
            .child(
                div().max_w(px(380.)).child(
                    Label::new(message)
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                ),
            )
            .children(self.forward_error.clone().map(|error| {
                div()
                    .max_w(px(380.))
                    .child(Label::new(error).size(LabelSize::Small).color(Color::Error))
            }))
            .when(copy_link.is_some() || open.is_some(), |panel| {
                let buttons = h_flex().mt_1().gap_2();
                panel.child(if in_dialog {
                    buttons.children(open).children(copy_link)
                } else {
                    buttons.children(copy_link).children(open)
                })
            })
            .when(!in_dialog, |panel| panel.child(cancel("Cancel", cx)))
            .into_any_element()
    }

    /// Where the dialog's login stands.
    pub(crate) fn dialog_step(&self, cx: &App) -> LoginStep {
        if self.thread.read(cx).is_authenticating() {
            return LoginStep::InProgress;
        }
        if self.entering.is_some() {
            return LoginStep::Entering {
                can_submit: self.can_submit(cx),
            };
        }
        if self.chosen.is_none() {
            return LoginStep::Choosing;
        }
        if self.chosen_failure(cx).is_some() {
            LoginStep::Failed
        } else {
            LoginStep::InProgress
        }
    }

    /// Why the picked method didn't log in, once it's tried.
    fn chosen_failure(&self, cx: &App) -> Option<SharedString> {
        self.chosen.as_ref()?;
        let failure = self.failure(cx)?;
        (self.chosen_started || self.failure_when_chosen.as_ref() != Some(&failure))
            .then_some(failure)
    }

    /// Why the last login didn't work: what the agent said, or the terminal login's error.
    fn failure(&self, cx: &App) -> Option<SharedString> {
        if let Some((_, view)) = &self.login_terminal
            && let Some(error) = view.read(cx).terminal().read(cx).error()
        {
            return Some(error.clone());
        }
        self.thread.read(cx).auth_error().cloned()
    }

    /// The dialog's Back: the login in progress stops, and the methods show again.
    pub(crate) fn back(&mut self, cx: &mut Context<Self>) {
        if self.thread.read(cx).is_authenticating() {
            self.cancel_login(cx);
        }
        self.stop_entering(cx);
        self.chosen = None;
        self.login_terminal = None;
        cx.notify();
    }

    /// The dialog's body: the methods, or the picked one's form, terminal, progress or failure.
    fn render_dialog(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if let Some(method) = self.thread.read(cx).authenticating().cloned() {
            return div()
                .py_4()
                .child(self.render_in_progress(&method, cx))
                .into_any_element();
        }
        if let Some(method) = self.entering.clone() {
            // Escape goes on to the dialog, which closes.
            return v_flex()
                .key_context(KEY_CONTEXT)
                .on_action(cx.listener(|this, _: &menu::Confirm, _, cx| this.submit(cx)))
                .gap_4()
                .child(render_method_heading(
                    &method,
                    self.thread.read(cx).agent_name(),
                    cx,
                ))
                .child(self.render_entry_fields(&method, window, cx))
                .into_any_element();
        }
        if let Some(terminal) = self.running_terminal_login(cx).map(|(_, view)| view) {
            return self.render_terminal(terminal, cx);
        }
        let Some(method) = self.chosen.clone() else {
            return self.render_method_list(window, cx);
        };
        if let Some(error) = self.chosen_failure(cx) {
            return v_flex()
                .py_4()
                .items_center()
                .gap_3()
                .text_center()
                .child(
                    Icon::new(IconName::XCircle)
                        .size(IconSize::Medium)
                        .color(Color::Error),
                )
                .child(Label::new("Couldn't log in"))
                .child(
                    div()
                        .max_w(px(380.))
                        .child(Label::new(error).size(LabelSize::Small).color(Color::Muted)),
                )
                .child(div().debug_selector(|| "login-try-again".into()).child(
                    ActionButton::new("login-try-again", "Try Again").on_click(cx.listener(
                        move |this, _, window, cx| this.choose(method.clone(), window, cx),
                    )),
                ))
                .into_any_element();
        }
        // Picked, until the agent says it's logging in.
        let agent_name = self.thread.read(cx).agent_name().clone();
        v_flex()
            .py_4()
            .items_center()
            .gap_3()
            .child(spinner(Color::Accent))
            .child(Label::new(format!("Logging in to {agent_name}…")))
            .into_any_element()
    }

    /// The dialog's first step: each method a row to pick, with what the agent said about
    /// logging in above them.
    fn render_method_list(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let thread = self.thread.read(cx);
        let agent_name = thread.agent_name().clone();
        let methods = thread.auth_methods().to_vec();
        v_flex()
            .gap_0p5()
            .children(self.render_description(window, cx))
            .children(methods.into_iter().map(|method| {
                let id = method.id().0.to_string();
                let description = method_description(&method, &agent_name);
                let selector = format!("login-method-{id}");
                h_flex()
                    .id(SharedString::from(format!("login-method-{id}")))
                    .debug_selector(move || selector)
                    .px_2p5()
                    .py(px(9.))
                    .gap_3()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|row| row.bg(colors.element_hover))
                    .child(icon_tile(
                        Icon::new(method_icon(&method))
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                        px(24.),
                        cx,
                    ))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_px()
                            .child(Label::new(method.name().to_string()))
                            .children(description.map(|description| {
                                Label::new(description)
                                    .size(LabelSize::XSmall)
                                    .color(Color::Muted)
                            })),
                    )
                    .child(
                        Icon::new(IconName::ChevronRight)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.choose(method.clone(), window, cx)
                    }))
            }))
            .into_any_element()
    }

    /// The account card's rows, under the status row the settings page shows.
    fn render_rows(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if let Some(method) = self.thread.read(cx).authenticating().cloned() {
            return div()
                .px_6()
                .py(px(26.))
                .child(self.render_in_progress(&method, cx))
                .into_any_element();
        }
        let methods = self.thread.read(cx).auth_methods().to_vec();
        v_flex()
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &menu::Confirm, _, cx| this.submit(cx)))
            .on_action(cx.listener(|this, _: &menu::Cancel, _, cx| this.stop_entering(cx)))
            .children(self.render_description(window, cx).map(|description| {
                // Reads as part of the status row above, aligned with its text.
                div().pl(px(35.)).pr_4().pb_3().child(description)
            }))
            .children(
                methods
                    .iter()
                    .enumerate()
                    .map(|(index, method)| self.render_method_row(index, method, window, cx)),
            )
            .into_any_element()
    }

    /// A thread's panel: the agent's icon, what to do, and a button for each method.
    fn render_centered(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let thread = self.thread.read(cx);
        let agent_name = thread.agent_name().clone();
        let methods = thread.auth_methods().to_vec();
        let authenticating = thread.authenticating().cloned();
        let auth_error = thread
            .auth_error()
            .cloned()
            .filter(|_| authenticating.is_none());
        let has_terminal_method = methods.iter().any(logs_in_through_terminal);
        let icon = match self
            .agent_id
            .as_ref()
            .and_then(|agent_id| agent_icon(agent_id, cx))
        {
            Some(markup) => Icon::from_svg_markup(markup),
            None => Icon::new(IconName::Sparkle),
        };
        let panel = v_flex().w_full().items_center().gap_3().child(icon_tile(
            icon.color(Color::Default),
            px(40.),
            cx,
        ));
        if let Some(method) = authenticating {
            return panel
                .child(div().mt_1().child(self.render_in_progress(&method, cx)))
                .into_any_element();
        }
        let panel = panel.child(
            v_flex()
                .items_center()
                .gap_0p5()
                .child(
                    Label::new(format!("Log in to {agent_name}"))
                        .size(LabelSize::Custom(rems_from_px(16_f32))),
                )
                .child(
                    Label::new(format!("Every thread with {agent_name} shares the login."))
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                ),
        );
        let panel = panel
            .children(
                self.render_description(window, cx)
                    .map(|description| div().max_w(px(420.)).child(description)),
            )
            .children(auth_error.map(|error| {
                div()
                    .max_w(px(420.))
                    .child(Label::new(error).size(LabelSize::Small).color(Color::Error))
            }));
        if let Some(method) = self.entering.clone() {
            return panel
                .child(self.render_centered_form(&method, window, cx))
                .into_any_element();
        }
        let terminal = self.running_terminal_login(cx).map(|(_, view)| view);
        panel
            .child(v_flex().w(CENTERED_WIDTH).mt_1p5().gap_2().children(
                methods.into_iter().enumerate().map(|(index, method)| {
                    let id = SharedString::from(format!("login-{}", method.id().0));
                    let icon = Icon::new(method_icon(&method)).size(IconSize::Small);
                    let description = method.description().map(str::to_string);
                    let name = method.name().to_string();
                    ActionButton::new(id, name)
                        .size(ActionSize::Large)
                        .full_width()
                        .style(if index == 0 {
                            ActionStyle::Primary
                        } else {
                            ActionStyle::Outline
                        })
                        .start_icon(icon)
                        .when_some(description, |button, description| {
                            button.tooltip(description)
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.choose(method.clone(), window, cx)
                        }))
                }),
            ))
            .children(terminal.map(|terminal| {
                div()
                    .w_full()
                    .max_w(px(560.))
                    .mt_2()
                    .child(self.render_terminal(terminal, cx))
            }))
            .when(has_terminal_method, |panel| {
                panel.child(
                    h_flex()
                        .gap_1()
                        .child(
                            Label::new("Logged in another way?")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        )
                        .child(
                            Button::new("login-retry", "Check Again")
                                .style(ButtonStyle::Transparent)
                                .label_size(LabelSize::Small)
                                .color(Color::Accent)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.thread
                                        .update(cx, |thread, cx| thread.retry_session(cx));
                                })),
                        ),
                )
            })
            .into_any_element()
    }

    /// An API key or gateway form in a thread, in a card under the title.
    fn render_centered_form(
        &self,
        method: &acp::AuthMethod,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        v_flex()
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(|this, _: &menu::Confirm, _, cx| this.submit(cx)))
            .on_action(cx.listener(|this, _: &menu::Cancel, _, cx| this.stop_entering(cx)))
            .w(px(380.))
            .mt_1p5()
            .p_4()
            .gap_4()
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .bg(colors.editor_background)
            .text_left()
            .child(
                h_flex()
                    .gap_3()
                    .child(icon_tile(
                        Icon::new(method_icon(method)).color(Color::Muted),
                        px(28.),
                        cx,
                    ))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(Label::new(method.name().to_string()))
                            .children(
                                method_description(method, self.thread.read(cx).agent_name()).map(
                                    |description| {
                                        Label::new(description)
                                            .size(LabelSize::Small)
                                            .color(Color::Muted)
                                    },
                                ),
                            ),
                    ),
            )
            .child(self.render_entry_fields(method, window, cx))
            .child(self.render_entry_actions(cx))
            .into_any_element()
    }
}

/// The ports to forward from the agent's machine for a login page: the page's own, when it's on
/// `localhost`, and those of the `localhost` addresses it sends the browser back to (OAuth's
/// `redirect_uri`, and whatever else it names in its query).
fn loopback_forwards(page: &str) -> Vec<LocalForward> {
    let Ok(page) = url::Url::parse(page) else {
        return Vec::new();
    };
    let named = page
        .query_pairs()
        .filter_map(|(_, value)| url::Url::parse(&value).ok());
    let mut forwards = Vec::new();
    for url in std::iter::once(page.clone()).chain(named) {
        if !matches!(url.scheme(), "http" | "https") {
            continue;
        }
        let host = match url.host() {
            Some(url::Host::Domain("localhost")) => "localhost".to_string(),
            Some(url::Host::Ipv4(address)) if address.is_loopback() => address.to_string(),
            Some(url::Host::Ipv6(address)) if address.is_loopback() => address.to_string(),
            _ => continue,
        };
        let Some(port) = url.port() else {
            continue;
        };
        let forward = LocalForward { host, port };
        if !forwards.contains(&forward) {
            forwards.push(forward);
        }
    }
    forwards
}

/// While a login runs, the page the agent asks to open for it (a device or browser login):
/// the login panel shows it, not a card of its own.
pub(crate) fn login_elicitation(thread: &ThreadView) -> Option<&Elicitation> {
    if !thread.is_authenticating() {
        return None;
    }
    thread.elicitations().iter().find(|elicitation| {
        elicitation.url().is_some()
            && matches!(
                elicitation.request.scope(),
                acp::ElicitationScope::Request(_)
            )
    })
}

/// A method's icon, name and description, over its form.
fn render_method_heading(method: &acp::AuthMethod, agent_name: &SharedString, cx: &App) -> Div {
    h_flex()
        .gap_3()
        .child(icon_tile(
            Icon::new(method_icon(method)).color(Color::Muted),
            px(28.),
            cx,
        ))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(Label::new(method.name().to_string()))
                .children(method_description(method, agent_name).map(|description| {
                    Label::new(description)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                })),
        )
}

/// A method's icon by how it logs in: a browser, a key, a gateway, or a terminal.
fn method_icon(method: &acp::AuthMethod) -> IconName {
    if logs_in_through_terminal(method) {
        return IconName::Terminal;
    }
    match login_input(method) {
        LoginInput::Nothing => IconName::ToolWeb,
        LoginInput::ApiKey => IconName::Lock,
        LoginInput::Gateway => IconName::Server,
    }
}

/// The agent's description of the method, or what a terminal method does when it gives none.
fn method_description(method: &acp::AuthMethod, agent_name: &SharedString) -> Option<String> {
    if let Some(description) = method.description().filter(|text| !text.trim().is_empty()) {
        return Some(description.to_string());
    }
    logs_in_through_terminal(method)
        .then(|| format!("Runs {agent_name}'s login in a terminal where it runs."))
}

impl Render for AgentLogin {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.layout {
            LoginLayout::Rows => self.render_rows(window, cx),
            LoginLayout::Centered => self.render_centered(window, cx),
            LoginLayout::Dialog => self.render_dialog(window, cx),
        };
        div().w_full().text_ui(cx).child(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forwards(page: &str) -> Vec<(String, u16)> {
        loopback_forwards(page)
            .into_iter()
            .map(|forward| (forward.host, forward.port))
            .collect()
    }

    #[test]
    fn forwards_the_ports_login_pages_come_back_to() {
        // Devin's, Codex's and Claude Code's pages, which send the browser back to the agent.
        assert_eq!(
            forwards(
                "https://app.devin.ai/auth/cli/continue?redirect_uri=http%3A%2F%2F127.0.0.1%3A43607\
                 %2Fcallback&state=49bcf2a7"
            ),
            [("127.0.0.1".to_string(), 43607)]
        );
        assert_eq!(
            forwards(
                "https://auth.openai.com/oauth/authorize?response_type=code&redirect_uri=\
                 http%3A%2F%2Flocalhost%3A1455%2Fauth%2Fcallback&scope=openid"
            ),
            [("localhost".to_string(), 1455)]
        );
        // A page served by the agent itself.
        assert_eq!(
            forwards("http://localhost:8085/start"),
            [("localhost".to_string(), 8085)]
        );
        assert_eq!(
            forwards("https://example.com/login?redirect_uri=http%3A%2F%2F%5B%3A%3A1%5D%3A9000%2F"),
            [("::1".to_string(), 9000)]
        );
        // Device logins and manual codes come back to nothing local.
        assert_eq!(forwards("https://auth.openai.com/codex/device"), []);
        assert_eq!(
            forwards(
                "https://claude.ai/oauth/authorize?code=true&redirect_uri=https%3A%2F%2F\
                 console.anthropic.com%2Foauth%2Fcode%2Fcallback"
            ),
            []
        );
        assert_eq!(forwards("http://localhost/no-port"), []);
    }
}
