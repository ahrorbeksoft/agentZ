//! The conversation with one agent. Layout, spacing and colors follow Zed's agent thread view
//! (`agent_ui::conversation_view::thread_view`).

use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agent_thread::{
    AgentThread, ConnectionStatus, Entry, FileDiff, PlanItem, SessionRestore, ToolCall,
};
use collections::{HashMap, HashSet};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, Hsla, KeyBinding, ScrollHandle,
    Subscription, Task, Window,
};
use markdown::{Markdown, MarkdownElement, MarkdownFont, MarkdownStyle};
use registry::{AgentId, AgentRegistryStore};
use text_input::{TextInput, TextInputEvent};
use ui::{
    Callout, CommonAnimationExt as _, ContextMenu, Disclosure, IconPosition, PopoverMenu, Severity,
    SpinnerLabel, Switch, ToggleState, Tooltip, prelude::*,
};

const KEY_CONTEXT: &str = "AgentComposer";

gpui::actions!(
    agent,
    [
        /// Completes the highlighted slash command in the message editor.
        AcceptSlashCommand,
    ]
);
/// Matches Zed's default `agent.max_content_width`.
const MAX_CONTENT_WIDTH: Pixels = px(850.);
const DIFF_PREVIEW_LINES: usize = 12;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
        // Only acted on while the slash-command menu is open.
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("tab", AcceptSlashCommand, Some(KEY_CONTEXT)),
    ]);
}

/// Identifies one rendered piece of markdown: an entry, and which part of it.
type MarkdownKey = (usize, usize);

pub struct AgentView {
    thread: Entity<AgentThread>,
    title: SharedString,
    registry: Entity<AgentRegistryStore>,
    agent_id: Option<AgentId>,
    composer: Entity<TextInput>,
    scroll_handle: ScrollHandle,
    markdowns: HashMap<MarkdownKey, Entity<Markdown>>,
    expanded_tool_calls: HashSet<acp::ToolCallId>,
    toggled_thoughts: HashSet<usize>,
    plan_expanded: bool,
    edits_expanded: bool,
    /// Messages typed while the agent works; sent one at a time as each turn ends, like Zed.
    queued_messages: Vec<String>,
    queue_expanded: bool,
    command_menu_index: usize,
    /// The composer text for which the user dismissed the slash-command menu.
    command_menu_dismissed_for: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
    _elapsed_refresh: Task<()>,
}

impl AgentView {
    pub fn new(
        thread: Entity<AgentThread>,
        title: SharedString,
        registry: Entity<AgentRegistryStore>,
        agent_id: Option<AgentId>,
        cx: &mut Context<Self>,
    ) -> Self {
        let composer = cx.new(|cx| TextInput::new("Message the agent…", cx));
        let subscriptions = vec![
            cx.observe(&thread, |this, _, cx| {
                // Only follow new output if the user hasn't scrolled up to read.
                let follow = this.is_scrolled_to_bottom();
                this.sync_markdowns(cx);
                if follow {
                    this.scroll_handle.scroll_to_bottom();
                }
                this.send_next_queued_message(cx);
                cx.notify();
            }),
            // The agent's display name and icon come from the registry, which may load later.
            cx.observe(&registry, |_, _, cx| cx.notify()),
        ];
        let mut subscriptions = subscriptions;
        subscriptions.push(cx.subscribe(&composer, |this, _, _: &TextInputEvent, cx| {
            this.command_menu_index = 0;
            cx.notify();
        }));
        // Keeps the elapsed-time label ticking while the agent works.
        let elapsed_refresh = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let still_open = this.update(cx, |this, cx| {
                    if this.thread.read(cx).is_working() {
                        cx.notify();
                    }
                });
                if still_open.is_err() {
                    break;
                }
            }
        });
        let mut this = Self {
            thread,
            title,
            registry,
            agent_id,
            composer,
            scroll_handle: ScrollHandle::new(),
            markdowns: HashMap::default(),
            expanded_tool_calls: HashSet::default(),
            toggled_thoughts: HashSet::default(),
            plan_expanded: false,
            edits_expanded: false,
            queued_messages: Vec::new(),
            queue_expanded: false,
            command_menu_index: 0,
            command_menu_dismissed_for: None,
            _subscriptions: subscriptions,
            _elapsed_refresh: elapsed_refresh,
        };
        this.sync_markdowns(cx);
        this
    }

    pub fn set_title(&mut self, title: SharedString, cx: &mut Context<Self>) {
        self.title = title;
        cx.notify();
    }

    /// Keeps a markdown entity per message so streamed text is appended instead of reparsed.
    fn sync_markdowns(&mut self, cx: &mut Context<Self>) {
        let entries = self.thread.read(cx).entries().to_vec();
        for (index, entry) in entries.iter().enumerate() {
            match entry {
                Entry::UserMessage(text)
                | Entry::AgentMessage(text)
                | Entry::AgentThought(text) => {
                    self.sync_markdown((index, 0), text, cx);
                }
                Entry::ToolCall(tool_call) => {
                    for (part, text) in tool_call.text.iter().enumerate() {
                        self.sync_markdown((index, part + 1), text, cx);
                    }
                }
                Entry::Plan => {}
            }
        }
    }

    fn sync_markdown(&mut self, key: MarkdownKey, text: &str, cx: &mut Context<Self>) {
        match self.markdowns.get(&key) {
            Some(markdown) => {
                let source = markdown.read(cx).source().to_string();
                if source == text {
                    return;
                }
                markdown.update(cx, |markdown, cx| {
                    match text.strip_prefix(source.as_str()) {
                        Some(appended) => markdown.append(appended, cx),
                        None => markdown.replace(text.to_string(), cx),
                    }
                });
            }
            None => {
                let markdown = cx.new(|cx| Markdown::new(text.to_string().into(), None, None, cx));
                self.markdowns.insert(key, markdown);
            }
        }
    }

    fn markdown(&self, key: MarkdownKey, style: MarkdownStyle) -> Option<MarkdownElement> {
        self.markdowns
            .get(&key)
            .map(|markdown| MarkdownElement::new(markdown.clone(), style))
    }

    /// Agent commands matching a `/name` being typed at the start of the message.
    fn matching_commands(&self, cx: &App) -> Vec<acp::AvailableCommand> {
        let text = self.composer.read(cx).text();
        if self.command_menu_dismissed_for.as_ref() == Some(text) {
            return Vec::new();
        }
        let Some(query) = text.strip_prefix('/') else {
            return Vec::new();
        };
        if query.contains(char::is_whitespace) {
            return Vec::new();
        }
        let query = query.to_lowercase();
        self.thread
            .read(cx)
            .available_commands()
            .iter()
            .filter(|command| command.name.to_lowercase().starts_with(&query))
            .take(8)
            .cloned()
            .collect()
    }

    fn select_next_command(
        &mut self,
        _: &menu::SelectNext,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.matching_commands(cx).len();
        if count == 0 {
            cx.propagate();
            return;
        }
        self.command_menu_index = (self.command_menu_index + 1) % count;
        cx.notify();
    }

    fn select_previous_command(
        &mut self,
        _: &menu::SelectPrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let count = self.matching_commands(cx).len();
        if count == 0 {
            cx.propagate();
            return;
        }
        self.command_menu_index = self.command_menu_index.checked_sub(1).unwrap_or(count - 1);
        cx.notify();
    }

    fn accept_slash_command(
        &mut self,
        _: &AcceptSlashCommand,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let commands = self.matching_commands(cx);
        match commands.get(
            self.command_menu_index
                .min(commands.len().saturating_sub(1)),
        ) {
            Some(command) => {
                let name = command.name.clone();
                self.accept_command(&name, cx);
            }
            None => cx.propagate(),
        }
    }

    fn accept_command(&mut self, name: &str, cx: &mut Context<Self>) {
        let text = format!("/{name} ");
        self.composer
            .update(cx, |composer, cx| composer.set_text(text, cx));
        cx.notify();
    }

    fn send(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        let commands = self.matching_commands(cx);
        if let Some(command) = commands.get(
            self.command_menu_index
                .min(commands.len().saturating_sub(1)),
        ) {
            let name = command.name.clone();
            self.accept_command(&name, cx);
            return;
        }
        let text = self.composer.read(cx).text().to_string();
        if text.trim().is_empty() {
            return;
        }
        self.composer
            .update(cx, |composer, cx| composer.set_text("", cx));
        if self.thread.read(cx).is_working() || !self.queued_messages.is_empty() {
            self.queued_messages.push(text);
            cx.notify();
            return;
        }
        self.scroll_handle.scroll_to_bottom();
        self.thread.update(cx, |thread, cx| thread.send(text, cx));
    }

    fn send_next_queued_message(&mut self, cx: &mut Context<Self>) {
        let thread = self.thread.read(cx);
        if self.queued_messages.is_empty()
            || thread.is_working()
            || thread.status() != &ConnectionStatus::Ready
        {
            return;
        }
        let text = self.queued_messages.remove(0);
        self.scroll_handle.scroll_to_bottom();
        // Deferred: this runs while the thread is notifying observers.
        let thread = self.thread.clone();
        cx.defer(move |cx| thread.update(cx, |thread, cx| thread.send(text, cx)));
    }

    fn is_scrolled_to_bottom(&self) -> bool {
        let offset = self.scroll_handle.offset();
        let max_offset = self.scroll_handle.max_offset();
        -offset.y >= max_offset.y - px(40.)
    }

    fn stop(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if !self.matching_commands(cx).is_empty() {
            self.command_menu_dismissed_for = Some(self.composer.read(cx).text().clone());
            cx.notify();
            return;
        }
        self.thread.update(cx, |thread, cx| thread.cancel(cx));
    }

    fn render_command_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let commands = self.matching_commands(cx);
        if commands.is_empty() {
            return None;
        }
        let selected = self.command_menu_index.min(commands.len() - 1);
        let mut items = Vec::new();
        for (index, command) in commands.into_iter().enumerate() {
            let name = command.name.clone();
            let hint = match &command.input {
                Some(acp::AvailableCommandInput::Unstructured(input)) => Some(input.hint.clone()),
                _ => None,
            };
            items.push(
                ui::ListItem::new(("slash-command", index))
                    .inset(true)
                    .spacing(ui::ListItemSpacing::Sparse)
                    .toggle_state(index == selected)
                    .child(
                        v_flex()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(Label::new(format!("/{}", command.name)).buffer_font(cx))
                                    .when_some(hint, |this, hint| {
                                        this.child(
                                            Label::new(hint)
                                                .size(LabelSize::Small)
                                                .color(Color::Placeholder),
                                        )
                                    }),
                            )
                            .child(
                                Label::new(command.description)
                                    .size(LabelSize::Small)
                                    .color(Color::Muted)
                                    .truncate(),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.accept_command(&name, cx))),
            );
        }
        Some(
            v_flex()
                .absolute()
                .bottom_full()
                .left_0()
                .mb_1()
                .w(rems(26.))
                .p_1()
                .elevation_2(cx)
                .children(items)
                .into_any_element(),
        )
    }

    fn agent_icon(&self, cx: &App) -> Icon {
        let icon_path = self
            .agent_id
            .as_ref()
            .and_then(|agent_id| self.registry.read(cx).agent(agent_id)?.icon_path().cloned());
        match icon_path {
            Some(path) => Icon::from_external_svg(path),
            None => Icon::new(IconName::Terminal),
        }
    }

    fn agent_name(&self, cx: &App) -> SharedString {
        self.agent_id
            .as_ref()
            .and_then(|agent_id| self.registry.read(cx).agent(agent_id))
            .map(|agent| agent.name().clone())
            .unwrap_or_else(|| self.thread.read(cx).agent_name().clone())
    }

    fn tool_card_header_bg(cx: &App) -> Hsla {
        let colors = cx.theme().colors();
        colors
            .element_background
            .blend(colors.editor_foreground.opacity(0.025))
    }

    fn tool_card_border_color(cx: &App) -> Hsla {
        cx.theme().colors().border.opacity(0.8)
    }

    fn activity_bar_bg(cx: &App) -> Hsla {
        let colors = cx.theme().colors();
        colors
            .editor_background
            .blend(colors.element_selected.opacity(0.3))
    }

    fn render_toolbar(&self, cx: &App) -> impl IntoElement {
        let agent_name = self.agent_name(cx);
        h_flex()
            .h(px(36.))
            .flex_none()
            .px_2()
            .gap_1p5()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .child(
                self.agent_icon(cx)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Label::new(self.title.clone())
                    .size(LabelSize::Small)
                    .truncate(),
            )
            .child(
                Label::new(agent_name)
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
    }

    fn render_entry(
        &self,
        index: usize,
        entry: &Entry,
        is_last: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        match entry {
            Entry::UserMessage(_) => {
                let style = MarkdownStyle::themed(MarkdownFont::Agent, window, cx);
                v_flex()
                    .id(("user-message", index))
                    .pt_2()
                    .pb_3()
                    .px_2()
                    .w_full()
                    .child(
                        div()
                            .py_3()
                            .px_2()
                            .rounded_md()
                            .bg(colors.editor_background)
                            .border_1()
                            .border_color(colors.border)
                            .shadow_md()
                            .text_xs()
                            .children(self.markdown((index, 0), style)),
                    )
                    .into_any_element()
            }
            Entry::AgentMessage(_) => {
                let style = MarkdownStyle::themed(MarkdownFont::Agent, window, cx);
                let show_controls = !self.thread.read(cx).is_working()
                    && self.thread.read(cx).entries()[index + 1..]
                        .iter()
                        .all(|entry| {
                            !matches!(entry, Entry::AgentMessage(_) | Entry::UserMessage(_))
                        });
                v_flex()
                    .w_full()
                    .child(
                        v_flex()
                            .px_5()
                            .py_1p5()
                            .when(is_last && !show_controls, |this| this.pb_4())
                            .w_full()
                            .text_ui(cx)
                            .children(self.markdown((index, 0), style)),
                    )
                    .when(show_controls, |this| {
                        this.child(self.render_thread_controls(index, cx))
                    })
                    .into_any_element()
            }
            Entry::AgentThought(_) => self.render_thinking_block(index, is_last, window, cx),
            Entry::ToolCall(tool_call) => self.render_tool_call(index, tool_call, window, cx),
            // In Zed the plan lives in the activity bar above the message editor.
            Entry::Plan => div().into_any_element(),
        }
    }

    /// Zed's controls under a finished reply: copy it, jump to the prompt, jump to the top.
    fn render_thread_controls(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let user_message_index = self.thread.read(cx).entries()[..index]
            .iter()
            .rposition(|entry| matches!(entry, Entry::UserMessage(_)));
        h_flex()
            .w_full()
            .py_1p5()
            .px_4()
            .gap_1()
            .justify_end()
            .opacity(0.4)
            .hover(|this| this.opacity(1.))
            .child(
                IconButton::new(("copy-agent-response", index), IconName::Copy)
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Copy This Agent Response"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(Entry::AgentMessage(text)) =
                            this.thread.read(cx).entries().get(index)
                        {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
                        }
                    })),
            )
            .when_some(user_message_index, |this, user_message_index| {
                this.child(
                    IconButton::new(("scroll-to-user-message", index), IconName::ForwardArrowUp)
                        .icon_size(IconSize::Small)
                        .icon_color(Color::Muted)
                        .tooltip(Tooltip::text("Scroll to User Message"))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.scroll_handle.scroll_to_top_of_item(user_message_index);
                            cx.notify();
                        })),
                )
            })
            .child(
                IconButton::new(("scroll-to-top", index), IconName::ArrowUp)
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Scroll to Top"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.scroll_handle.set_offset(gpui::point(px(0.), px(0.)));
                        cx.notify();
                    })),
            )
            .into_any_element()
    }

    fn render_thinking_block(
        &self,
        index: usize,
        is_last: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // A thought that is still streaming starts open; clicking flips the default.
        let open_by_default = is_last && self.thread.read(cx).is_working();
        let is_open = open_by_default != self.toggled_thoughts.contains(&index);
        let header_group = SharedString::from(format!("thinking-header-{index}"));
        let line_height = window.line_height();
        let style = MarkdownStyle::themed(MarkdownFont::Agent, window, cx).with_muted_text(cx);

        v_flex()
            .px_5()
            .py_1p5()
            .w_full()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        h_flex()
                            .id(("thinking-block", index))
                            .group(&header_group)
                            .w_full()
                            .pr_1()
                            .justify_between()
                            .cursor_pointer()
                            .child(
                                h_flex()
                                    .h(line_height - px(2.))
                                    .gap_1p5()
                                    .child(
                                        Icon::new(IconName::ToolThink)
                                            .size(IconSize::Small)
                                            .color(Color::Muted),
                                    )
                                    .child(
                                        div()
                                            .text_size(rems_from_px(13_f32))
                                            .text_color(cx.theme().colors().text_muted)
                                            .child("Thinking"),
                                    ),
                            )
                            .child(
                                Disclosure::new(("thinking-disclosure", index), is_open)
                                    .opened_icon(IconName::ChevronUp)
                                    .closed_icon(IconName::ChevronDown)
                                    .visible_on_hover(&header_group),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if !this.toggled_thoughts.remove(&index) {
                                    this.toggled_thoughts.insert(index);
                                }
                                cx.notify();
                            })),
                    )
                    .when(is_open, |this| {
                        this.child(
                            div()
                                .ml_1p5()
                                .pl_3p5()
                                .border_l_1()
                                .border_color(Self::tool_card_border_color(cx))
                                .children(self.markdown((index, 0), style)),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_tool_call(
        &self,
        index: usize,
        tool_call: &ToolCall,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let editor_background = cx.theme().colors().editor_background;
        let failed = matches!(tool_call.status, acp::ToolCallStatus::Failed);
        let needs_confirmation = self
            .thread
            .read(cx)
            .permission_request(&tool_call.id)
            .is_some();
        let is_terminal_tool = matches!(tool_call.kind, acp::ToolKind::Execute);
        let is_edit = matches!(tool_call.kind, acp::ToolKind::Edit) || !tool_call.diffs.is_empty();
        let use_card_layout = needs_confirmation || is_edit || is_terminal_tool;
        let has_text = !tool_call.text.is_empty();
        let is_collapsible = has_text && !needs_confirmation && !is_terminal_tool;
        let is_open = needs_confirmation
            || is_terminal_tool
            || is_edit
            || self.expanded_tool_calls.contains(&tool_call.id);
        let header_group = SharedString::from(format!("tool-call-header-{index}"));

        let label = self.render_tool_call_label(tool_call, use_card_layout, window, cx);
        let header = h_flex()
            .group(&header_group)
            .relative()
            .w_full()
            .justify_between()
            .when(use_card_layout, |this| {
                this.p_0p5()
                    .rounded_t(rems_from_px(5_f32))
                    .bg(Self::tool_card_header_bg(cx))
            })
            .child(label)
            .child(
                h_flex()
                    .pr_0p5()
                    .gap_1()
                    .when(
                        matches!(tool_call.status, acp::ToolCallStatus::InProgress)
                            && use_card_layout,
                        |this| {
                            this.child(
                                Icon::new(IconName::LoadCircle)
                                    .size(IconSize::Small)
                                    .color(Color::Muted)
                                    .with_rotate_animation(2),
                            )
                        },
                    )
                    .when(is_collapsible, |this| {
                        let tool_call_id = tool_call.id.clone();
                        this.child(
                            Disclosure::new(("tool-call-disclosure", index), is_open)
                                .opened_icon(IconName::ChevronUp)
                                .closed_icon(IconName::ChevronDown)
                                .visible_on_hover(&header_group)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.expanded_tool_calls.remove(&tool_call_id) {
                                        this.expanded_tool_calls.insert(tool_call_id.clone());
                                    }
                                    cx.notify();
                                })),
                        )
                    })
                    .when(failed, |this| {
                        this.child(
                            Icon::new(IconName::Close)
                                .color(Color::Error)
                                .size(IconSize::Small),
                        )
                    }),
            );

        let output = is_open.then(|| {
            let mut parts = Vec::new();
            for diff in &tool_call.diffs {
                parts.push(render_diff(diff, cx));
            }
            for part in 0..tool_call.text.len() {
                let style = tool_output_style(is_terminal_tool, window, cx);
                if let Some(markdown) = self.markdown((index, part + 1), style) {
                    parts.push(
                        div()
                            .when(use_card_layout, |this| {
                                this.p_2()
                                    .border_t_1()
                                    .border_color(Self::tool_card_border_color(cx))
                            })
                            .when(!use_card_layout, |this| {
                                this.mt_1p5()
                                    .ml(rems(0.4))
                                    .px_3p5()
                                    .border_l_1()
                                    .border_color(Self::tool_card_border_color(cx))
                            })
                            .text_xs()
                            .child(markdown)
                            .into_any_element(),
                    );
                }
            }
            parts
        });

        let permission_buttons = self.render_permission_buttons(index, &tool_call.id, cx);

        v_flex()
            .map(|this| {
                if use_card_layout {
                    this.my_1p5()
                        .rounded_md()
                        .border_1()
                        .when(failed, |this| this.border_dashed())
                        .border_color(Self::tool_card_border_color(cx))
                        .bg(editor_background)
                        .overflow_hidden()
                } else {
                    this.my_1()
                }
            })
            .map(|this| {
                if tool_call.locations.len() == 1 && !use_card_layout {
                    this.ml_4()
                } else {
                    this.ml_5()
                }
            })
            .mr_5()
            .child(header)
            .children(output.into_iter().flatten())
            .children(permission_buttons)
            .into_any_element()
    }

    fn render_tool_call_label(
        &self,
        tool_call: &ToolCall,
        use_card_layout: bool,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let icon = Icon::new(match tool_call.kind {
            acp::ToolKind::Read => IconName::ToolSearch,
            acp::ToolKind::Edit => IconName::ToolPencil,
            acp::ToolKind::Delete => IconName::ToolDeleteFile,
            acp::ToolKind::Move => IconName::ArrowRightLeft,
            acp::ToolKind::Search => IconName::ToolSearch,
            acp::ToolKind::Execute => IconName::ToolTerminal,
            acp::ToolKind::Think => IconName::ToolThink,
            acp::ToolKind::Fetch => IconName::ToolWeb,
            acp::ToolKind::SwitchMode => IconName::ArrowRightLeft,
            _ => IconName::ToolHammer,
        })
        .size(IconSize::Small)
        .color(Color::Muted);

        let fade_bg = if use_card_layout {
            Self::tool_card_header_bg(cx)
        } else {
            colors.panel_background
        };

        if matches!(tool_call.kind, acp::ToolKind::Execute) {
            return v_flex()
                .w_full()
                .p_1p5()
                .pt_1()
                .bg(Self::tool_card_header_bg(cx))
                .child(
                    h_flex().h_6().child(
                        Label::new("Run Command")
                            .buffer_font(cx)
                            .size(LabelSize::XSmall)
                            .color(Color::Muted),
                    ),
                )
                .child(
                    div()
                        .font_buffer(cx)
                        .text_size(rems_from_px(12_f32))
                        .line_height(rems_from_px(17_f32))
                        .text_color(colors.text)
                        .child(tool_call.title.clone()),
                )
                .into_any_element();
        }

        h_flex()
            .relative()
            .w_full()
            .h(window.line_height() - px(2.))
            .text_size(rems_from_px(13_f32))
            .gap_1p5()
            .when(use_card_layout, |this| this.px_1())
            .overflow_hidden()
            .child(div().flex_none().child(icon))
            .child(
                div()
                    .w_full()
                    .whitespace_nowrap()
                    .text_color(if use_card_layout {
                        colors.text
                    } else {
                        colors.text_muted
                    })
                    .child(tool_call.title.clone()),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .right_0()
                    .w_12()
                    .h_full()
                    .bg(gpui::linear_gradient(
                        90.,
                        gpui::linear_color_stop(fade_bg, 1.),
                        gpui::linear_color_stop(fade_bg.opacity(0.2), 0.),
                    )),
            )
            .into_any_element()
    }

    fn render_permission_buttons(
        &self,
        index: usize,
        tool_call_id: &acp::ToolCallId,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let request = self.thread.read(cx).permission_request(tool_call_id)?;
        let options = request.options.clone();
        let mut buttons = Vec::new();
        for (option_index, option) in options.into_iter().enumerate() {
            let icon = match option.kind {
                acp::PermissionOptionKind::AllowOnce => Icon::new(IconName::Check)
                    .size(IconSize::XSmall)
                    .color(Color::Success),
                acp::PermissionOptionKind::AllowAlways => Icon::new(IconName::CheckDouble)
                    .size(IconSize::XSmall)
                    .color(Color::Success),
                _ => Icon::new(IconName::Close)
                    .size(IconSize::XSmall)
                    .color(Color::Error),
            };
            let tool_call_id = tool_call_id.clone();
            let option_id = option.id.clone();
            buttons.push(
                Button::new(
                    SharedString::from(format!("permission-{index}-{option_index}")),
                    option.name,
                )
                .start_icon(icon)
                .label_size(LabelSize::Small)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let option_id = option_id.clone();
                    this.thread.update(cx, |thread, cx| {
                        thread.respond_to_permission(&tool_call_id, option_id, cx)
                    });
                })),
            );
        }
        Some(
            v_flex()
                .p_1()
                .border_t_1()
                .border_color(Self::tool_card_border_color(cx))
                .w_full()
                .gap_0p5()
                .children(buttons)
                .into_any_element(),
        )
    }

    /// A permission request whose tool call never arrived as its own entry.
    fn render_orphan_permission(
        &self,
        index: usize,
        tool_call_id: &acp::ToolCallId,
        title: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let buttons = self.render_permission_buttons(index, tool_call_id, cx)?;
        let tool_call = ToolCall {
            id: tool_call_id.clone(),
            title: title.to_string(),
            kind: acp::ToolKind::Other,
            status: acp::ToolCallStatus::Pending,
            text: Vec::new(),
            diffs: Vec::new(),
            locations: Vec::new(),
        };
        Some(
            v_flex()
                .my_1p5()
                .ml_5()
                .mr_5()
                .rounded_md()
                .border_1()
                .border_color(Self::tool_card_border_color(cx))
                .bg(cx.theme().colors().editor_background)
                .overflow_hidden()
                .child(
                    div()
                        .p_0p5()
                        .bg(Self::tool_card_header_bg(cx))
                        .child(self.render_tool_call_label(&tool_call, true, window, cx)),
                )
                .child(buttons)
                .into_any_element(),
        )
    }

    fn render_generating(&self, cx: &App) -> Option<AnyElement> {
        let thread = self.thread.read(cx);
        let started_at = thread.turn_started_at()?;
        let awaiting_confirmation = thread.orphan_permission_requests().next().is_some()
            || thread.entries().iter().any(|entry| {
                matches!(entry, Entry::ToolCall(tool_call)
                    if thread.permission_request(&tool_call.id).is_some())
            });
        let elapsed = started_at.elapsed().as_secs();
        let elapsed_label = if elapsed >= 60 {
            format!("{}m {:02}s", elapsed / 60, elapsed % 60)
        } else {
            format!("{elapsed}s")
        };
        let status_label = if thread.status() == &ConnectionStatus::Connecting {
            Some(format!("Starting {}…", self.agent_name(cx)))
        } else if awaiting_confirmation {
            Some("Awaiting Confirmation".to_string())
        } else {
            None
        };

        Some(
            h_flex()
                .py_2()
                .px(rems_from_px(22_f32))
                .gap_2()
                .child(
                    h_flex()
                        .w_2()
                        .justify_center()
                        .child(if awaiting_confirmation {
                            SpinnerLabel::sand()
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                        } else {
                            SpinnerLabel::dots()
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                        }),
                )
                .children(
                    status_label
                        .map(|label| Label::new(label).size(LabelSize::Small).color(Color::Muted)),
                )
                .child(
                    Label::new(elapsed_label)
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .into_any_element(),
        )
    }

    /// Zed's notice for agents that continue a session without showing its earlier messages.
    fn render_restore_notice(&self, cx: &App) -> Option<AnyElement> {
        let (title, description) = match self.thread.read(cx).session_restore()? {
            SessionRestore::ResumedWithoutHistory => (
                "Resumed Session",
                "This agent does not support viewing previous messages. However, your session will still continue from where you last left off.",
            ),
            SessionRestore::Unavailable => (
                "New Session",
                "This agent couldn't restore the previous conversation, so this is a new session.",
            ),
            SessionRestore::New | SessionRestore::Loaded => return None,
        };
        Some(
            Callout::new()
                .border_position(ui::CalloutBorderPosition::Bottom)
                .severity(Severity::Info)
                .icon(IconName::Info)
                .title(title)
                .description(description)
                .into_any_element(),
        )
    }

    /// Zed's "Authenticate to …" callout, with a button per login method the agent offers.
    fn render_auth_required(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let thread = self.thread.read(cx);
        if thread.status() != &ConnectionStatus::AuthRequired {
            return None;
        }
        let agent_name = self.agent_name(cx);
        let methods = thread.auth_methods().to_vec();
        let auth_error = thread.auth_error().cloned();
        let has_terminal_method = methods
            .iter()
            .any(|method| matches!(method, acp::AuthMethod::Terminal(_)));

        let mut buttons = Vec::new();
        for (index, method) in methods.iter().enumerate().rev() {
            let (method_id, name, description, is_terminal) = match method {
                acp::AuthMethod::Agent(method) => (
                    method.id.clone(),
                    method.name.clone(),
                    method.description.clone(),
                    false,
                ),
                acp::AuthMethod::Terminal(method) => (
                    method.id.clone(),
                    method.name.clone(),
                    method.description.clone(),
                    true,
                ),
                _ => continue,
            };
            buttons.push(
                Button::new(SharedString::from(format!("auth-{}", method_id.0)), name)
                    .label_size(LabelSize::Small)
                    .style(if index == 0 {
                        ButtonStyle::Tinted(ui::TintColor::Accent)
                    } else {
                        ButtonStyle::Outlined
                    })
                    .when_some(description, |button, description| {
                        button.tooltip(Tooltip::text(description))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if is_terminal {
                            let command = this.thread.read(cx).terminal_auth_command(&method_id);
                            let cwd = this.thread.read(cx).cwd().clone();
                            if let Some(command) = command {
                                cx.background_spawn(async move {
                                    if let Err(error) = open_in_terminal(&command, &cwd).await {
                                        log::error!(
                                            "couldn't open a terminal to log in: {error:#}"
                                        );
                                    }
                                })
                                .detach();
                            }
                        } else {
                            let method_id = method_id.clone();
                            this.thread
                                .update(cx, |thread, cx| thread.authenticate(method_id, cx));
                        }
                    })),
            );
        }
        if has_terminal_method {
            buttons.push(
                Button::new("auth-retry", "I've Logged In")
                    .label_size(LabelSize::Small)
                    .style(ButtonStyle::Outlined)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.thread
                            .update(cx, |thread, cx| thread.retry_session(cx));
                    })),
            );
        }

        let description = auth_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| {
                if methods.len() > 1 {
                    "Choose one of the following authentication options:".to_string()
                } else {
                    format!("{agent_name} needs you to log in before it can start.")
                }
            });
        Some(
            div()
                .px_2()
                .pb_2()
                .child(
                    Callout::new()
                        .icon(IconName::Info)
                        .title(format!("Authenticate to {agent_name}"))
                        .description(description)
                        .actions_slot(h_flex().justify_end().flex_wrap().gap_1().children(buttons)),
                )
                .into_any_element(),
        )
    }

    fn render_errors(&self, cx: &App) -> Option<AnyElement> {
        let thread = self.thread.read(cx);
        let callout = if let ConnectionStatus::Failed(error) = thread.status() {
            Callout::new()
                .severity(Severity::Error)
                .icon(IconName::XCircle)
                .title(format!("{} couldn't start", self.agent_name(cx)))
                .description(error.clone())
        } else if let Some(error) = thread.turn_error() {
            Callout::new()
                .severity(Severity::Error)
                .icon(IconName::XCircle)
                .title("The agent stopped with an error")
                .description(error.clone())
        } else {
            return None;
        };
        Some(div().px_2().pb_2().child(callout).into_any_element())
    }

    fn render_plan_section(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let plan = self.thread.read(cx).plan().to_vec();
        if plan.is_empty() {
            return None;
        }
        let colors = cx.theme().colors();
        let completed = plan
            .iter()
            .filter(|item| item.status == acp::PlanEntryStatus::Completed)
            .count();
        let pending = plan
            .iter()
            .filter(|item| item.status == acp::PlanEntryStatus::Pending)
            .count();
        let in_progress = plan
            .iter()
            .find(|item| item.status == acp::PlanEntryStatus::InProgress)
            .cloned();
        let activity_bg = Self::activity_bar_bg(cx);
        let plan_expanded = self.plan_expanded;

        let title = match in_progress.filter(|_| !plan_expanded) {
            Some(item) => h_flex()
                .relative()
                .w_full()
                .gap_1()
                .overflow_hidden()
                .child(
                    Label::new("Current:")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .whitespace_nowrap()
                        .child(item.content),
                )
                .when(pending > 0, |this| {
                    this.child(
                        h_flex()
                            .absolute()
                            .top_0()
                            .right_0()
                            .h_full()
                            .child(div().min_w_8().h_full().bg(gpui::linear_gradient(
                                90.,
                                gpui::linear_color_stop(activity_bg, 1.),
                                gpui::linear_color_stop(activity_bg.opacity(0.2), 0.),
                            )))
                            .child(
                                div().pr_0p5().bg(activity_bg).child(
                                    Label::new(format!("{pending} left"))
                                        .size(LabelSize::Small)
                                        .color(Color::Muted),
                                ),
                            ),
                    )
                }),
            None => h_flex()
                .w_full()
                .gap_1()
                .justify_between()
                .child(
                    Label::new("Plan")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    Label::new(if completed == plan.len() {
                        "All Done".to_string()
                    } else {
                        format!("{completed}/{} Tasks", plan.len())
                    })
                    .size(LabelSize::Small)
                    .color(Color::Muted)
                    .mr_1(),
                ),
        };

        let summary = h_flex()
            .id("plan-summary")
            .p_1()
            .w_full()
            .gap_1()
            .cursor_pointer()
            .when(plan_expanded, |this| {
                this.border_b_1().border_color(colors.border)
            })
            .child(Disclosure::new("plan-disclosure", plan_expanded))
            .child(title.flex_1())
            .child(
                IconButton::new("dismiss-plan", IconName::Close)
                    .icon_size(IconSize::XSmall)
                    .shape(ui::IconButtonShape::Square)
                    .tooltip(Tooltip::text("Clear Plan"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.thread.update(cx, |thread, cx| thread.clear_plan(cx));
                        cx.stop_propagation();
                    })),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.plan_expanded = !this.plan_expanded;
                cx.notify();
            }));

        Some(
            v_flex()
                .child(summary)
                .when(plan_expanded, |this| {
                    this.child(render_plan_entries(&plan, window, cx))
                })
                .into_any_element(),
        )
    }

    /// Files the agent changed in this thread, with line counts, like Zed's edits summary.
    fn render_edits_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let mut files: Vec<(String, usize, usize)> = Vec::new();
        for entry in self.thread.read(cx).entries() {
            let Entry::ToolCall(tool_call) = entry else {
                continue;
            };
            for diff in &tool_call.diffs {
                let path = diff.path.to_string_lossy().into_owned();
                let (added, removed) = diff.line_counts();
                match files.iter_mut().find(|(existing, _, _)| *existing == path) {
                    Some(file) => {
                        file.1 += added;
                        file.2 += removed;
                    }
                    None => files.push((path, added, removed)),
                }
            }
        }
        if files.is_empty() {
            return None;
        }
        let cwd = self.thread.read(cx).cwd().clone();
        let colors = cx.theme().colors();
        let total_added: usize = files.iter().map(|(_, added, _)| added).sum();
        let total_removed: usize = files.iter().map(|(_, _, removed)| removed).sum();
        let expanded = self.edits_expanded;
        let file_count = files.len();

        let summary = h_flex()
            .id("edits-summary")
            .p_1()
            .w_full()
            .gap_1()
            .cursor_pointer()
            .when(expanded, |this| {
                this.border_b_1().border_color(colors.border)
            })
            .child(Disclosure::new("edits-disclosure", expanded))
            .child(
                Label::new("Edits")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .child(
                Label::new(if file_count == 1 {
                    "1 file".to_string()
                } else {
                    format!("{file_count} files")
                })
                .size(LabelSize::Small)
                .color(Color::Muted),
            )
            .child(div().flex_1())
            .child(diff_stat(total_added, total_removed))
            .on_click(cx.listener(|this, _, _, cx| {
                this.edits_expanded = !this.edits_expanded;
                cx.notify();
            }));

        Some(
            v_flex()
                .child(summary)
                .when(expanded, |this| {
                    this.child(
                        v_flex()
                            .id("edited-files")
                            .max_h_40()
                            .overflow_y_scroll()
                            .children(files.into_iter().enumerate().map(
                                |(index, (path, added, removed))| {
                                    let display_path = std::path::Path::new(&path)
                                        .strip_prefix(&cwd)
                                        .map(|relative| relative.to_string_lossy().into_owned())
                                        .unwrap_or(path);
                                    h_flex()
                                        .py_1()
                                        .px_2()
                                        .gap_2()
                                        .bg(colors.editor_background)
                                        .when(index + 1 < file_count, |this| {
                                            this.border_b_1().border_color(colors.border)
                                        })
                                        .child(
                                            Icon::new(IconName::File)
                                                .size(IconSize::Small)
                                                .color(Color::Muted),
                                        )
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w_0()
                                                .text_xs()
                                                .text_color(colors.text_muted)
                                                .child(display_path),
                                        )
                                        .child(diff_stat(added, removed))
                                },
                            )),
                    )
                })
                .into_any_element(),
        )
    }

    fn render_queue_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.queued_messages.is_empty() {
            return None;
        }
        let colors = cx.theme().colors();
        let count = self.queued_messages.len();
        let expanded = self.queue_expanded;
        let summary = h_flex()
            .id("queue-summary")
            .p_1()
            .w_full()
            .gap_1()
            .cursor_pointer()
            .when(expanded, |this| {
                this.border_b_1().border_color(colors.border)
            })
            .child(Disclosure::new("queue-disclosure", expanded))
            .child(
                Label::new(if count == 1 {
                    "1 Queued Message".to_string()
                } else {
                    format!("{count} Queued Messages")
                })
                .size(LabelSize::Small)
                .color(Color::Muted),
            )
            .child(div().flex_1())
            .child(
                Button::new("send-queued-now", "Send Now")
                    .label_size(LabelSize::Small)
                    .tooltip(Tooltip::text(
                        "Stop the current turn and send the next queued message",
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.thread.update(cx, |thread, cx| thread.cancel(cx));
                        cx.stop_propagation();
                    })),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.queue_expanded = !this.queue_expanded;
                cx.notify();
            }));
        let mut rows = Vec::new();
        for (index, message) in self.queued_messages.iter().enumerate() {
            rows.push(
                h_flex()
                    .py_1()
                    .px_2()
                    .gap_2()
                    .bg(colors.editor_background)
                    .when(index + 1 < count, |this| {
                        this.border_b_1().border_color(colors.border)
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(colors.text_muted)
                            .child(message.lines().next().unwrap_or_default().to_string()),
                    )
                    .child(
                        IconButton::new(("remove-queued", index), IconName::Close)
                            .icon_size(IconSize::XSmall)
                            .tooltip(Tooltip::text("Remove From Queue"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if index < this.queued_messages.len() {
                                    this.queued_messages.remove(index);
                                }
                                cx.notify();
                            })),
                    ),
            );
        }
        Some(
            v_flex()
                .child(summary)
                .when(expanded, |this| this.children(rows))
                .into_any_element(),
        )
    }

    /// The bar above the message editor: plan, edited files and queued messages.
    fn render_activity_bar(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let sections: Vec<AnyElement> = [
            self.render_plan_section(window, cx),
            self.render_edits_section(cx),
            self.render_queue_section(cx),
        ]
        .into_iter()
        .flatten()
        .collect();
        if sections.is_empty() {
            return None;
        }
        let colors = cx.theme().colors();
        let section_count = sections.len();
        let mut children = Vec::new();
        for (index, section) in sections.into_iter().enumerate() {
            children.push(section);
            if index + 1 < section_count {
                children.push(ui::Divider::horizontal().into_any_element());
            }
        }
        Some(
            h_flex()
                .w_full()
                .px_2()
                .justify_center()
                .child(
                    v_flex()
                        .w_full()
                        .max_w(MAX_CONTENT_WIDTH)
                        .bg(Self::activity_bar_bg(cx))
                        .border_1()
                        .border_b_0()
                        .border_color(colors.border)
                        .rounded_t_md()
                        .shadow(vec![
                            gpui::BoxShadow::new(px(1.), px(-1.), gpui::black().opacity(0.12))
                                .blur_radius(px(2.)),
                        ])
                        .children(children),
                )
                .into_any_element(),
        )
    }

    /// The agent's session settings (model, effort, mode, …) as Zed shows them: a muted
    /// dropdown button per choice and a switch per on/off setting.
    fn render_session_settings(&self, cx: &App) -> Vec<AnyElement> {
        let thread = self.thread.read(cx);
        let mut controls = Vec::new();
        for option in thread.config_options() {
            let config_id = option.id.clone();
            let element_id = SharedString::from(format!("config-option-{}", option.id.0));
            let tooltip_title: SharedString = option.name.clone().into();
            let tooltip_description = option.description.clone().map(SharedString::from);
            match &option.kind {
                acp::SessionConfigKind::Select(select) => {
                    let mut choices: Vec<(
                        Option<SharedString>,
                        acp::SessionConfigValueId,
                        SharedString,
                    )> = Vec::new();
                    match &select.options {
                        acp::SessionConfigSelectOptions::Ungrouped(options) => {
                            for choice in options {
                                choices.push((
                                    None,
                                    choice.value.clone(),
                                    choice.name.clone().into(),
                                ));
                            }
                        }
                        acp::SessionConfigSelectOptions::Grouped(groups) => {
                            for group in groups {
                                let group_name: SharedString = group.name.clone().into();
                                for choice in &group.options {
                                    choices.push((
                                        Some(group_name.clone()),
                                        choice.value.clone(),
                                        choice.name.clone().into(),
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                    let current_value = select.current_value.clone();
                    let current_name = choices
                        .iter()
                        .find(|(_, value, _)| *value == current_value)
                        .map(|(_, _, name)| name.clone())
                        .unwrap_or_else(|| current_value.0.to_string().into());
                    let thread = self.thread.clone();
                    controls.push(
                        PopoverMenu::new(element_id.clone())
                            .menu(move |window, cx| {
                                let choices = choices.clone();
                                let current_value = current_value.clone();
                                let thread = thread.clone();
                                let config_id = config_id.clone();
                                Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                                    let mut current_group: Option<SharedString> = None;
                                    for (group, value, name) in choices {
                                        if group.is_some() && group != current_group {
                                            if current_group.is_some() {
                                                menu = menu.separator();
                                            }
                                            menu = menu.header(group.clone().unwrap_or_default());
                                            current_group = group;
                                        }
                                        let thread = thread.clone();
                                        let config_id = config_id.clone();
                                        let is_current = value == current_value;
                                        menu = menu.toggleable_entry(
                                            name,
                                            is_current,
                                            IconPosition::End,
                                            None,
                                            move |_, cx| {
                                                let value = value.clone();
                                                thread.update(cx, |thread, cx| {
                                                    thread.set_config_option(
                                                        config_id.clone(),
                                                        acp::SessionConfigOptionValue::value_id(
                                                            value,
                                                        ),
                                                        cx,
                                                    )
                                                });
                                            },
                                        );
                                    }
                                    menu
                                }))
                            })
                            .trigger_with_tooltip(
                                Button::new(
                                    SharedString::from(format!("{element_id}-trigger")),
                                    current_name,
                                )
                                .label_size(LabelSize::Small)
                                .color(Color::Muted)
                                .end_icon(
                                    Icon::new(IconName::ChevronDown)
                                        .size(IconSize::XSmall)
                                        .color(Color::Muted),
                                ),
                                setting_tooltip(tooltip_title, tooltip_description),
                            )
                            .anchor(gpui::Anchor::BottomRight)
                            .into_any_element(),
                    );
                }
                acp::SessionConfigKind::Boolean(boolean) => {
                    let thread = self.thread.clone();
                    controls.push(
                        h_flex()
                            .id(element_id.clone())
                            .pr_1()
                            .tooltip(setting_tooltip(tooltip_title.clone(), tooltip_description))
                            .child(
                                Switch::new(
                                    SharedString::from(format!("{element_id}-switch")),
                                    if boolean.current_value {
                                        ToggleState::Selected
                                    } else {
                                        ToggleState::Unselected
                                    },
                                )
                                .label(tooltip_title)
                                .label_position(ui::SwitchLabelPosition::Start)
                                .label_size(LabelSize::Small)
                                .label_color(Color::Muted)
                                .on_click(move |state, _, cx| {
                                    let enabled = matches!(state, ToggleState::Selected);
                                    thread.update(cx, |thread, cx| {
                                        thread.set_config_option(
                                            config_id.clone(),
                                            acp::SessionConfigOptionValue::boolean(enabled),
                                            cx,
                                        )
                                    });
                                }),
                            )
                            .into_any_element(),
                    );
                }
                _ => {}
            }
        }

        // Agents that predate config options report modes separately.
        let has_mode_option = thread.config_options().iter().any(|option| {
            matches!(
                option.category,
                Some(acp::SessionConfigOptionCategory::Mode)
            )
        });
        if let Some(modes) = thread
            .modes()
            .filter(|modes| !has_mode_option && modes.available_modes.len() > 1)
        {
            let current_mode = modes.current_mode_id.clone();
            let current_name: SharedString = modes
                .available_modes
                .iter()
                .find(|mode| mode.id == current_mode)
                .map(|mode| mode.name.clone())
                .unwrap_or_else(|| current_mode.0.to_string())
                .into();
            let available: Vec<(acp::SessionModeId, SharedString)> = modes
                .available_modes
                .iter()
                .map(|mode| (mode.id.clone(), mode.name.clone().into()))
                .collect();
            let thread = self.thread.clone();
            controls.insert(
                0,
                PopoverMenu::new("session-mode")
                    .menu(move |window, cx| {
                        let available = available.clone();
                        let current_mode = current_mode.clone();
                        let thread = thread.clone();
                        Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                            for (mode_id, name) in available {
                                let thread = thread.clone();
                                let is_current = mode_id == current_mode;
                                menu = menu.toggleable_entry(
                                    name,
                                    is_current,
                                    IconPosition::End,
                                    None,
                                    move |_, cx| {
                                        let mode_id = mode_id.clone();
                                        thread
                                            .update(cx, |thread, cx| thread.set_mode(mode_id, cx));
                                    },
                                );
                            }
                            menu
                        }))
                    })
                    .trigger_with_tooltip(
                        Button::new("session-mode-trigger", current_name)
                            .label_size(LabelSize::Small)
                            .color(Color::Muted)
                            .end_icon(
                                Icon::new(IconName::ChevronDown)
                                    .size(IconSize::XSmall)
                                    .color(Color::Muted),
                            ),
                        Tooltip::text("Mode"),
                    )
                    .anchor(gpui::Anchor::BottomRight)
                    .into_any_element(),
            );
        }
        controls
    }

    /// Zed's context ring: how full the agent's context window is, with details on hover.
    fn render_context_usage(&self, cx: &App) -> Option<AnyElement> {
        let thread = self.thread.read(cx);
        let usage = thread.context_usage()?;
        let ratio = if usage.size > 0 {
            usage.used as f32 / usage.size as f32
        } else {
            0.
        };
        let percentage = SharedString::from(format!("{}%", (ratio * 100.).round() as u32));
        let used = SharedString::from(humanize_token_count(usage.used));
        let size = SharedString::from(humanize_token_count(usage.size));
        let cost_label: Option<SharedString> = thread.cost().map(|cost| {
            let precision = if cost.amount > 0. && cost.amount < 0.01 {
                4
            } else {
                2
            };
            format!("{:.precision$} {}", cost.amount, cost.currency).into()
        });
        let progress_color = if ratio >= 0.85 {
            cx.theme().status().warning
        } else {
            cx.theme().colors().text_muted
        };
        Some(
            h_flex()
                .id("context-usage")
                .mt_px()
                .mr_1()
                .child(
                    ui::CircularProgress::new(usage.used as f32, usage.size as f32, px(16.), cx)
                        .stroke_width(px(2.))
                        .progress_color(progress_color),
                )
                .tooltip(move |window, cx| {
                    let percentage = percentage.clone();
                    let used = used.clone();
                    let size = size.clone();
                    let cost_label = cost_label.clone();
                    Tooltip::element(move |_, cx| {
                        let separator =
                            Color::Custom(cx.theme().colors().text_disabled.opacity(0.6));
                        v_flex()
                            .min_w_40()
                            .child(
                                Label::new("Context")
                                    .color(Color::Muted)
                                    .size(LabelSize::Small),
                            )
                            .child(
                                h_flex()
                                    .gap_0p5()
                                    .child(Label::new(percentage.clone()))
                                    .child(Label::new("\u{2022}").color(separator).mx_1())
                                    .child(Label::new(used.clone()))
                                    .child(Label::new("/").color(separator))
                                    .child(Label::new(size.clone()).color(Color::Muted)),
                            )
                            .when_some(cost_label.clone(), |this, cost_label| {
                                this.child(
                                    v_flex()
                                        .mt_1p5()
                                        .pt_1p5()
                                        .gap_0p5()
                                        .border_t_1()
                                        .border_color(cx.theme().colors().border_variant)
                                        .child(
                                            Label::new("Cost")
                                                .color(Color::Muted)
                                                .size(LabelSize::Small),
                                        )
                                        .child(Label::new(cost_label)),
                                )
                            })
                            .into_any_element()
                    })(window, cx)
                })
                .into_any_element(),
        )
    }

    fn render_message_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let thread = self.thread.read(cx);
        let agent_name = self.agent_name(cx);
        let is_generating = thread.is_working();
        let is_editor_empty = self.composer.read(cx).text().trim().is_empty();
        let has_failed = matches!(thread.status(), ConnectionStatus::Failed(_));

        let send_button = if is_generating && is_editor_empty {
            IconButton::new("stop-generation", IconName::Stop)
                .icon_color(Color::Error)
                .style(ButtonStyle::Tinted(ui::TintColor::Error))
                .tooltip(Tooltip::text("Stop Generation"))
                .on_click(|_, window, cx| window.dispatch_action(Box::new(menu::Cancel), cx))
                .into_any_element()
        } else {
            IconButton::new(
                "send-message",
                if is_generating {
                    IconName::QueueMessage
                } else {
                    IconName::Send
                },
            )
            .style(ButtonStyle::Filled)
            .map(|this| {
                if is_editor_empty || has_failed {
                    this.disabled(true).icon_color(Color::Muted)
                } else {
                    this.icon_color(Color::Accent)
                }
            })
            .tooltip(Tooltip::text(if is_editor_empty {
                "Type to Send"
            } else if is_generating {
                "Queue and Send"
            } else {
                "Send Message"
            }))
            .on_click(|_, window, cx| window.dispatch_action(Box::new(menu::Confirm), cx))
            .into_any_element()
        };

        h_flex()
            .key_context(KEY_CONTEXT)
            .on_action(cx.listener(Self::send))
            .on_action(cx.listener(Self::stop))
            .on_action(cx.listener(Self::select_next_command))
            .on_action(cx.listener(Self::accept_slash_command))
            .on_action(cx.listener(Self::select_previous_command))
            .py_2()
            .bg(colors.editor_background)
            .justify_center()
            .border_t_1()
            .border_color(colors.border)
            .child(
                v_flex()
                    .w_full()
                    .max_w(MAX_CONTENT_WIDTH)
                    .min_w_0()
                    .px_2()
                    .gap_2()
                    .child(
                        v_flex()
                            .relative()
                            .w_full()
                            .pt_1()
                            .pr_2p5()
                            .text_ui(cx)
                            .child(self.composer.clone())
                            .children(self.render_command_menu(cx)),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_1()
                                    .px_1()
                                    .child(
                                        self.agent_icon(cx)
                                            .size(IconSize::XSmall)
                                            .color(Color::Muted),
                                    )
                                    .child(
                                        Label::new(agent_name)
                                            .size(LabelSize::Small)
                                            .color(Color::Muted),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .min_w_0()
                                    .flex_wrap()
                                    .gap_1()
                                    .children(self.render_context_usage(cx))
                                    .children(self.render_session_settings(cx))
                                    .child(send_button),
                            ),
                    ),
            )
    }
}

fn render_plan_entries(plan: &[PlanItem], _window: &Window, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let entry_bg = colors.editor_background;
    let count = plan.len();
    v_flex()
        .id("plan-entries")
        .max_h_40()
        .overflow_y_scroll()
        .children(plan.iter().enumerate().map(|(index, item)| {
            let (icon_name, icon_color) = match item.status {
                acp::PlanEntryStatus::InProgress => (IconName::TodoProgress, Color::Accent),
                acp::PlanEntryStatus::Completed => (IconName::TodoComplete, Color::Success),
                _ => (IconName::TodoPending, Color::Muted),
            };
            let icon = Icon::new(icon_name).size(IconSize::Small).color(icon_color);
            let icon = if item.status == acp::PlanEntryStatus::InProgress {
                icon.with_rotate_animation(2).into_any_element()
            } else {
                icon.into_any_element()
            };
            h_flex()
                .py_1()
                .px_2()
                .gap_2()
                .bg(entry_bg)
                .when(index + 1 < count, |this| {
                    this.border_b_1().border_color(colors.border)
                })
                .child(
                    h_flex()
                        .gap_1p5()
                        .min_w_0()
                        .text_xs()
                        .text_color(colors.text_muted)
                        .child(icon)
                        .child(item.content.clone()),
                )
        }))
        .into_any_element()
}

fn render_diff(diff: &FileDiff, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let status = cx.theme().status();
    let (removed, added) = diff.changed_lines();
    let mut lines: Vec<(bool, &str)> = removed.iter().map(|line| (false, *line)).collect();
    lines.extend(added.iter().map(|line| (true, *line)));
    let hidden = lines.len().saturating_sub(DIFF_PREVIEW_LINES);
    lines.truncate(DIFF_PREVIEW_LINES);

    v_flex()
        .w_full()
        .border_t_1()
        .border_color(colors.border.opacity(0.8))
        .font_buffer(cx)
        .text_size(rems_from_px(12_f32))
        .line_height(rems_from_px(18_f32))
        .children(lines.into_iter().map(|(is_added, line)| {
            let background = if is_added {
                status.created_background
            } else {
                status.deleted_background
            };
            h_flex()
                .px_2()
                .bg(background)
                .child(
                    div()
                        .w(px(14.))
                        .text_color(colors.text_muted)
                        .child(if is_added { "+" } else { "-" }),
                )
                .child(line.to_string())
        }))
        .when(hidden > 0, |this| {
            this.child(
                div()
                    .px_2()
                    .py_1()
                    .text_color(colors.text_muted)
                    .child(format!("… {hidden} more lines")),
            )
        })
        .into_any_element()
}

impl Focusable for AgentView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.composer.focus_handle(cx)
    }
}

impl Render for AgentView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel_background = cx.theme().colors().panel_background;
        let entries: Vec<Entry> = self.thread.read(cx).entries().to_vec();
        let entry_count = entries.len();
        let mut rows = Vec::with_capacity(entry_count + 2);
        for (index, entry) in entries.iter().enumerate() {
            rows.push(self.render_entry(index, entry, index + 1 == entry_count, window, cx));
        }
        let orphans: Vec<(acp::ToolCallId, String)> = self
            .thread
            .read(cx)
            .orphan_permission_requests()
            .map(|request| (request.tool_call_id.clone(), request.title.clone()))
            .collect();
        for (offset, (tool_call_id, title)) in orphans.iter().enumerate() {
            if let Some(element) =
                self.render_orphan_permission(entry_count + offset, tool_call_id, title, window, cx)
            {
                rows.push(element);
            }
        }
        if let Some(generating) = self.render_generating(cx) {
            rows.push(generating);
        }
        let has_rows = !rows.is_empty();

        v_flex()
            .size_full()
            .bg(panel_background)
            .child(self.render_toolbar(cx))
            .children(self.render_restore_notice(cx))
            .child(
                // Each row is a direct child of the scrolled element, so rows can be scrolled to
                // by index (entries come first, in order).
                v_flex()
                    .id("agent-conversation")
                    .flex_1()
                    .min_h_0()
                    .pt_2()
                    .pb_4()
                    .items_center()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .children(
                        rows.into_iter()
                            .map(|row| div().w_full().max_w(MAX_CONTENT_WIDTH).child(row)),
                    )
                    .when(!has_rows, |this| {
                        this.child(
                            div().w_full().max_w(MAX_CONTENT_WIDTH).px_5().py_8().child(
                                Label::new(format!(
                                    "Start a conversation with {}.",
                                    self.agent_name(cx)
                                ))
                                .color(Color::Muted),
                            ),
                        )
                    }),
            )
            .children(self.render_auth_required(cx))
            .children(self.render_errors(cx))
            .children(self.render_activity_bar(window, cx))
            .child(self.render_message_editor(cx))
    }
}

/// Tool output is secondary to the conversation, so it uses the small buffer-font sizing Zed
/// uses for command cards.
fn tool_output_style(is_terminal_tool: bool, window: &Window, cx: &App) -> MarkdownStyle {
    let mut style = MarkdownStyle::themed(MarkdownFont::Agent, window, cx);
    if is_terminal_tool {
        style = style.with_agent_buffer_font(cx);
    }
    style.base_text_style.font_size = rems_from_px(12_f32).into();
    style.base_text_style.line_height = rems_from_px(17_f32).into();
    style.code_block.text.font_size = Some(rems_from_px(12_f32).into());
    style.code_block.text.line_height = Some(rems_from_px(17_f32).into());
    style.code_block.margin.top = Some(gpui::Length::Definite(px(0.).into()));
    style.code_block.margin.bottom = Some(gpui::Length::Definite(px(0.).into()));
    style
}

fn setting_tooltip(
    title: SharedString,
    description: Option<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
    Tooltip::element(move |_, _| {
        v_flex()
            .gap_1()
            .child(Label::new(title.clone()))
            .when_some(description.clone(), |content, description| {
                content.child(
                    Label::new(description)
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
            })
            .into_any_element()
    })
}

/// Like Zed: `950`, `1.2k`, `45k`, `1.5M`.
fn humanize_token_count(count: u64) -> String {
    match count {
        0..=999 => count.to_string(),
        1_000..=9_999 => {
            let thousands = count / 1_000;
            let hundreds = (count % 1_000 + 50) / 100;
            match hundreds {
                0 => format!("{thousands}k"),
                10 => format!("{}k", thousands + 1),
                _ => format!("{thousands}.{hundreds}k"),
            }
        }
        10_000..=999_999 => format!("{}k", (count + 500) / 1_000),
        _ => {
            let millions = count / 1_000_000;
            let hundred_thousands = (count % 1_000_000 + 50_000) / 100_000;
            match hundred_thousands {
                0 => format!("{millions}M"),
                10 => format!("{}M", millions + 1),
                _ => format!("{millions}.{hundred_thousands}M"),
            }
        }
    }
}

fn diff_stat(added: usize, removed: usize) -> impl IntoElement {
    h_flex()
        .gap_1()
        .child(
            Label::new(format!("+{added}"))
                .size(LabelSize::Small)
                .color(Color::Created),
        )
        .child(
            Label::new(format!("−{removed}"))
                .size(LabelSize::Small)
                .color(Color::Deleted),
        )
}

/// Opens the system terminal running `command` in `cwd`, for agents that log in interactively.
async fn open_in_terminal(
    command: &registry::AgentCommand,
    cwd: &std::path::Path,
) -> anyhow::Result<()> {
    fn shell_quote(text: &str) -> String {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
    let mut script = format!("cd {}", shell_quote(&cwd.to_string_lossy()));
    script.push_str(" && env");
    for (key, value) in &command.env {
        script.push_str(&format!(" {}={}", key, shell_quote(value)));
    }
    script.push(' ');
    script.push_str(&shell_quote(&command.path.to_string_lossy()));
    for argument in &command.args {
        script.push(' ');
        script.push_str(&shell_quote(argument));
    }

    #[cfg(target_os = "macos")]
    {
        let apple_script_string = script.replace('\\', "\\\\").replace('"', "\\\"");
        let status = smol::process::Command::new("osascript")
            .args([
                "-e",
                "tell application \"Terminal\" to activate",
                "-e",
                &format!("tell application \"Terminal\" to do script \"{apple_script_string}\""),
            ])
            .status()
            .await?;
        anyhow::ensure!(status.success(), "osascript exited with {status}");
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let status = smol::process::Command::new("x-terminal-emulator")
            .args(["-e", "sh", "-c", &script])
            .status()
            .await?;
        anyhow::ensure!(status.success(), "the terminal exited with {status}");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::humanize_token_count;

    #[test]
    fn token_counts() {
        assert_eq!(humanize_token_count(950), "950");
        assert_eq!(humanize_token_count(1_234), "1.2k");
        assert_eq!(humanize_token_count(45_400), "45k");
        assert_eq!(humanize_token_count(200_000), "200k");
        assert_eq!(humanize_token_count(1_500_000), "1.5M");
    }
}
