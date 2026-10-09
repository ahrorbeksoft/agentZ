//! Logging in to an agent with the methods it offers, alike wherever it shows: each method is
//! a row (its icon, its name and the agent's description of it) to pick, and the one picked
//! shows its step in their place, with Back to them: a one-time code to enter, the browser to
//! finish in, its terminal on the agent's machine, or the API key or gateway it takes. In an
//! empty thread the login is a card in the middle; after a message, the thread opens it in a
//! dialog ([`LoginDialog`]). Add Account's dialog and an account's card on the agent's
//! settings page show the same rows and steps.

use std::rc::Rc;
use std::time::Duration;

use agentz_client::ssh::{HeldForward, LocalForward, Ssh};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::AgentId;
use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::thread::{
    ConnectionStatus, Elicitation, LoginInput, ThreadView, api_key_meta, gateway_meta, login_code,
    login_input, logs_in_through_terminal,
};
use gpui::{
    AnyElement, App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
    KeyBinding, MouseButton, SharedString, Subscription, Task, Window, div,
};
use markdown::{Markdown, MarkdownElement, MarkdownFont, MarkdownStyle};
use text_input::{TextInput, TextInputEvent};
use ui::{Tooltip, prelude::*};

use crate::agent_icons::agent_icon;
use crate::controls::{
    ActionButton, ActionStyle, code_boxes, copy_to_clipboard, dialog_frame, dialog_title,
    field_label, icon_tile, link_host, spinner, text_field,
};
use crate::server_client::Transport;
use crate::settings_page::{ADD_ACCOUNT_DIALOG_WIDTH, logged_in_title};
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;
use crate::thread_entity::AgentThread;

const KEY_CONTEXT: &str = "AgentLoginForm";
const DIALOG_KEY_CONTEXT: &str = "LoginDialog";
/// Room for a login command's prompts, a URL and a pasted code.
const LOGIN_TERMINAL_HEIGHT: Pixels = px(240.);
/// A thread's login card, a little narrower than Add Account's dialog.
const CARD_WIDTH: Pixels = px(440.);
const COPIED_FOR: Duration = Duration::from_secs(2);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(DIALOG_KEY_CONTEXT)),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoginLayout {
    /// In an account's card on the agent's settings page, under its status row. A step has
    /// Back under it.
    Rows,
    /// A card in the middle of an empty thread, under the agent's icon and "Log in to …". A
    /// step has Back under it.
    Card,
    /// A dialog's body (Add Account's, or a thread's [`LoginDialog`]), which has its own
    /// buttons for Back and Cancel.
    Dialog,
}

/// Where a login stands, for the buttons under its step.
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
    /// For the agent's icon on a thread's card.
    agent_id: Option<AgentId>,
    /// The account the login is for, named when the agent says nothing about logging in and
    /// has more than one: else every thread with it shares the login.
    account: Option<SharedString>,
    /// Where a terminal login method runs, on the agent's machine, with the method.
    login_terminal: Option<(acp::AuthMethodId, Entity<TerminalView>)>,
    /// The API-key or gateway method whose details are being typed in.
    entering: Option<acp::AuthMethod>,
    /// The method last picked, whose step shows until Back, and which Try Again tries again
    /// after it fails.
    chosen: Option<acp::AuthMethod>,
    /// Whether a login was under way when the thread last changed, to notice one finishing.
    was_authenticating: bool,
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
    /// step stops asking to open it.
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
                let thread = thread.read(cx);
                let is_authenticating = thread.is_authenticating();
                let finished = std::mem::replace(&mut this.was_authenticating, is_authenticating)
                    && !is_authenticating
                    && thread.auth_error().is_none();
                // A card's next login starts from the methods: once this one's done, or as the
                // agent starts again logged in after a terminal login. A dialog keeps the
                // method, to say how it logged in.
                if this.layout != LoginLayout::Dialog
                    && (finished || thread.status() == &ConnectionStatus::Connecting)
                {
                    this.chosen = None;
                    this.chosen_started = false;
                    this.login_terminal = None;
                }
                // The login's page and its forwarded ports are done with once it's over.
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
            account: None,
            login_terminal: None,
            entering: None,
            chosen: None,
            was_authenticating: false,
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

    /// Names the account the login is for, while the agent has more than one.
    pub(crate) fn set_account(&mut self, account: Option<SharedString>, cx: &mut Context<Self>) {
        if self.account != account {
            self.account = account;
            cx.notify();
        }
    }

    /// The method last picked, to say how the login went in.
    pub(crate) fn chosen_method(&self) -> Option<&acp::AuthMethod> {
        self.chosen.as_ref()
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

    /// What the agent said when it asked for the login, as markdown (Zed shows it so).
    fn render_description(&self, window: &Window, cx: &App) -> Option<AnyElement> {
        let (_, markdown) = self.description.as_ref()?;
        let mut style = MarkdownStyle::themed(MarkdownFont::Agent, window, cx);
        style.base_text_style.font_size = rems_from_px(12_f32).into();
        style.base_text_style.color = cx.theme().colors().text_muted;
        Some(MarkdownElement::new(markdown.clone(), style).into_any_element())
    }

    /// Under a card's title, as Zed's: what the agent said, or when it says nothing, that every
    /// thread with it shares the login, or the account it's for.
    fn render_head_line(&self, window: &Window, cx: &App) -> AnyElement {
        if let Some(description) = self.render_description(window, cx) {
            return description;
        }
        let line = self.account.clone().unwrap_or_else(|| {
            format!(
                "Every thread with {} shares the login.",
                self.thread.read(cx).agent_name()
            )
            .into()
        });
        Label::new(line)
            .size(LabelSize::Small)
            .color(Color::Muted)
            .into_any_element()
    }

    /// Each method a row to pick: its icon in a tile, its name, the agent's description of it
    /// and a chevron. None is put forward.
    fn render_methods(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let thread = self.thread.read(cx);
        let agent_name = thread.agent_name().clone();
        let methods = thread.auth_methods().to_vec();
        v_flex()
            .gap_0p5()
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
                        Icon::new(method_icon(&method)).color(Color::Muted),
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

    /// The picked method's step, in the methods' place: the login in progress, the key or
    /// gateway to type in, its terminal, or why it failed. `None` until one is picked.
    fn render_step(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let thread = self.thread.read(cx);
        let agent_name = thread.agent_name().clone();
        if let Some(method) = thread.authenticating().cloned() {
            return Some(self.render_in_progress(&method, cx));
        }
        if let Some(method) = self.entering.clone() {
            let in_dialog = self.layout == LoginLayout::Dialog;
            let heading = render_method_heading(&method, cx);
            let fields = self.render_entry_fields(&method, window, cx);
            return Some(
                v_flex()
                    .key_context(KEY_CONTEXT)
                    .on_action(cx.listener(|this, _: &menu::Confirm, _, cx| this.submit(cx)))
                    // In a dialog, Escape goes on to the dialog, which closes.
                    .when(!in_dialog, |step| {
                        step.on_action(cx.listener(|this, _: &menu::Cancel, _, cx| this.back(cx)))
                    })
                    .gap_3()
                    .child(heading)
                    .child(fields)
                    .into_any_element(),
            );
        }
        if let Some((method_id, terminal)) = self.running_terminal_login(cx) {
            let heading = self
                .thread
                .read(cx)
                .auth_methods()
                .iter()
                .find(|method| method.id() == method_id)
                .map(|method| render_method_heading(method, cx));
            let terminal = self.render_terminal(terminal, cx);
            return Some(
                v_flex()
                    .gap_2()
                    .children(heading)
                    .child(terminal)
                    .into_any_element(),
            );
        }
        let method = self.chosen.clone()?;
        if let Some(error) = self.chosen_failure(cx) {
            return Some(
                v_flex()
                    .py_2()
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
                    .into_any_element(),
            );
        }
        // Picked, until the agent says it's logging in.
        Some(
            v_flex()
                .py_2()
                .items_center()
                .gap_3()
                .child(spinner(Color::Accent))
                .child(Label::new(format!("Logging in to {agent_name}…")))
                .into_any_element(),
        )
    }

    /// Under a card's step: Back to the methods, and Cancel, or Log In for a key or a gateway.
    fn render_footer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (id, action) = match self.step(cx) {
            LoginStep::Choosing => return None,
            LoginStep::Entering { can_submit } => (
                "login-submit",
                ActionButton::new("login-submit", "Log In")
                    .style(ActionStyle::Primary)
                    .disabled(!can_submit)
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            ),
            LoginStep::InProgress | LoginStep::Failed => (
                "login-cancel",
                ActionButton::new("login-cancel", "Cancel")
                    .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
            ),
        };
        Some(
            h_flex()
                .justify_end()
                .gap_2()
                .child(
                    div().debug_selector(|| "login-back".into()).child(
                        ActionButton::new("login-back", "Back")
                            .style(ActionStyle::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.back(cx))),
                    ),
                )
                .child(div().debug_selector(move || id.into()).child(action))
                .into_any_element(),
        )
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

    /// The API key, or the gateway's address and headers, as the method asks.
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

    /// The login in progress: a one-time code to enter, the browser to finish in, or only that
    /// it's logging in.
    fn render_in_progress(&self, method: &acp::AuthMethod, cx: &mut Context<Self>) -> AnyElement {
        let agent_name = self.thread.read(cx).agent_name().clone();
        let wait = self.login_wait(cx);
        let host = wait.url.as_deref().and_then(link_host);
        let wait = Rc::new(wait);
        let open_button = |label: String, style: ActionStyle, cx: &mut Context<Self>| {
            let wait = wait.clone();
            ActionButton::new("login-open", label)
                .style(style)
                .end_icon(Icon::new(IconName::ArrowUpRight).size(IconSize::XSmall))
                .on_click(cx.listener(move |this, _, _, cx| this.open(&wait, cx)))
        };
        let panel = v_flex().py_2().items_center().gap_3().text_center();

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
                            Label::new("Waiting for you to finish")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        ),
                )
                .into_any_element();
        }

        if login_input(method) != LoginInput::Nothing {
            return panel
                .child(spinner(Color::Accent))
                .child(Label::new(format!("Logging in to {agent_name}…")))
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
            } else {
                open_button("Open the Page Again".to_string(), ActionStyle::Outline, cx)
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
                panel.child(h_flex().mt_1().gap_2().children(open).children(copy_link))
            })
            .into_any_element()
    }

    /// Where the login stands, for the buttons under its step.
    pub(crate) fn step(&self, cx: &App) -> LoginStep {
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

    /// Back: the login in progress stops, and the methods show again.
    pub(crate) fn back(&mut self, cx: &mut Context<Self>) {
        if self.thread.read(cx).is_authenticating() {
            self.cancel_login(cx);
        }
        self.stop_entering(cx);
        self.chosen = None;
        self.login_terminal = None;
        cx.notify();
    }

    /// A dialog's body: what the agent said and the methods, or the picked one's step.
    fn render_dialog(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if let Some(step) = self.render_step(window, cx) {
            return step;
        }
        v_flex()
            .children(
                self.render_description(window, cx)
                    .map(|description| div().mb_2p5().child(description)),
            )
            .child(self.render_methods(cx))
            .into_any_element()
    }

    /// An account card's rows, under the status row the settings page shows: what the agent
    /// said and the methods, or the picked one's step with Back.
    fn render_rows(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        // The Account tab shows no status row over a login under way.
        let is_authenticating = self.thread.read(cx).is_authenticating();
        if let Some(step) = self.render_step(window, cx) {
            return v_flex()
                .px_4()
                .py_3()
                .gap_3()
                .when(!is_authenticating, |rows| {
                    rows.border_t_1().border_color(colors.border_variant)
                })
                .child(step)
                .children(self.render_footer(cx))
                .into_any_element();
        }
        v_flex()
            .children(self.render_description(window, cx).map(|description| {
                // Reads as part of the status row above, aligned with its text.
                div().pl(px(35.)).pr_4().pb_3().child(description)
            }))
            .child(
                div()
                    .px(px(6.))
                    .pt_1()
                    .pb(px(6.))
                    .border_t_1()
                    .border_color(colors.border_variant)
                    .child(self.render_methods(cx)),
            )
            .into_any_element()
    }

    /// A thread's card, in the look of Add Account's dialog: the agent's icon and "Log in to …"
    /// on one line, with what the agent said under it, then the methods, or the picked one's
    /// step with Back.
    fn render_card(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors().clone();
        let thread = self.thread.read(cx);
        let agent_name = thread.agent_name().clone();
        let has_terminal_method = thread.auth_methods().iter().any(logs_in_through_terminal);
        let auth_error = thread.auth_error().cloned();
        let icon = match self
            .agent_id
            .as_ref()
            .and_then(|agent_id| agent_icon(agent_id, cx))
        {
            Some(markup) => Icon::from_svg_markup(markup),
            None => Icon::new(IconName::Sparkle),
        };
        let step = self.render_step(window, cx);
        let is_choosing = step.is_none();
        let head_line = self.render_head_line(window, cx);
        let auth_error = auth_error
            .filter(|_| is_choosing)
            .map(|error| Label::new(error).size(LabelSize::Small).color(Color::Error));
        let body = match step {
            Some(step) => step,
            None => {
                let check_again = has_terminal_method.then(|| {
                    h_flex()
                        .px_2p5()
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
                        )
                });
                v_flex()
                    .gap_2()
                    .child(self.render_methods(cx))
                    .children(check_again)
                    .into_any_element()
            }
        };
        let footer = self.render_footer(cx);
        v_flex()
            .debug_selector(|| "login-card".into())
            .w(CARD_WIDTH)
            .p_4()
            .gap(px(14.))
            .rounded(px(10.))
            .border_1()
            .border_color(colors.border)
            .bg(colors.elevated_surface_background)
            .child(
                h_flex()
                    .gap_3()
                    .child(icon_tile(icon.color(Color::Default), px(32.), cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_0p5()
                            .child(
                                Label::new(format!("Log in to {agent_name}"))
                                    .size(LabelSize::Custom(rems_from_px(15_f32))),
                            )
                            .child(head_line)
                            .children(auth_error),
                    ),
            )
            .child(body)
            .children(footer)
            .into_any_element()
    }
}

/// A thread's login after a message: Add Account's dialog over the thread, titled "Log in to
/// …", with the methods, then the picked one's step, then who it's logged in as, with Done.
/// The thread's "needs a login" line opens it; it doesn't open by itself.
pub struct LoginDialog {
    thread: Entity<AgentThread>,
    login: Entity<AgentLogin>,
    /// Over the methods when the agent says nothing: that every thread with it shares the
    /// login, or the account it's for.
    line: SharedString,
    focus_handle: FocusHandle,
    _subscriptions: [Subscription; 2],
}

impl LoginDialog {
    pub fn new(
        thread: Entity<AgentThread>,
        agent_id: Option<AgentId>,
        line: SharedString,
        cx: &mut Context<Self>,
    ) -> Self {
        let login = cx.new(|cx| AgentLogin::new(thread.clone(), LoginLayout::Dialog, agent_id, cx));
        let subscriptions = [
            cx.observe(&thread, |_, _, cx| cx.notify()),
            cx.observe(&login, |_, _, cx| cx.notify()),
        ];
        Self {
            thread,
            login,
            line,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// Cancel, Escape and a click beside it: a login under way stops with it.
    fn dismiss(&mut self, cx: &mut Context<Self>) {
        if self.thread.read(cx).status() == &ConnectionStatus::AuthRequired {
            self.login.update(cx, |login, cx| login.back(cx));
        }
        cx.emit(DismissEvent);
    }

    /// The body and buttons by where the agent stands: logging in, starting again with the
    /// login, logged in, or failed to start.
    fn render_content(&self, cx: &mut Context<Self>) -> (AnyElement, Vec<AnyElement>) {
        let thread = self.thread.read(cx);
        let agent_name = thread.agent_name().clone();
        let status = thread.status().clone();
        let says_nothing = thread.auth_description().is_none();
        let auth_status = thread.auth_status().cloned();
        let wrap = |id: &'static str, button: ActionButton| {
            div()
                .debug_selector(move || id.into())
                .child(button)
                .into_any_element()
        };
        let cancel = |style: ActionStyle, cx: &mut Context<Self>| {
            wrap(
                "login-dialog-cancel",
                ActionButton::new("login-dialog-cancel", "Cancel")
                    .style(style)
                    .on_click(cx.listener(|this, _, _, cx| this.dismiss(cx))),
            )
        };
        let centered = |children: Vec<AnyElement>| {
            v_flex()
                .py_4()
                .items_center()
                .gap_2p5()
                .text_center()
                .children(children)
                .into_any_element()
        };
        match status {
            ConnectionStatus::Ready => {
                let method = self
                    .login
                    .read(cx)
                    .chosen_method()
                    .map(|method| method.name().to_string());
                let title = logged_in_title(auth_status.as_ref(), method.as_deref());
                let body = centered(vec![
                    Icon::new(IconName::Check)
                        .size(IconSize::Medium)
                        .color(Color::Success)
                        .into_any_element(),
                    div()
                        .debug_selector(|| "login-dialog-logged-in".into())
                        .child(Label::new(title))
                        .into_any_element(),
                ]);
                let done = wrap(
                    "login-dialog-done",
                    ActionButton::new("login-dialog-done", "Done")
                        .style(ActionStyle::Primary)
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(DismissEvent))),
                );
                (body, vec![done])
            }
            ConnectionStatus::Connecting => {
                let body = centered(vec![
                    spinner(Color::Muted),
                    Label::new(format!("Starting {agent_name}…"))
                        .color(Color::Muted)
                        .into_any_element(),
                ]);
                (body, vec![cancel(ActionStyle::Ghost, cx)])
            }
            ConnectionStatus::Failed(error) => {
                let body = centered(vec![
                    Icon::new(IconName::XCircle)
                        .size(IconSize::Medium)
                        .color(Color::Error)
                        .into_any_element(),
                    Label::new(format!("Couldn't start {agent_name}")).into_any_element(),
                    Label::new(error)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                        .into_any_element(),
                ]);
                (body, vec![cancel(ActionStyle::Ghost, cx)])
            }
            ConnectionStatus::AuthRequired => {
                let step = self.login.read(cx).step(cx);
                let body = v_flex()
                    .when(step == LoginStep::Choosing && says_nothing, |body| {
                        body.child(
                            div().mb_2p5().child(
                                Label::new(self.line.clone())
                                    .size(LabelSize::Small)
                                    .color(Color::Muted),
                            ),
                        )
                    })
                    .child(self.login.clone())
                    .into_any_element();
                let back = || {
                    let login = self.login.clone();
                    // The step's field goes, and Escape still closes the dialog.
                    let focus_handle = self.focus_handle.clone();
                    wrap(
                        "login-dialog-back",
                        ActionButton::new("login-dialog-back", "Back")
                            .style(ActionStyle::Ghost)
                            .on_click(move |_, window, cx| {
                                login.update(cx, |login, cx| login.back(cx));
                                window.focus(&focus_handle, cx);
                            }),
                    )
                };
                let buttons = match step {
                    LoginStep::Choosing => vec![cancel(ActionStyle::Ghost, cx)],
                    LoginStep::InProgress | LoginStep::Failed => {
                        vec![back(), cancel(ActionStyle::Outline, cx)]
                    }
                    LoginStep::Entering { can_submit } => {
                        let login = self.login.clone();
                        vec![
                            back(),
                            cancel(ActionStyle::Outline, cx),
                            wrap(
                                "login-dialog-log-in",
                                ActionButton::new("login-dialog-log-in", "Log In")
                                    .style(ActionStyle::Primary)
                                    .disabled(!can_submit)
                                    .on_click(move |_, _, cx| {
                                        login.update(cx, |login, cx| login.submit(cx))
                                    }),
                            ),
                        ]
                    }
                };
                (body, buttons)
            }
        }
    }
}

impl EventEmitter<DismissEvent> for LoginDialog {}

impl Focusable for LoginDialog {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for LoginDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let agent_name = self.thread.read(cx).agent_name().clone();
        let (body, buttons) = self.render_content(cx);
        div()
            .id("login-dialog-backdrop")
            .key_context(DIALOG_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &menu::Cancel, _, cx| this.dismiss(cx)))
            .w(viewport.width)
            .h(viewport.height)
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.25))
            // As the shell's modal backdrop: nothing under it gets the mouse, and pressing on it
            // closes the dialog.
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.dismiss(cx)),
            )
            .child(
                dialog_frame(cx)
                    .debug_selector(|| "login-dialog".into())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .w(ADD_ACCOUNT_DIALOG_WIDTH)
                    .px_4()
                    .pt_4()
                    .pb_3p5()
                    .text_ui(cx)
                    .child(dialog_title(format!("Log in to {agent_name}")))
                    .child(body)
                    .child(h_flex().mt_3p5().gap_2().justify_end().children(buttons)),
            )
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
/// the login's step shows it, not a card of its own.
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

/// A method's icon and name, over its step.
fn render_method_heading(method: &acp::AuthMethod, cx: &App) -> Div {
    h_flex()
        .gap_2()
        .child(icon_tile(
            Icon::new(method_icon(method)).color(Color::Muted),
            px(24.),
            cx,
        ))
        .child(Label::new(method.name().to_string()))
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
            LoginLayout::Card => self.render_card(window, cx),
            LoginLayout::Dialog => self.render_dialog(window, cx),
        };
        div()
            .w_full()
            .text_ui(cx)
            .when(self.layout == LoginLayout::Card, |this| {
                this.flex().justify_center()
            })
            .child(content)
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
