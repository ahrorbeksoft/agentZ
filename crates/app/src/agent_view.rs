//! The conversation with one agent. Layout, spacing and colors follow Zed's agent thread view
//! (`agent_ui::conversation_view::thread_view`).

use std::time::{Duration, SystemTime};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::agents::AgentId;
use agentz_protocol::diff::DiffScope;
use agentz_protocol::terminal::TerminalKey;
use agentz_protocol::thread::{
    ConnectionStatus, DiffLineKind, Entry, FileDiff, PlanItem, SessionRestore, ToolCall,
};
use agentz_protocol::{CAPABILITY_THREAD_DIFF, Request, Response};
use collections::{HashMap, HashSet};
use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, DragMoveEvent, Entity, EventEmitter,
    FocusHandle, Focusable, Hsla, KeyBinding, ScrollHandle, Subscription, Task, Window,
    pulsating_between,
};
use markdown::{Markdown, MarkdownElement, MarkdownFont, MarkdownStyle};
use projects::{TaskEnd, ThreadId};
use text_input::{TextInput, TextInputEvent};
use ui::{
    Callout, CommonAnimationExt as _, ContextMenu, Disclosure, IconPosition, PopoverMenu, Severity,
    SpinnerLabel, Switch, ToggleState, Tooltip, prelude::*,
};

use crate::project_store::ProjectStore;
use crate::registry_store::AgentRegistryStore;
use crate::server_client::{MachineStatus, ServerClient};
use crate::terminal_drawer::{TerminalDrawer, TerminalDrawerEvent};
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;
use crate::thread_entity::AgentThread;
use crate::{ToggleDiff, ToggleTerminalDrawer};

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
/// Every header along the top of a view: the thread, terminal and diff toolbars, the
/// workspace tabs (`ui::Tab`) and pane headers, and the sidebars' search and settings
/// rows, so their bottom borders line up.
pub(crate) const TOOLBAR_HEIGHT: Pixels = px(36.);
/// Unchanged lines shown around an edit, like a diff editor's context.
const DIFF_CONTEXT_LINES: usize = 3;
/// t3code's default drawer height.
const DRAWER_HEIGHT: Pixels = px(280.);
const MIN_DRAWER_HEIGHT: Pixels = px(100.);
/// What a dragged drawer leaves of the conversation.
const MIN_CONVERSATION_HEIGHT: Pixels = px(160.);
/// The strip along a panel's edge that drags to resize it.
pub(crate) const RESIZE_EDGE_SIZE: Pixels = px(6.);

/// The drawer's top edge, being dragged to resize it.
struct DraggedDrawerEdge;

/// The most lines of a command's terminal a tool call shows.
const TOOL_TERMINAL_MAX_LINES: usize = 16;

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
/// The [`MarkdownKey`] part holding a tool call's raw input.
const RAW_INPUT_PART: usize = usize::MAX;

pub enum AgentViewEvent {
    Unarchive,
    /// Show another thread: a subthread from the Agents control, or a subthread's parent.
    OpenThread(ThreadId),
}

/// The most subthreads the Agents control lists before it scrolls.
const MAX_AGENT_ROWS_SHOWN: usize = 6;

pub struct AgentView {
    thread_id: ThreadId,
    /// Focused instead of the message editor on a subthread, which has none.
    focus_handle: FocusHandle,
    /// Archived threads stay readable but take no new messages until they're unarchived.
    is_archived: bool,
    /// A workspace pane shows the title and this toolbar's buttons in its own header,
    /// so the thread doesn't stack a second header under it.
    shows_toolbar: bool,
    /// Whether the shell shows this thread's changes beside it.
    is_diff_open: bool,
    /// How many files the thread has changed, for the diff button's dot.
    changed_files: usize,
    /// The turn completion and connection `changed_files` was last asked for.
    changed_files_asked_for: Option<(Option<SystemTime>, bool)>,
    _changed_files_load: Task<()>,
    thread: Entity<AgentThread>,
    title: SharedString,
    /// The thread's machine.
    client: Entity<ServerClient>,
    store: Entity<ProjectStore>,
    registry: Entity<AgentRegistryStore>,
    agent_id: Option<AgentId>,
    composer: Entity<TextInput>,
    scroll_handle: ScrollHandle,
    markdowns: HashMap<MarkdownKey, Entity<Markdown>>,
    /// Tool calls the user opened or closed, relative to their default (edits open, others closed).
    toggled_tool_calls: HashSet<acp::ToolCallId>,
    expanded_raw_inputs: HashSet<acp::ToolCallId>,
    /// Keeps a streaming thought scrolled to its newest text while it's height-limited.
    thought_scroll_handles: HashMap<usize, ScrollHandle>,
    toggled_thoughts: HashSet<usize>,
    plan_expanded: bool,
    edits_expanded: bool,
    /// Messages typed while the agent works; sent one at a time as each turn ends, like Zed.
    queued_messages: Vec<String>,
    queue_expanded: bool,
    command_menu_index: usize,
    /// The composer text for which the user dismissed the slash-command menu.
    command_menu_dismissed_for: Option<SharedString>,
    agents_expanded: bool,
    /// Subthreads at any depth waiting for a permission answer, which is given here (t3code).
    blocked_subthreads: HashMap<ThreadId, (Entity<AgentThread>, Subscription)>,
    /// The thread's terminals (t3code's drawer). Kept while hidden, so its layout stays.
    drawer: Option<(Entity<TerminalDrawer>, Subscription)>,
    is_drawer_open: bool,
    /// The drawer's height, as its top edge was last dragged.
    drawer_height: Pixels,
    /// The drawer fills the thread's area.
    drawer_full_screen: bool,
    /// The terminals the agent runs its commands in, by the ids it got, shown in their tool
    /// calls.
    tool_terminals: HashMap<String, Entity<TerminalView>>,
    _subscriptions: Vec<Subscription>,
    _elapsed_refresh: Task<()>,
}

impl AgentView {
    pub fn new(
        thread_id: ThreadId,
        thread: Entity<AgentThread>,
        title: SharedString,
        agent_id: Option<AgentId>,
        cx: &mut Context<Self>,
    ) -> Self {
        let client = thread.read(cx).client().clone();
        let store = client.read(cx).projects().clone();
        let registry = client.read(cx).registry().clone();
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
            cx.observe(&store, |this, _, cx| {
                this.sync_blocked_subthreads(cx);
                this.load_changed_files(false, cx);
                cx.notify();
            }),
            // Whether messages can be sent follows the machine's connection.
            cx.observe(&client, |this, _, cx| {
                this.load_changed_files(false, cx);
                cx.notify();
            }),
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
            thread_id,
            focus_handle: cx.focus_handle(),
            thread,
            title,
            is_archived: false,
            shows_toolbar: true,
            is_diff_open: false,
            changed_files: 0,
            changed_files_asked_for: None,
            _changed_files_load: Task::ready(()),
            client,
            store,
            registry,
            agent_id,
            composer,
            scroll_handle: ScrollHandle::new(),
            markdowns: HashMap::default(),
            toggled_tool_calls: HashSet::default(),
            expanded_raw_inputs: HashSet::default(),
            thought_scroll_handles: HashMap::default(),
            toggled_thoughts: HashSet::default(),
            plan_expanded: false,
            edits_expanded: false,
            queued_messages: Vec::new(),
            queue_expanded: false,
            command_menu_index: 0,
            command_menu_dismissed_for: None,
            agents_expanded: true,
            blocked_subthreads: HashMap::default(),
            drawer: None,
            is_drawer_open: false,
            drawer_height: DRAWER_HEIGHT,
            drawer_full_screen: false,
            tool_terminals: HashMap::default(),
            _subscriptions: subscriptions,
            _elapsed_refresh: elapsed_refresh,
        };
        this.sync_markdowns(cx);
        this.sync_blocked_subthreads(cx);
        this.load_changed_files(false, cx);
        this
    }

    /// Follows the subthreads that wait for a permission answer, and stops following those
    /// that no longer do.
    fn sync_blocked_subthreads(&mut self, cx: &mut Context<Self>) {
        let store = self.store.clone();
        let blocked: Vec<ThreadId> = store
            .read(cx)
            .thread_and_subthreads(self.thread_id)
            .into_iter()
            .skip(1)
            .filter(|thread_id| store.read(cx).is_thread_blocked(*thread_id))
            .collect();
        self.blocked_subthreads
            .retain(|thread_id, _| blocked.contains(thread_id));
        for thread_id in blocked {
            if self.blocked_subthreads.contains_key(&thread_id) {
                continue;
            }
            let thread = AgentThread::shared(&self.client, thread_id, cx);
            let subscription = cx.observe(&thread, |_, _, cx| cx.notify());
            self.blocked_subthreads
                .insert(thread_id, (thread, subscription));
        }
    }

    /// The thread that delegated this one, for a subthread.
    fn parent(&self, cx: &App) -> Option<ThreadId> {
        self.store.clone().read(cx).thread(self.thread_id)?.parent()
    }

    /// t3code's Agents control: the thread's subthreads, each with its state, title and agent.
    fn render_agents_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let store = self.store.clone();
        let store = store.read(cx);
        let subthreads: Vec<projects::Thread> = store
            .subthreads(self.thread_id)
            .into_iter()
            .cloned()
            .collect();
        if subthreads.is_empty() {
            return None;
        }
        let colors = cx.theme().colors();
        let count = subthreads.len();
        let running = subthreads
            .iter()
            .filter(|thread| {
                thread
                    .task
                    .as_ref()
                    .is_some_and(|task| task.outcome.is_none())
            })
            .count();
        let expanded = self.agents_expanded;
        let title = if count == 1 {
            "1 Agent".to_string()
        } else {
            format!("{count} Agents")
        };
        let summary = h_flex()
            .id("agents-summary")
            .p_1()
            .w_full()
            .gap_1()
            .cursor_pointer()
            .when(expanded, |this| {
                this.border_b_1().border_color(colors.border)
            })
            .child(Disclosure::new("agents-disclosure", expanded))
            .child(Label::new(title).size(LabelSize::Small).color(Color::Muted))
            .when(running > 0, |this| {
                this.child(
                    Label::new(format!("· {running} running"))
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.agents_expanded = !this.agents_expanded;
                cx.notify();
            }));

        let registry = self.registry.read(cx);
        let rows: Vec<AnyElement> = subthreads
            .iter()
            .enumerate()
            .map(|(index, thread)| {
                let thread_id = thread.id;
                let outcome = thread.task.as_ref().and_then(|task| task.outcome.as_ref());
                let (icon, label, color) = match outcome.map(|outcome| outcome.end) {
                    Some(TaskEnd::Completed) => (IconName::Check, "Done", Color::Success),
                    Some(TaskEnd::Failed) => (IconName::XCircle, "Failed", Color::Error),
                    Some(TaskEnd::Cancelled) => (IconName::Stop, "Cancelled", Color::Muted),
                    Some(TaskEnd::Interrupted) => (IconName::Stop, "Stopped", Color::Muted),
                    None if store.is_thread_or_subthread_blocked(thread_id) => {
                        (IconName::Warning, "Needs approval", Color::Warning)
                    }
                    None if store.is_thread_working(thread_id) => {
                        (IconName::LoadCircle, "Working", Color::Accent)
                    }
                    None => (IconName::Clock, "Waiting", Color::Muted),
                };
                let icon = Icon::new(icon).size(IconSize::Small).color(color);
                let icon = if outcome.is_none() && label == "Working" {
                    icon.with_rotate_animation(2).into_any_element()
                } else {
                    icon.into_any_element()
                };
                let agent_name = thread
                    .agent_id
                    .as_ref()
                    .map(|agent_id| {
                        let agent_id = AgentId::new(agent_id.clone());
                        registry
                            .agent(&agent_id)
                            .map(|agent| agent.name().clone())
                            .unwrap_or(agent_id.0)
                    })
                    .unwrap_or_else(|| "Agent".into());
                let role = thread.task.as_ref().and_then(|task| task.role.clone());
                let details = match role {
                    Some(role) => format!("{agent_name} · {role}"),
                    None => agent_name.to_string(),
                };
                h_flex()
                    .id(("agent-row", thread_id.0))
                    .w_full()
                    .p_1p5()
                    .gap_1p5()
                    .bg(colors.editor_background)
                    .cursor_pointer()
                    .hover(|this| this.bg(colors.element_hover))
                    .when(index + 1 < count, |this| {
                        this.border_b_1().border_color(colors.border_variant)
                    })
                    .child(icon)
                    .child(
                        div().flex_1().min_w_0().child(
                            Label::new(thread.title.clone())
                                .size(LabelSize::Small)
                                .truncate(),
                        ),
                    )
                    .child(
                        Label::new(details)
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    )
                    .child(Label::new(label).size(LabelSize::Small).color(color))
                    .tooltip(Tooltip::text("Open this agent's thread"))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(AgentViewEvent::OpenThread(thread_id))
                    }))
                    .into_any_element()
            })
            .collect();
        Some(
            v_flex()
                .child(summary)
                .when(expanded, |this| {
                    this.child(
                        v_flex()
                            .id("agent-rows")
                            .max_h(rems_from_px(31. * MAX_AGENT_ROWS_SHOWN as f32))
                            .overflow_y_scroll()
                            .children(rows),
                    )
                })
                .into_any_element(),
        )
    }

    /// The permission requests of subthreads, answered here for them.
    fn render_subthread_permissions(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let store = self.store.clone();
        let mut subthreads: Vec<(&ThreadId, &(Entity<AgentThread>, Subscription))> =
            self.blocked_subthreads.iter().collect();
        subthreads.sort_by_key(|(thread_id, _)| **thread_id);
        let mut cards = Vec::new();
        for (thread_id, (thread, _)) in subthreads {
            let thread_id = *thread_id;
            let title = store
                .read(cx)
                .thread(thread_id)
                .map(|thread| thread.title.clone())
                .unwrap_or_default();
            let agent_name = thread.read(cx).state.agent_name.clone();
            for (request_index, request) in
                thread.read(cx).state.permission_requests.iter().enumerate()
            {
                let mut buttons = Vec::new();
                for (option_index, option) in request.options.iter().enumerate() {
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
                    let thread = thread.clone();
                    let tool_call_id = request.tool_call_id.clone();
                    let option_id = option.id.clone();
                    buttons.push(
                        Button::new(
                            SharedString::from(format!(
                                "subthread-permission-{}-{request_index}-{option_index}",
                                thread_id.0
                            )),
                            option.name.clone(),
                        )
                        .start_icon(icon)
                        .label_size(LabelSize::Small)
                        .on_click(move |_, _, cx| {
                            let option_id = option_id.clone();
                            thread.update(cx, |thread, cx| {
                                thread.respond_to_permission(&tool_call_id, option_id, cx)
                            });
                        }),
                    );
                }
                cards.push(
                    v_flex()
                        .my_1p5()
                        .mx_5()
                        .rounded_md()
                        .border_1()
                        .border_color(Self::tool_card_border_color(cx))
                        .bg(cx.theme().colors().editor_background)
                        .overflow_hidden()
                        .child(
                            h_flex()
                                .id(("subthread-permission", thread_id.0))
                                .px_2()
                                .py_1()
                                .gap_1p5()
                                .bg(Self::tool_card_header_bg(cx))
                                .cursor_pointer()
                                .child(
                                    Icon::new(IconName::Warning)
                                        .size(IconSize::Small)
                                        .color(Color::Warning),
                                )
                                .child(
                                    Label::new(format!("{agent_name} in “{title}” wants to:"))
                                        .size(LabelSize::Small)
                                        .color(Color::Muted),
                                )
                                .child(
                                    Label::new(request.title.clone())
                                        .size(LabelSize::Small)
                                        .truncate(),
                                )
                                .tooltip(Tooltip::text("Open the subagent's thread"))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(AgentViewEvent::OpenThread(thread_id))
                                })),
                        )
                        .child(
                            v_flex()
                                .p_1()
                                .border_t_1()
                                .border_color(Self::tool_card_border_color(cx))
                                .gap_0p5()
                                .children(buttons),
                        )
                        .into_any_element(),
                );
            }
        }
        cards
    }

    /// Stands in for the message editor on a subthread, as t3code's subagent bar does: its
    /// messages come from its parent.
    fn render_subthread_bar(&self, parent: ThreadId, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let store = self.store.clone();
        let parent_title = store
            .read(cx)
            .thread(parent)
            .map(|thread| thread.title.clone())
            .unwrap_or_default();
        let is_working = self.thread.read(cx).is_working();
        h_flex()
            .py_2()
            .px_4()
            .gap_2()
            .bg(colors.editor_background)
            .border_t_1()
            .border_color(colors.border)
            .child(
                self.agent_icon(cx)
                    .size(IconSize::Small)
                    .color(Color::Muted),
            )
            .child(
                div().flex_1().min_w_0().child(
                    Label::new(format!(
                        "A subagent of “{parent_title}”. It runs on its own; message its parent instead."
                    ))
                    .size(LabelSize::Small)
                    .color(Color::Muted)
                    .truncate(),
                ),
            )
            .when(is_working, |this| {
                this.child(
                    Button::new("stop-subthread", "Stop")
                        .label_size(LabelSize::Small)
                        .start_icon(Icon::new(IconName::Stop).size(IconSize::XSmall).color(Color::Error))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.thread.update(cx, |thread, cx| thread.cancel(cx))
                        })),
                )
            })
            .child(
                Button::new("open-parent", "Open Parent")
                    .label_size(LabelSize::Small)
                    .start_icon(Icon::new(IconName::ArrowUpRight).size(IconSize::XSmall))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(AgentViewEvent::OpenThread(parent))
                    })),
            )
            .into_any_element()
    }

    pub fn hide_toolbar(&mut self, cx: &mut Context<Self>) {
        self.shows_toolbar = false;
        cx.notify();
    }

    pub fn set_archived(&mut self, is_archived: bool, cx: &mut Context<Self>) {
        if self.is_archived != is_archived {
            self.is_archived = is_archived;
            cx.notify();
        }
    }

    pub fn set_diff_open(&mut self, is_diff_open: bool, cx: &mut Context<Self>) {
        if self.is_diff_open != is_diff_open {
            self.is_diff_open = is_diff_open;
            // Reverting from the panel changes the files without a turn.
            if !is_diff_open {
                self.load_changed_files(true, cx);
            }
            cx.notify();
        }
    }

    /// Asks the server how many files the thread has changed, after each turn and on
    /// reconnecting, or now when `force`d.
    fn load_changed_files(&mut self, force: bool, cx: &mut Context<Self>) {
        let completed_at = self
            .store
            .read(cx)
            .thread(self.thread_id)
            .and_then(|thread| thread.completed_at);
        let client = self.client.read(cx);
        let asked_for = (completed_at, client.connection().is_some());
        if !force && self.changed_files_asked_for == Some(asked_for) {
            return;
        }
        self.changed_files_asked_for = Some(asked_for);
        if !client.has_capability(CAPABILITY_THREAD_DIFF) {
            return;
        }
        let request = client.request(Request::ThreadDiff {
            thread_id: self.thread_id,
            scope: DiffScope::All,
        });
        self._changed_files_load = cx.spawn(async move |this, cx| {
            let changed_files = match request.await {
                Ok(Response::ThreadDiff(diff)) => diff.files.len(),
                Ok(response) => {
                    log::error!("expected a diff, got {response:?}");
                    return;
                }
                Err(error) => {
                    log::warn!("couldn't load the thread's changes: {error:#}");
                    return;
                }
            };
            this.update(cx, |this, cx| {
                if this.changed_files != changed_files {
                    this.changed_files = changed_files;
                    cx.notify();
                }
            })
            .ok();
        });
    }

    /// Opens the thread's terminal under it and focuses it, or closes it.
    pub fn toggle_terminal_drawer(
        &mut self,
        _: &ToggleTerminalDrawer,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_drawer_open {
            self.is_drawer_open = false;
            self.drawer_full_screen = false;
            window.focus(&self.focus_handle(cx), cx);
        } else {
            let drawer = match &self.drawer {
                Some((drawer, _)) => drawer.clone(),
                None => {
                    let client = self.client.clone();
                    let thread_id = self.thread_id;
                    let drawer = cx.new(|cx| TerminalDrawer::new(client, thread_id, cx));
                    let events = cx.subscribe(&drawer, |this, drawer, event, cx| match event {
                        TerminalDrawerEvent::ToggleFullScreen => {
                            this.drawer_full_screen = !this.drawer_full_screen;
                            let is_full_screen = this.drawer_full_screen;
                            drawer.update(cx, |drawer, cx| {
                                drawer.set_full_screen(is_full_screen, cx)
                            });
                            cx.notify();
                        }
                        // Its last terminal closed, so the drawer does too.
                        TerminalDrawerEvent::Empty => {
                            this.drawer = None;
                            this.is_drawer_open = false;
                            this.drawer_full_screen = false;
                            cx.notify();
                        }
                    });
                    self.drawer = Some((drawer.clone(), events));
                    drawer
                }
            };
            self.is_drawer_open = true;
            window.focus(&drawer.focus_handle(cx), cx);
        }
        let is_full_screen = self.drawer_full_screen;
        if let Some((drawer, _)) = &self.drawer {
            drawer.update(cx, |drawer, cx| drawer.set_full_screen(is_full_screen, cx));
        }
        cx.notify();
    }

    /// The drawer under the conversation, or filling the thread's area.
    fn render_drawer(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (drawer, _) = self.drawer.as_ref().filter(|_| self.is_drawer_open)?;
        let colors = cx.theme().colors();
        let is_full_screen = self.drawer_full_screen;
        // The top edge drags to resize, as t3code's does.
        let resize_edge = div()
            .id("drawer-resize-edge")
            .absolute()
            .top(-RESIZE_EDGE_SIZE / 2.)
            .left_0()
            .w_full()
            .h(RESIZE_EDGE_SIZE)
            .cursor_row_resize()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_drag(DraggedDrawerEdge, |_, _, _, cx| cx.new(|_| gpui::Empty));
        Some(
            div()
                .relative()
                .map(|this| {
                    if is_full_screen {
                        this.flex_1().min_h_0()
                    } else {
                        this.h(self.drawer_height).flex_none()
                    }
                })
                .border_t_1()
                .border_color(colors.border)
                .child(drawer.clone())
                .when(!is_full_screen, |this| this.child(resize_edge))
                .into_any_element(),
        )
    }

    pub fn set_title(&mut self, title: SharedString, cx: &mut Context<Self>) {
        if self.title != title {
            self.title = title;
            cx.notify();
        }
    }

    /// Keeps a markdown entity per message so streamed text is appended instead of reparsed.
    fn sync_markdowns(&mut self, cx: &mut Context<Self>) {
        let entries = self.thread.read(cx).entries().to_vec();
        for (index, entry) in entries.iter().enumerate() {
            match entry {
                Entry::UserMessage(text) | Entry::AgentMessage(text) => {
                    self.sync_markdown((index, 0), text, cx);
                }
                Entry::AgentThought(text) => {
                    self.sync_markdown((index, 0), text, cx);
                    self.thought_scroll_handles.entry(index).or_default();
                }
                Entry::ToolCall(tool_call) => {
                    for (part, text) in tool_call.text.iter().enumerate() {
                        self.sync_markdown((index, part + 1), text, cx);
                    }
                    if let Some(raw_input) = &tool_call.raw_input {
                        self.sync_markdown((index, RAW_INPUT_PART), raw_input, cx);
                    }
                    for terminal_id in &tool_call.terminals {
                        if !self.tool_terminals.contains_key(terminal_id) {
                            let terminal = Terminal::shared(
                                &self.client,
                                TerminalKey::Agent {
                                    thread_id: self.thread_id,
                                    terminal_id: terminal_id.clone(),
                                },
                                cx,
                            );
                            let view = cx.new(|cx| {
                                TerminalView::new(
                                    terminal,
                                    TerminalMode::Inline {
                                        max_lines: TOOL_TERMINAL_MAX_LINES,
                                    },
                                    cx,
                                )
                            });
                            self.tool_terminals.insert(terminal_id.clone(), view);
                        }
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
        if self.is_archived || !self.client.read(cx).is_online() {
            return;
        }
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
        // As in Zed, `/login` and `/logout` bring up the agent's login methods, unless the agent
        // has its own `/logout`, which may need to reset its state.
        let trimmed = text.trim();
        if trimmed == "/login" || trimmed == "/logout" {
            let thread = self.thread.read(cx);
            let can_login = !thread.auth_methods().is_empty();
            let agent_handles_logout = trimmed == "/logout"
                && thread
                    .available_commands()
                    .iter()
                    .any(|command| command.name == "logout");
            if can_login && !agent_handles_logout {
                self.composer
                    .update(cx, |composer, cx| composer.set_text("", cx));
                self.thread
                    .update(cx, |thread, cx| thread.reauthenticate(cx));
                return;
            }
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
        if self.is_archived
            || self.queued_messages.is_empty()
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

    pub(crate) fn agent_name(&self, cx: &App) -> SharedString {
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

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let agent_name = self.agent_name(cx);
        h_flex()
            .h(TOOLBAR_HEIGHT)
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
            .child(div().flex_1())
            .child(self.render_toolbar_buttons(cx))
    }

    /// The terminal, changes and options buttons, also shown in a workspace pane's header.
    pub(crate) fn render_toolbar_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let thread = self.thread.clone();
        h_flex()
            .gap_1p5()
            .child({
                // With the drawer hidden, a dot says something still runs in its terminals.
                let running: Vec<String> = if self.is_drawer_open {
                    Vec::new()
                } else {
                    self.client
                        .read(cx)
                        .projects()
                        .read(cx)
                        .drawer_commands(self.thread_id)
                        .map(|(_, command)| command.to_string())
                        .collect()
                };
                let tooltip: SharedString = match running.as_slice() {
                    [] => "Toggle Terminal".into(),
                    [command] => format!("Toggle Terminal · {command} running").into(),
                    commands => format!("Toggle Terminal · {} running", commands.join(", ")).into(),
                };
                div()
                    .relative()
                    .child(
                        IconButton::new("toggle-terminal-drawer", IconName::Terminal)
                            .icon_size(IconSize::Small)
                            .toggle_state(self.is_drawer_open)
                            .tooltip(move |_, cx| {
                                Tooltip::for_action(tooltip.clone(), &ToggleTerminalDrawer, cx)
                            })
                            // Directly: dispatched, the action would start wherever focus is,
                            // which may be outside this view.
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_terminal_drawer(&ToggleTerminalDrawer, window, cx)
                            })),
                    )
                    .when(!running.is_empty(), |button| {
                        button.child(indicator_dot(cx))
                    })
            })
            .child({
                // With the changes hidden, a dot says the thread has changed files.
                let changed_files = if self.is_diff_open {
                    0
                } else {
                    self.changed_files
                };
                let tooltip: SharedString = match changed_files {
                    0 => "Show Changes".into(),
                    1 => "Show Changes · 1 file changed".into(),
                    count => format!("Show Changes · {count} files changed").into(),
                };
                div()
                    .relative()
                    .child(
                        IconButton::new("toggle-diff", IconName::Diff)
                            .icon_size(IconSize::Small)
                            .toggle_state(self.is_diff_open)
                            .tooltip(move |_, cx| {
                                Tooltip::for_action(tooltip.clone(), &ToggleDiff, cx)
                            })
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(ToggleDiff), cx)
                            }),
                    )
                    .when(changed_files > 0, |button| button.child(indicator_dot(cx)))
            })
            .child(
                // Zed's agent options: log in again, log out, or restart the agent.
                PopoverMenu::new("thread-options")
                    .menu(move |window, cx| {
                        let thread = thread.clone();
                        let (has_auth_methods, supports_logout, can_reload) = {
                            let thread = thread.read(cx);
                            (
                                !thread.auth_methods().is_empty(),
                                thread.supports_logout()
                                    && thread.status() == &ConnectionStatus::Ready,
                                !matches!(thread.status(), ConnectionStatus::Connecting),
                            )
                        };
                        Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                            if has_auth_methods {
                                let thread = thread.clone();
                                menu = menu.entry("Reauthenticate", None, move |_, cx| {
                                    thread.update(cx, |thread, cx| thread.reauthenticate(cx))
                                });
                            }
                            if supports_logout {
                                let thread = thread.clone();
                                menu = menu.entry("Log Out", None, move |_, cx| {
                                    thread.update(cx, |thread, cx| thread.logout(cx))
                                });
                            }
                            if has_auth_methods || supports_logout {
                                menu = menu.separator();
                            }
                            menu.when(can_reload, |menu| {
                                let thread = thread.clone();
                                menu.entry("Reload Agent", None, move |_, cx| {
                                    thread.update(cx, |thread, cx| thread.reload(cx))
                                })
                            })
                        }))
                    })
                    .trigger_with_tooltip(
                        IconButton::new("thread-options-trigger", IconName::Ellipsis)
                            .icon_size(IconSize::Small),
                        Tooltip::text("Agent Options"),
                    )
                    .anchor(gpui::Anchor::TopRight),
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
                // Messages from other threads' agents are marked, as t3code marks
                // `createdBy: agent`.
                let sent_by = self.thread.read(cx).prompt_sender(index).map(|sender| {
                    format!(
                        "Sent by {}",
                        self.store.clone().read(cx).describe_creator(sender)
                    )
                });
                v_flex()
                    .id(("user-message", index))
                    .pt_2()
                    .pb_3()
                    .px_2()
                    .w_full()
                    .when_some(sent_by, |this, sent_by| {
                        this.child(
                            h_flex()
                                .px_1()
                                .pb_1()
                                .gap_1()
                                .child(
                                    Icon::new(IconName::Sparkle)
                                        .size(IconSize::XSmall)
                                        .color(Color::Muted),
                                )
                                .child(
                                    Label::new(sent_by)
                                        .size(LabelSize::XSmall)
                                        .color(Color::Muted),
                                ),
                        )
                    })
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
        // Like Zed: a thought that is still streaming is open but height-limited and follows
        // the newest text; once done it collapses. Clicking flips the default.
        let open_by_default = is_last && self.thread.read(cx).is_working();
        let is_toggled = self.toggled_thoughts.contains(&index);
        let is_open = open_by_default != is_toggled;
        let is_constrained = open_by_default && !is_toggled;
        let scroll_handle = self.thought_scroll_handles.get(&index).cloned();
        if is_constrained && let Some(scroll_handle) = &scroll_handle {
            scroll_handle.scroll_to_bottom();
        }
        let panel_background = cx.theme().colors().panel_background;
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
                                .when(is_constrained, |this| this.relative())
                                .child(
                                    div()
                                        .id(("thinking-content", index))
                                        .ml_1p5()
                                        .pl_3p5()
                                        .border_l_1()
                                        .border_color(Self::tool_card_border_color(cx))
                                        .when(is_constrained, |this| this.max_h_64())
                                        .when_some(scroll_handle, |this, scroll_handle| {
                                            this.track_scroll(&scroll_handle)
                                        })
                                        .overflow_hidden()
                                        .children(self.markdown((index, 0), style)),
                                )
                                .when(is_constrained, |this| {
                                    this.child(
                                        div()
                                            .absolute()
                                            .inset_0()
                                            .size_full()
                                            .bg(gpui::linear_gradient(
                                                180.,
                                                gpui::linear_color_stop(
                                                    panel_background.opacity(0.8),
                                                    0.,
                                                ),
                                                gpui::linear_color_stop(
                                                    panel_background.opacity(0.),
                                                    0.1,
                                                ),
                                            ))
                                            .block_mouse_except_scroll(),
                                    )
                                }),
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
        let should_show_raw_input = !is_terminal_tool && !is_edit;
        let has_content = !tool_call.text.is_empty()
            || !tool_call.diffs.is_empty()
            || !tool_call.terminals.is_empty()
            || (should_show_raw_input && tool_call.raw_input.is_some());
        let is_collapsible = has_content && !needs_confirmation;
        // Like Zed (with its default `expand_edit_card`), edits start open and everything else
        // starts collapsed; clicking flips that.
        // A command's live terminal starts open too, as Zed's terminal cards do.
        let open_by_default = is_edit || !tool_call.terminals.is_empty();
        let is_open = needs_confirmation
            || open_by_default != self.toggled_tool_calls.contains(&tool_call.id);
        let header_group = SharedString::from(format!("tool-call-header-{index}"));

        let label = self.render_tool_call_label(tool_call, use_card_layout, window, cx);
        let toggle = {
            let view = cx.entity().downgrade();
            let tool_call_id = tool_call.id.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut App| {
                view.update(cx, |this, cx| {
                    if !this.toggled_tool_calls.remove(&tool_call_id) {
                        this.toggled_tool_calls.insert(tool_call_id.clone());
                    }
                    cx.notify();
                })
                .ok();
            }
        };
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
                        this.child(
                            Disclosure::new(("tool-call-disclosure", index), is_open)
                                .opened_icon(IconName::ChevronUp)
                                .closed_icon(IconName::ChevronDown)
                                .visible_on_hover(&header_group)
                                .on_click(toggle.clone()),
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

        let input_output_header = |label: &'static str| {
            Label::new(label)
                .size(LabelSize::XSmall)
                .color(Color::Muted)
                .buffer_font(cx)
        };

        let mut output = Vec::new();
        if is_open {
            if needs_confirmation {
                // Zed tucks the raw input behind "View Raw Input" while awaiting permission.
                if should_show_raw_input && tool_call.raw_input.is_some() {
                    let is_raw_input_expanded = self.expanded_raw_inputs.contains(&tool_call.id);
                    let tool_call_id = tool_call.id.clone();
                    output.push(
                        v_flex()
                            .p_2()
                            .gap_1()
                            .border_t_1()
                            .border_color(Self::tool_card_border_color(cx))
                            .child(
                                h_flex()
                                    .id(("raw-input-toggle", index))
                                    .pl_0p5()
                                    .gap_1()
                                    .justify_between()
                                    .rounded_xs()
                                    .cursor_pointer()
                                    .hover(|this| this.bg(cx.theme().colors().element_hover))
                                    .child(input_output_header(if is_raw_input_expanded {
                                        "Raw Input:"
                                    } else {
                                        "View Raw Input"
                                    }))
                                    .child(
                                        Disclosure::new(
                                            ("raw-input-disclosure", index),
                                            is_raw_input_expanded,
                                        )
                                        .opened_icon(IconName::ChevronUp)
                                        .closed_icon(IconName::ChevronDown),
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if !this.expanded_raw_inputs.remove(&tool_call_id) {
                                            this.expanded_raw_inputs.insert(tool_call_id.clone());
                                        }
                                        cx.notify();
                                    })),
                            )
                            .when(is_raw_input_expanded, |this| {
                                this.children(self.markdown(
                                    (index, RAW_INPUT_PART),
                                    MarkdownStyle::themed(MarkdownFont::Agent, window, cx),
                                ))
                            })
                            .into_any_element(),
                    );
                }
            } else if should_show_raw_input && tool_call.raw_input.is_some() {
                output.push(
                    v_flex()
                        .mt_1p5()
                        .w_full()
                        .ml(rems(0.4))
                        .px_3p5()
                        .pb_1()
                        .gap_1()
                        .border_l_1()
                        .border_color(Self::tool_card_border_color(cx))
                        .child(input_output_header("Raw Input:"))
                        .children(self.markdown(
                            (index, RAW_INPUT_PART),
                            MarkdownStyle::themed(MarkdownFont::Agent, window, cx),
                        ))
                        .child(input_output_header("Output:"))
                        .into_any_element(),
                );
            }
            for diff in &tool_call.diffs {
                output.push(render_diff(diff, cx));
            }
            for terminal_id in &tool_call.terminals {
                if let Some(terminal) = self.tool_terminals.get(terminal_id) {
                    output.push(
                        div()
                            .w_full()
                            .py_1()
                            .when(use_card_layout, |this| {
                                this.border_t_1()
                                    .border_color(Self::tool_card_border_color(cx))
                            })
                            .bg(cx.theme().colors().terminal_background)
                            .child(terminal.clone())
                            .into_any_element(),
                    );
                }
            }
            for part in 0..tool_call.text.len() {
                let style = tool_output_style(is_terminal_tool, window, cx);
                if let Some(markdown) = self.markdown((index, part + 1), style) {
                    output.push(
                        div()
                            .id(SharedString::from(format!("tool-output-{index}-{part}")))
                            .when(use_card_layout, |this| {
                                this.p_2()
                                    .border_t_1()
                                    .border_color(Self::tool_card_border_color(cx))
                            })
                            // Long command output scrolls inside the card, like Zed's terminal
                            // card (`h_72`).
                            .when(is_terminal_tool, |this| this.max_h_72().overflow_y_scroll())
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
            if !use_card_layout && is_collapsible {
                output.push(
                    div()
                        .ml(rems(0.4))
                        .px_3p5()
                        .pt_2()
                        .border_l_1()
                        .border_color(Self::tool_card_border_color(cx))
                        .child(
                            IconButton::new(("tool-call-collapse", index), IconName::ChevronUp)
                                .full_width()
                                .style(ButtonStyle::Outlined)
                                .icon_color(Color::Muted)
                                .on_click(toggle),
                        )
                        .into_any_element(),
                );
            }
        }

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
            .children(output)
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
            raw_input: None,
            terminals: Vec::new(),
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
        let elapsed = started_at.elapsed().unwrap_or_default().as_secs();
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
    /// Takes the composer's place while the thread is archived, like t3code's notice for a
    /// settled thread.
    fn render_archived_notice(&self, cx: &mut Context<Self>) -> AnyElement {
        Callout::new()
            .border_position(ui::CalloutBorderPosition::Top)
            .severity(Severity::Info)
            .icon(IconName::Info)
            .title("This thread is archived")
            .description("Unarchive it to send new messages.")
            .actions_slot(
                Button::new("unarchive-thread", "Unarchive")
                    .style(ButtonStyle::Filled)
                    .label_size(LabelSize::Small)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(AgentViewEvent::Unarchive))),
            )
            .into_any_element()
    }

    /// While the thread's machine is unreachable, in place of the message editor. The draft
    /// stays in the editor for when it's back.
    fn render_offline_notice(&self, cx: &App) -> AnyElement {
        let label = self.client.read(cx).label().clone();
        let description = match self.client.read(cx).status() {
            MachineStatus::Attention { error, .. } | MachineStatus::Reconnecting(error) => {
                format!("{error}. New messages can be sent once it reconnects.")
            }
            MachineStatus::Stopped => {
                "Its agentz-server is stopped. Start it in Settings › Machines to send messages."
                    .to_string()
            }
            MachineStatus::Connecting | MachineStatus::Online => {
                "New messages can be sent once it reconnects.".to_string()
            }
        };
        Callout::new()
            .border_position(ui::CalloutBorderPosition::Top)
            .severity(Severity::Warning)
            .icon(IconName::Disconnected)
            .title(format!("{label} is offline"))
            .description(description)
            .into_any_element()
    }

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
        let title = if count == 1 {
            "1 Queued Message".to_string()
        } else {
            format!("{count} Queued Messages")
        };
        let summary = h_flex()
            .p_1()
            .w_full()
            .gap_1()
            .justify_between()
            .when(expanded, |this| {
                this.border_b_1().border_color(colors.border)
            })
            .child(
                h_flex()
                    .id("queue-summary")
                    .gap_1()
                    .cursor_pointer()
                    .child(Disclosure::new("queue-disclosure", expanded))
                    .child(Label::new(title).size(LabelSize::Small).color(Color::Muted))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.queue_expanded = !this.queue_expanded;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("clear-queue", "Clear All")
                    .label_size(LabelSize::Small)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.queued_messages.clear();
                        cx.notify();
                    })),
            );

        let mut rows = Vec::new();
        for (index, message) in self.queued_messages.iter().enumerate() {
            let is_next = index == 0;
            rows.push(
                h_flex()
                    .group("queue-entry")
                    .w_full()
                    .p_1p5()
                    .gap_1()
                    .bg(colors.editor_background)
                    .when(index + 1 < count, |this| {
                        this.border_b_1().border_color(colors.border_variant)
                    })
                    .child(
                        div()
                            .id(("queue-entry-dot", index))
                            .child(
                                Icon::new(IconName::Circle)
                                    .size(IconSize::Small)
                                    .color(if is_next { Color::Accent } else { Color::Muted }),
                            )
                            .tooltip(Tooltip::text(if is_next {
                                "Next in Queue"
                            } else {
                                "In Queue"
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .child(message.lines().next().unwrap_or_default().to_string()),
                    )
                    .child(
                        h_flex()
                            .when(!is_next, |this| this.visible_on_hover("queue-entry"))
                            .gap_1()
                            .min_w(rems_from_px(160_f32))
                            .justify_end()
                            .child(
                                IconButton::new(("delete-queued", index), IconName::Trash)
                                    .icon_size(IconSize::Small)
                                    .tooltip(Tooltip::text("Remove Message from Queue"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index < this.queued_messages.len() {
                                            this.queued_messages.remove(index);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                IconButton::new(("edit-queued", index), IconName::Pencil)
                                    .icon_size(IconSize::Small)
                                    .tooltip(Tooltip::text("Edit"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        if index < this.queued_messages.len() {
                                            let text = this.queued_messages.remove(index);
                                            this.composer.update(cx, |composer, cx| {
                                                composer.set_text(text, cx)
                                            });
                                            window.focus(&this.composer.focus_handle(cx), cx);
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(("send-queued-now", index), "Send Now")
                                    .label_size(LabelSize::Small)
                                    .when(is_next, |this| this.style(ButtonStyle::Outlined))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.send_queued_message_now(index, cx);
                                    })),
                            ),
                    ),
            );
        }
        Some(
            v_flex()
                .child(summary)
                .when(expanded, |this| {
                    this.child(
                        v_flex()
                            .id("queued-messages")
                            .max_h_40()
                            .overflow_y_scroll()
                            .children(rows),
                    )
                })
                .into_any_element(),
        )
    }

    /// Moves a queued message to the front and sends it as soon as possible, stopping the
    /// current turn if the agent is working.
    fn send_queued_message_now(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.queued_messages.len() {
            return;
        }
        let message = self.queued_messages.remove(index);
        self.queued_messages.insert(0, message);
        if self.thread.read(cx).is_working() {
            self.thread.update(cx, |thread, cx| thread.cancel(cx));
        } else {
            self.send_next_queued_message(cx);
        }
        cx.notify();
    }

    /// The bar above the message editor: agents, plan, edited files and queued messages.
    fn render_activity_bar(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let sections: Vec<AnyElement> = [
            self.render_agents_section(cx),
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
    v_flex()
        .w_full()
        .border_t_1()
        .border_color(colors.border.opacity(0.8))
        .font_buffer(cx)
        .text_size(rems_from_px(12_f32))
        .line_height(rems_from_px(18_f32))
        .children(
            diff.hunk(DIFF_CONTEXT_LINES)
                .into_iter()
                .map(|(kind, line)| {
                    let (marker, background) = match kind {
                        DiffLineKind::Context => (" ", None),
                        DiffLineKind::Removed => ("-", Some(status.deleted_background)),
                        DiffLineKind::Added => ("+", Some(status.created_background)),
                    };
                    h_flex()
                        .px_2()
                        .when_some(background, |this, background| this.bg(background))
                        .when(kind == DiffLineKind::Context, |this| {
                            this.text_color(colors.text_muted)
                        })
                        .child(div().w(px(14.)).text_color(colors.text_muted).child(marker))
                        .child(line.to_string())
                }),
        )
        .into_any_element()
}

impl Focusable for AgentView {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.parent(cx).is_some() {
            self.focus_handle.clone()
        } else {
            self.composer.focus_handle(cx)
        }
    }
}

impl EventEmitter<AgentViewEvent> for AgentView {}

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
        rows.extend(self.render_subthread_permissions(cx));
        if let Some(generating) = self.render_generating(cx) {
            rows.push(generating);
        }
        let has_rows = !rows.is_empty();
        let is_connecting = self.thread.read(cx).status() == &ConnectionStatus::Connecting;

        let is_subthread = self.parent(cx).is_some();
        let is_drawer_full_screen =
            self.drawer_full_screen && self.is_drawer_open && self.drawer.is_some();

        v_flex()
            // Otherwise clicking the conversation would take focus from the message editor.
            .when(is_subthread, |this| this.track_focus(&self.focus_handle))
            .size_full()
            .bg(panel_background)
            .on_action(cx.listener(Self::toggle_terminal_drawer))
            .on_drag_move(
                cx.listener(|this, event: &DragMoveEvent<DraggedDrawerEdge>, _, cx| {
                    let available = event.bounds.size.height - MIN_CONVERSATION_HEIGHT;
                    let height = (event.bounds.bottom() - event.event.position.y)
                        .min(available)
                        .max(MIN_DRAWER_HEIGHT);
                    if this.drawer_height != height {
                        this.drawer_height = height;
                        cx.notify();
                    }
                }),
            )
            .when(self.shows_toolbar, |this| {
                this.child(self.render_toolbar(cx))
            })
            // A full-screen terminal hides the conversation and the composer.
            .when(!is_drawer_full_screen, |this| {
                this.children(self.render_restore_notice(cx))
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
                            .when(!has_rows && is_connecting, |this| {
                                // Zed's loading state: while the agent starts and the session (and its
                                // history) loads, not the empty-thread prompt.
                                this.child(
                                    v_flex()
                                        .flex_1()
                                        .w_full()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            Label::new("Loading…")
                                                .color(Color::Muted)
                                                .with_animation(
                                                    "loading-agent-label",
                                                    Animation::new(Duration::from_secs(2))
                                                        .repeat()
                                                        .with_easing(pulsating_between(0.3, 0.7)),
                                                    |label, delta| label.alpha(delta),
                                                ),
                                        ),
                                )
                            })
                            .when(!has_rows && !is_connecting, |this| {
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
                    .map(|this| {
                        if let Some(parent) = self.parent(cx) {
                            this.child(self.render_subthread_bar(parent, cx))
                        } else if self.is_archived {
                            this.child(self.render_archived_notice(cx))
                        } else if !self.client.read(cx).is_online() {
                            this.child(self.render_offline_notice(cx))
                        } else {
                            this.child(self.render_message_editor(cx))
                        }
                    })
            })
            .children(self.render_drawer(cx))
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

/// The dot in a toolbar button's corner that says something is behind it.
fn indicator_dot(cx: &App) -> Div {
    div()
        .absolute()
        .top(px(3.))
        .right(px(3.))
        .size_1p5()
        .rounded_full()
        .bg(Color::Accent.color(cx))
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
pub(crate) async fn open_in_terminal(
    command: &agentz_protocol::agents::AgentCommand,
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
