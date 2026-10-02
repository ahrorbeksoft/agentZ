//! The conversation with one agent. Layout, spacing and colors follow Zed's agent thread view
//! (`agent_ui::conversation_view::thread_view`).

use std::time::Duration;

use agent_client_protocol::schema::v1 as acp;
use agent_thread::{AgentThread, ConnectionStatus, Entry, FileDiff, PlanItem, ToolCall};
use collections::{HashMap, HashSet};
use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, Hsla, KeyBinding, ScrollHandle,
    Subscription, Task, Window,
};
use markdown::{Markdown, MarkdownElement, MarkdownFont, MarkdownStyle};
use registry::{AgentId, AgentRegistryStore};
use text_input::TextInput;
use ui::{
    Callout, CommonAnimationExt as _, Disclosure, Severity, SpinnerLabel, Tooltip, prelude::*,
};

const KEY_CONTEXT: &str = "AgentComposer";
/// Matches Zed's default `agent.max_content_width`.
const MAX_CONTENT_WIDTH: Pixels = px(850.);
const DIFF_PREVIEW_LINES: usize = 12;

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
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
                this.sync_markdowns(cx);
                this.scroll_handle.scroll_to_bottom();
                cx.notify();
            }),
            // The agent's display name and icon come from the registry, which may load later.
            cx.observe(&registry, |_, _, cx| cx.notify()),
        ];
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

    fn send(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        let text = self.composer.read(cx).text().to_string();
        if text.trim().is_empty() || self.thread.read(cx).is_working() {
            return;
        }
        self.composer
            .update(cx, |composer, cx| composer.set_text("", cx));
        self.thread.update(cx, |thread, cx| thread.send(text, cx));
    }

    fn stop(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        self.thread.update(cx, |thread, cx| thread.cancel(cx));
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
                v_flex()
                    .px_5()
                    .py_1p5()
                    .when(is_last, |this| this.pb_4())
                    .w_full()
                    .text_ui(cx)
                    .children(self.markdown((index, 0), style))
                    .into_any_element()
            }
            Entry::AgentThought(_) => self.render_thinking_block(index, is_last, window, cx),
            Entry::ToolCall(tool_call) => self.render_tool_call(index, tool_call, window, cx),
            // In Zed the plan lives in the activity bar above the message editor.
            Entry::Plan => div().into_any_element(),
        }
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

    fn render_activity_bar(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
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
            .on_click(cx.listener(|this, _, _, cx| {
                this.plan_expanded = !this.plan_expanded;
                cx.notify();
            }));

        Some(
            h_flex()
                .w_full()
                .px_2()
                .justify_center()
                .child(
                    v_flex()
                        .w_full()
                        .max_w(MAX_CONTENT_WIDTH)
                        .bg(activity_bg)
                        .border_1()
                        .border_b_0()
                        .border_color(colors.border)
                        .rounded_t_md()
                        .shadow(vec![
                            gpui::BoxShadow::new(px(1.), px(-1.), gpui::black().opacity(0.12))
                                .blur_radius(px(2.)),
                        ])
                        .child(summary)
                        .when(plan_expanded, |this| {
                            this.child(render_plan_entries(&plan, window, cx))
                        }),
                )
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
            IconButton::new("send-message", IconName::Send)
                .style(ButtonStyle::Filled)
                .map(|this| {
                    if is_editor_empty || is_generating || has_failed {
                        this.disabled(true).icon_color(Color::Muted)
                    } else {
                        this.icon_color(Color::Accent)
                    }
                })
                .tooltip(Tooltip::text(if is_editor_empty {
                    "Type to Send"
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
                            .w_full()
                            .pt_1()
                            .pr_2p5()
                            .text_ui(cx)
                            .child(self.composer.clone()),
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
                            .child(send_button),
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
            .child(
                div()
                    .id("agent-conversation")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(
                        h_flex().w_full().justify_center().child(
                            v_flex()
                                .w_full()
                                .max_w(MAX_CONTENT_WIDTH)
                                .pt_2()
                                .pb_4()
                                .children(rows)
                                .when(!has_rows, |this| {
                                    this.child(
                                        div().px_5().py_8().child(
                                            Label::new(format!(
                                                "Start a conversation with {}.",
                                                self.agent_name(cx)
                                            ))
                                            .color(Color::Muted),
                                        ),
                                    )
                                }),
                        ),
                    ),
            )
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
