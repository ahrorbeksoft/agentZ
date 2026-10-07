//! The conversation with one agent. Layout, spacing and colors follow Zed's agent thread view
//! (`agent_ui::conversation_view::thread_view`).

use std::borrow::Cow;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::accounts::{AccountChoice, AccountId, AgentAccounts, LimitWindow};
use agentz_protocol::agents::AgentId;
use agentz_protocol::agents::InstallState;
use agentz_protocol::attachments::{AttachmentId, MAX_ATTACHMENT_SIZE};
use agentz_protocol::diff::DiffScope;
use agentz_protocol::terminal::{TerminalCommand, TerminalKey};
use agentz_protocol::thread::{
    ConnectionStatus, DiffLineKind, Entry, FileDiff, PlanItem, SessionRestore, ToolCall,
    without_handoff,
};
use agentz_protocol::workspace::{PastureSupport, ProjectGit, WorkspaceChoice};
use agentz_protocol::{CAPABILITY_THREAD_DIFF, PromptPart, Request, Response};
use collections::{HashMap, HashSet};
use gpui::{
    Anchor, Animation, AnimationExt as _, AnyElement, App, ClickEvent, ClipboardEntry,
    ClipboardItem, Context, DismissEvent, DragMoveEvent, Entity, EventEmitter, ExternalPaths,
    FocusHandle, Focusable, FollowMode, Hsla, ImageSource, KeyBinding, ListAlignment, ListState,
    ObjectFit, Pixels, Point, PromptLevel, Subscription, Task, Window, anchored, deferred, img,
    list, pulsating_between,
};
use markdown::{Markdown, MarkdownElement, MarkdownFont, MarkdownStyle};
use projects::{ProjectId, TaskEnd, Thread, ThreadId, UnsentMention, WorkspaceKind};
use text_input::{ChipId, ChipPreview, FittedImage, TextInput, TextInputEvent};
use ui::{
    ButtonLike, Callout, CommonAnimationExt as _, ContextMenu, ContextMenuEntry, Disclosure,
    IconPosition, PopoverMenu, PopoverMenuHandle, Severity, SpinnerLabel, SplitButton,
    SplitButtonStyle, Switch, ToggleState, Tooltip, prelude::*,
};
use util::ResultExt as _;

use crate::mention_menu::{
    Mention, MentionKind, MentionMatch, MentionQuery, MentionTarget, MentionableThread,
    find_mentions, mention_query, render_mention_menu, target_icon,
};

use crate::agent_icons::agent_icon;
use crate::agent_login::{AgentLogin, LoginLayout};
use crate::attachment_image::{
    AttachmentImage, ImagePreviewTooltip, ImageViewer, render_hover_preview,
};
use crate::confirm_dialog::ConfirmRequest;
use crate::controls::account_color;
use crate::elicitation_card::{ElicitationCard, sync_elicitation_cards};
use crate::machines::{MachineId, Machines, ProjectKey, ThreadKey};
use crate::project_info::{ProjectInfoStore, render_project_icon, workspace_icon};
use crate::project_store::ProjectStore;
use crate::project_switcher::compact_path;
use crate::registry_store::AgentRegistryStore;
use crate::server_client::{MachineStatus, ServerClient};
use crate::settings_page::{
    AccountEntry, account_entries, account_entry, account_selector, render_entry_avatar,
};
use crate::terminal_drawer::{TerminalDrawer, TerminalDrawerEvent};
use crate::terminal_element::TerminalMode;
use crate::terminal_entity::Terminal;
use crate::terminal_view::TerminalView;
use crate::thread_entity::AgentThread;
use crate::usage_limits::{
    LOW_PERCENT, UsagePopover, left_label, reset_phrase, tightest_window, used_up_window,
};
use crate::{ToggleDiff, ToggleTerminalDrawer};

pub(crate) const KEY_CONTEXT: &str = "AgentComposer";
const RENAME_KEY_CONTEXT: &str = "ThreadHeaderRename";

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
/// The most an image in a tool call's output takes, as Zed's `max_w_96` and `max_h_96`.
const TOOL_IMAGE_SIZE: gpui::Size<Pixels> = gpui::Size {
    width: px(384.),
    height: px(384.),
};

pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("enter", menu::Confirm, Some(KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(KEY_CONTEXT)),
        // Only acted on while the slash-command menu is open.
        KeyBinding::new("up", menu::SelectPrevious, Some(KEY_CONTEXT)),
        KeyBinding::new("down", menu::SelectNext, Some(KEY_CONTEXT)),
        KeyBinding::new("tab", AcceptSlashCommand, Some(KEY_CONTEXT)),
        KeyBinding::new("enter", menu::Confirm, Some(RENAME_KEY_CONTEXT)),
        KeyBinding::new("escape", menu::Cancel, Some(RENAME_KEY_CONTEXT)),
        // Zed's `use_modifier_to_send`: Cmd-Enter sends and Enter makes a new line.
        KeyBinding::new(
            "enter",
            text_input::Newline,
            Some("AgentComposer && use_modifier_to_send > TextInput"),
        ),
        KeyBinding::new(
            "secondary-enter",
            menu::Confirm,
            Some("AgentComposer && use_modifier_to_send > TextInput"),
        ),
        // While a menu is open for the composer, its keys act in the menu rather than in the
        // composer's lines.
        KeyBinding::new("up", menu::SelectPrevious, Some("TextInput && menu")),
        KeyBinding::new("down", menu::SelectNext, Some("TextInput && menu")),
        KeyBinding::new("enter", menu::Confirm, Some("TextInput && menu")),
        // Zed's key for copying what's selected in a message.
        KeyBinding::new("secondary-c", markdown::Copy, Some("Markdown")),
    ]);
    crate::attachment_image::init(cx);
}

/// Identifies one rendered piece of markdown: an entry, and which part of it.
type MarkdownKey = (usize, usize);
/// The [`MarkdownKey`] part holding a tool call's raw input.
const RAW_INPUT_PART: usize = usize::MAX;

pub enum AgentViewEvent {
    Unarchive,
    /// Show another thread: a subthread from the Agents control, or a subthread's parent.
    OpenThread(ThreadId),
    /// Ask before a destructive action, in the shell's modal layer.
    Confirm(ConfirmRequest),
    /// The header's project was clicked: a new thread in it, as in t3code.
    NewThreadInProject(ProjectId),
    /// The new thread was made again with another agent, account, checkout or machine, or as
    /// a terminal: show `thread` in its place, with `text` in its composer. This one is
    /// deleted right after.
    Replaced {
        thread: ThreadKey,
        text: SharedString,
    },
    /// Manage Agents, from the new thread's agent picker: Settings › Agents.
    OpenAgentSettings,
    /// Manage Accounts…, from the new thread's account picker: the agent's Account tab on the
    /// thread's machine, adding an account there with Add Account….
    OpenAgentAccounts {
        agent_id: AgentId,
        add_account: bool,
    },
}

/// What a new thread is made again to run, from its agent picker.
#[derive(Clone)]
enum Starter {
    Agent(AgentId),
    /// A login shell, as the thread's terminal.
    Terminal,
}

const COMPOSER_PLACEHOLDER: &str = "Message the agent…";
/// As picked in `design/composer/`: one line, growing with the text to eight, then scrolling.
const COMPOSER_MAX_LINES: usize = 8;
/// How long typing pauses before what's typed is kept on the server.
const UNSENT_TEXT_SAVE_DELAY: Duration = Duration::from_millis(500);

/// The most subthreads the Agents control lists before it scrolls.
const MAX_AGENT_ROWS_SHOWN: usize = 6;

/// An image link in a message under the mouse, whose thumbnail shows above it.
struct HoveredImage {
    key: MarkdownKey,
    id: AttachmentId,
    /// Where the mouse first came onto the link, so the preview stays put on it.
    position: Point<Pixels>,
}

fn encode_base64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// The image formats agents take, by a file's extension.
fn image_format(path: &Path) -> Option<gpui::ImageFormat> {
    let extension = path.extension()?.to_str()?.to_lowercase();
    Some(match extension.as_str() {
        "png" => gpui::ImageFormat::Png,
        "jpg" | "jpeg" => gpui::ImageFormat::Jpeg,
        "gif" => gpui::ImageFormat::Gif,
        "webp" => gpui::ImageFormat::Webp,
        _ => return None,
    })
}

pub struct AgentView {
    thread_id: ThreadId,
    /// Focused instead of the message editor on a subthread, which has none.
    focus_handle: FocusHandle,
    /// Archived threads stay readable but take no new messages until they're unarchived.
    is_archived: bool,
    /// In a workspace pane, the pane's header shows the title and the agent options, so the
    /// thread doesn't stack a second header under it. There it has no terminal drawer, as
    /// the workspace has shells beside it, and no changes, which the shell shows only beside
    /// the Agents view's thread.
    is_in_pane: bool,
    /// Whether the shell shows this thread's changes beside it.
    is_diff_open: bool,
    /// What the thread has changed, for the header's counts.
    changes: ChangeStat,
    /// The turn completion and connection `changes` were last asked for.
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
    /// The composer's right-click menu, where it was opened.
    composer_menu: Option<(Entity<ContextMenu>, Point<Pixels>, Subscription)>,
    pending_composer_menu: Option<Point<Pixels>>,
    /// The conversation: a head row, the entries, then a tail row, drawn only where visible
    /// (Zed's thread list).
    list_state: ListState,
    /// The entries' revisions the rows and markdowns were last synced with.
    synced_revisions: Vec<u64>,
    markdowns: HashMap<MarkdownKey, Entity<Markdown>>,
    /// Each markdown's row is measured again when it changes, as when it's parsed.
    _markdown_subscriptions: Vec<Subscription>,
    /// Tool calls the user opened or closed, relative to their default (edits open, others closed).
    toggled_tool_calls: HashSet<acp::ToolCallId>,
    /// Folded runs of work the user opened, by their first entry.
    opened_runs: HashSet<usize>,
    /// Whether the agent was working when the rows were last measured: a turn's last run folds
    /// as it ends.
    synced_working: bool,
    /// The running turn's folded last run and the entry its live line showed when the rows were
    /// last measured.
    synced_live_line: Option<LiveLine>,
    /// Settings › General's "Show thinking", as the rows were last measured with it.
    synced_show_thinking: bool,
    toggled_thoughts: HashSet<usize>,
    plan_expanded: bool,
    /// What each of the composer's chips mentions.
    mentions: HashMap<ChipId, Mention>,
    /// Files of this Mac being sent to another machine's thread, by their chips, which mention
    /// them once they're there.
    uploads: HashMap<ChipId, Task<()>>,
    /// The message was sent while files were still on their way, so it goes once they're there.
    send_after_uploads: bool,
    /// Why a pasted or dropped file couldn't be attached.
    attachment_error: Option<SharedString>,
    hovered_image: Option<HoveredImage>,
    /// The image to show in the viewer, which opens at the next render, where the window is.
    pending_image_viewer: Option<ImageSource>,
    image_viewer: Option<(Entity<ImageViewer>, Subscription)>,
    /// The `@query` the composer's cursor is at, while its menu is open.
    mention_query: Option<MentionQuery>,
    mention_kind: MentionKind,
    mention_matches: Vec<MentionMatch>,
    mention_index: usize,
    /// The `@` whose menu the user closed.
    mention_dismissed_at: Option<usize>,
    /// The thread folder's files, for @, once asked for.
    files: Option<agentz_protocol::FileListing>,
    files_loading: bool,
    /// The files arrived, or the kind changed, since the matches were found.
    files_changed: bool,
    _files_load: Task<()>,
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
    /// The agent's login methods, shown when it needs a login.
    login: Entity<AgentLogin>,
    /// What the agent is asking the user, by ACP's `elicitation/create`.
    elicitation_cards: Vec<Entity<ElicitationCard>>,
    /// What the composer says while empty: to log in first, while the agent needs a login.
    composer_placeholder: SharedString,
    /// The header title's menu: Rename, Continue with Another Agent, Archive, Delete.
    title_menu: PopoverMenuHandle<ContextMenu>,
    /// The header's title, while it's renamed in place.
    rename_input: Entity<TextInput>,
    renaming: bool,
    _rename_blur: Option<Subscription>,
    /// The composer's handoff chip shows what goes to the agent.
    handoff_expanded: bool,
    /// Why "Continue with another agent" didn't start a thread.
    continue_error: Option<SharedString>,
    _continuing: Task<()>,
    /// The limit notice was closed, until the next turn.
    limit_notice_dismissed: bool,
    /// The project's repository, for the new thread screen's checkout picker. Asked for the
    /// first time that screen shows.
    draft_git: Option<ProjectGit>,
    _draft_git_load: Option<Task<()>>,
    /// What's being made in place of the new thread, from its pickers.
    replacing: Option<SharedString>,
    /// Why the new thread couldn't be made again with what was picked.
    replace_error: Option<SharedString>,
    _replacing: Task<()>,
    /// What the server was last told is typed in the composer (t3code's composer draft).
    saved_unsent: UnsentDraft,
    /// The server's copy as last seen, to notice when it's discarded elsewhere.
    observed_unsent_text: Option<String>,
    _save_unsent_text: Task<()>,
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
        let composer = cx.new(|cx| {
            TextInput::new(COMPOSER_PLACEHOLDER, cx)
                .multi_line(COMPOSER_MAX_LINES)
                .handles_paste()
        });
        let rename_input = cx.new(|cx| TextInput::new("Thread title", cx));
        let login = cx
            .new(|cx| AgentLogin::new(thread.clone(), LoginLayout::Centered, agent_id.clone(), cx));
        let subscriptions = vec![
            cx.subscribe(&rename_input, |this, _, _: &TextInputEvent, cx| {
                this.apply_rename(cx)
            }),
            cx.observe(&thread, |this, thread, cx| {
                if thread.read(cx).is_working() {
                    this.limit_notice_dismissed = false;
                }
                this.sync_entries(cx);
                sync_elicitation_cards(&mut this.elicitation_cards, &thread, cx);
                this.sync_composer_placeholder(cx);
                cx.notify();
            }),
            // The agent's display name and icon come from the registry, which may load later.
            cx.observe(&registry, |this, _, cx| {
                this.sync_composer_placeholder(cx);
                cx.notify()
            }),
            // Settings › General's "Show thinking" opens or closes every thought.
            cx.observe(
                &crate::app_settings::AppSettingsStore::global(cx),
                |this, settings, cx| {
                    let show_thinking = settings.read(cx).settings().show_thinking;
                    if show_thinking != this.synced_show_thinking {
                        this.synced_show_thinking = show_thinking;
                        this.list_state.remeasure();
                        cx.notify();
                    }
                },
            ),
            cx.observe(&store, |this, _, cx| {
                this.sync_blocked_subthreads(cx);
                this.load_changed_files(false, cx);
                this.follow_discarded_unsent_text(cx);
                cx.notify();
            }),
            // Whether messages can be sent follows the machine's connection.
            cx.observe(&client, |this, _, cx| {
                this.load_changed_files(false, cx);
                cx.notify();
            }),
        ];
        // What's typed is saved as the view closes, when the user leaves the thread.
        cx.on_release(|this, cx| this.save_unsent_text_now(cx))
            .detach();
        let mut subscriptions = subscriptions;
        subscriptions.push(cx.subscribe(
            &composer,
            |this, _, event: &TextInputEvent, cx| match event {
                TextInputEvent::Changed => {
                    this.command_menu_index = 0;
                    this.forget_removed_mentions(cx);
                    this.save_unsent_text(cx);
                    cx.notify();
                }
                TextInputEvent::Paste { item, plain } => this.paste_into_composer(item, *plain, cx),
                // Built at the next render, which has the window.
                TextInputEvent::ContextMenu(position) => {
                    this.pending_composer_menu = Some(*position);
                    cx.notify();
                }
                TextInputEvent::ChipClicked(chip) => {
                    if let Some(Mention::Image(id)) = this.mentions.get(chip) {
                        let image = this.attachment_image(id.clone(), cx);
                        this.view_image(image.original(), cx);
                    }
                }
            },
        ));
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
            is_in_pane: false,
            is_diff_open: false,
            changes: ChangeStat::default(),
            changed_files_asked_for: None,
            _changed_files_load: Task::ready(()),
            client,
            store,
            registry,
            agent_id,
            composer,
            composer_menu: None,
            pending_composer_menu: None,
            list_state: {
                let list_state = ListState::new(0, ListAlignment::Top, px(2048.));
                list_state.set_follow_mode(FollowMode::Tail);
                list_state
            },
            synced_revisions: Vec::new(),
            markdowns: HashMap::default(),
            _markdown_subscriptions: Vec::new(),
            toggled_tool_calls: HashSet::default(),
            opened_runs: HashSet::default(),
            synced_working: false,
            synced_live_line: None,
            synced_show_thinking: crate::app_settings::AppSettingsStore::global(cx)
                .read(cx)
                .settings()
                .show_thinking,
            toggled_thoughts: HashSet::default(),
            plan_expanded: false,
            mentions: HashMap::default(),
            uploads: HashMap::default(),
            send_after_uploads: false,
            attachment_error: None,
            hovered_image: None,
            pending_image_viewer: None,
            image_viewer: None,
            mention_query: None,
            mention_kind: MentionKind::Any,
            mention_matches: Vec::new(),
            mention_index: 0,
            mention_dismissed_at: None,
            files: None,
            files_loading: false,
            files_changed: false,
            _files_load: Task::ready(()),
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
            login,
            elicitation_cards: Vec::new(),
            composer_placeholder: COMPOSER_PLACEHOLDER.into(),
            title_menu: PopoverMenuHandle::default(),
            rename_input,
            renaming: false,
            _rename_blur: None,
            handoff_expanded: false,
            continue_error: None,
            _continuing: Task::ready(()),
            limit_notice_dismissed: false,
            draft_git: None,
            _draft_git_load: None,
            replacing: None,
            replace_error: None,
            _replacing: Task::ready(()),
            saved_unsent: UnsentDraft::default(),
            observed_unsent_text: None,
            _save_unsent_text: Task::ready(()),
            _subscriptions: subscriptions,
            _elapsed_refresh: elapsed_refresh,
        };
        let unsent = this
            .store
            .read(cx)
            .thread(thread_id)
            .map(|thread| UnsentDraft {
                text: thread.unsent_text.clone(),
                mentions: thread.unsent_mentions.clone(),
            })
            .unwrap_or_default();
        if let Some(text) = &unsent.text {
            let prompt = unsent_prompt(text, &unsent.mentions);
            this.restore_prompt(&prompt, cx);
        }
        this.observed_unsent_text = unsent.text.clone();
        this.saved_unsent = unsent;
        this.sync_entries(cx);
        let thread = this.thread.clone();
        sync_elicitation_cards(&mut this.elicitation_cards, &thread, cx);
        this.sync_composer_placeholder(cx);
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
                                    Label::new(one_line(&request.title))
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
            .child(self.thread_agent_icon(cx).size(IconSize::Small))
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

    pub fn show_in_pane(&mut self, cx: &mut Context<Self>) {
        self.is_in_pane = true;
        self._changed_files_load = Task::ready(());
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

    /// Asks the server what the thread has changed, after each turn and on
    /// reconnecting, or now when `force`d.
    fn load_changed_files(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.is_in_pane {
            return;
        }
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
        let request = load_change_stat(client, self.thread_id);
        self._changed_files_load = cx.spawn(async move |this, cx| {
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
    /// Follows the thread's entries, redoing only those whose revision changed: their
    /// markdown, and the list's rows, which are measured again.
    fn sync_entries(&mut self, cx: &mut Context<Self>) {
        let revisions = self.thread.read(cx).entry_revisions().to_vec();
        let entry_count = revisions.len();
        let previous = self.synced_revisions.len();
        if self.list_state.item_count() == 0 {
            // The head and tail rows.
            self.list_state.splice(0..0, 2);
        }
        if entry_count > previous {
            self.list_state
                .splice(previous + 1..previous + 1, entry_count - previous);
        } else if entry_count < previous {
            self.list_state.splice(entry_count + 1..previous + 1, 0);
            self.synced_revisions.truncate(entry_count);
            self.opened_runs.clear();
        }
        let mut changed = Vec::new();
        for (index, revision) in revisions.iter().enumerate() {
            if self.synced_revisions.get(index) == Some(revision) {
                continue;
            }
            let Some(entry) = self.thread.read(cx).entries().get(index).cloned() else {
                continue;
            };
            changed.push(index);
            let is_new = self.synced_revisions.get(index).is_none();
            self.sync_entry(index, &entry, cx);
            self.list_state.remeasure_items(index + 1..index + 2);
            // A message after a run of work folds the run, also while the turn goes on.
            if is_new
                && !is_work(&entry)
                && let Some(run) = index
                    .checked_sub(1)
                    .and_then(|previous| work_run(self.thread.read(cx).entries(), previous))
            {
                self.list_state.remeasure_items(run.start + 1..run.end + 1);
            }
        }
        self.synced_revisions = revisions;
        // As a turn starts or ends, its runs of work open or fold.
        let is_working = self.thread.read(cx).is_working();
        if is_working != self.synced_working {
            self.synced_working = is_working;
            let turn_start = current_turn_start(self.thread.read(cx).entries());
            self.list_state
                .remeasure_items(turn_start + 1..entry_count + 1);
        }
        // The live line draws in its run's first row whichever entry it shows: that row follows
        // every entry of the run, and all of the run's rows change as the line moves to another
        // entry, or the run grows, folds or ends.
        let live_line = self.live_line(cx);
        if live_line != self.synced_live_line {
            for line in [self.synced_live_line.take(), live_line.clone()]
                .into_iter()
                .flatten()
            {
                let end = line.run.end.min(entry_count);
                if line.run.start < end {
                    self.list_state.remeasure_items(line.run.start + 1..end + 1);
                }
            }
        } else if let Some(line) = &live_line
            && changed.iter().any(|index| line.run.contains(index))
        {
            self.list_state
                .remeasure_items(line.run.start + 1..line.run.start + 2);
        }
        self.synced_live_line = live_line;
        // The head and tail follow the thread's state.
        self.list_state.remeasure_items(0..1);
        self.list_state
            .remeasure_items(entry_count + 1..entry_count + 2);
    }

    fn sync_entry(&mut self, index: usize, entry: &Entry, cx: &mut Context<Self>) {
        {
            match entry {
                // An agent may replay a continued thread's first message with what it brought.
                Entry::UserMessage(text) => {
                    self.sync_markdown((index, 0), without_handoff(text), cx);
                }
                Entry::AgentMessage(text) | Entry::AgentThought(text) => {
                    self.sync_markdown((index, 0), text, cx);
                }
                Entry::ToolCall(tool_call) => {
                    let is_read = matches!(tool_call.kind, acp::ToolKind::Read);
                    for (part, text) in tool_call.text.iter().enumerate() {
                        let text = if is_read {
                            as_code_block(text)
                        } else {
                            Cow::Borrowed(text.as_str())
                        };
                        self.sync_markdown((index, part + 1), &text, cx);
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
                // Streamed text only grows, so most changes are an append.
                let appended = {
                    let source = markdown.read(cx).source();
                    if source == text {
                        return;
                    }
                    text.strip_prefix(source.as_ref()).map(str::to_string)
                };
                markdown.update(cx, |markdown, cx| match appended {
                    Some(appended) => markdown.append(&appended, cx),
                    None => markdown.replace(text.to_string(), cx),
                });
            }
            None => {
                let markdown = cx.new(|cx| Markdown::new(text.to_string().into(), None, None, cx));
                // Markdown parses in the background, so the list may measure the row before
                // there's any text to show. A row measured above the view keeps that height
                // until it's drawn, and the conversation jumps as it scrolls into view.
                let row = key.0 + 1;
                self._markdown_subscriptions
                    .push(cx.observe(&markdown, move |this, _, _| {
                        this.list_state.remeasure_items(row..row + 1);
                    }));
                self.markdowns.insert(key, markdown);
            }
        }
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
        if self.mention_query.is_some() {
            let count = self.mention_matches.len().max(1);
            self.mention_index = (self.mention_index + 1) % count;
            cx.notify();
            return;
        }
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
        if self.mention_query.is_some() {
            let count = self.mention_matches.len().max(1);
            self.mention_index = self.mention_index.checked_sub(1).unwrap_or(count - 1);
            cx.notify();
            return;
        }
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
        if self.mention_query.is_some() && !self.mention_matches.is_empty() {
            self.accept_mention(self.mention_index, cx);
            return;
        }
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

    /// Whether the agent needs a login before it takes messages.
    fn needs_login(&self, cx: &App) -> bool {
        self.thread.read(cx).status() == &ConnectionStatus::AuthRequired
    }

    fn sync_composer_placeholder(&mut self, cx: &mut Context<Self>) {
        let placeholder: SharedString = if self.needs_login(cx) {
            format!("Log in to {} to send a message", self.agent_name(cx)).into()
        } else {
            COMPOSER_PLACEHOLDER.into()
        };
        if placeholder != self.composer_placeholder {
            self.composer_placeholder = placeholder.clone();
            self.composer
                .update(cx, |composer, cx| composer.set_placeholder(placeholder, cx));
        }
    }

    fn send(&mut self, _: &menu::Confirm, _: &mut Window, cx: &mut Context<Self>) {
        if self.mention_query.is_some() && !self.mention_matches.is_empty() {
            self.accept_mention(self.mention_index, cx);
            return;
        }
        self.send_message(cx);
    }

    fn send_message(&mut self, cx: &mut Context<Self>) {
        if self.is_archived || !self.client.read(cx).is_online() || self.needs_login(cx) {
            return;
        }
        // As Zed waits for its mentions to load: the message goes once its files are there.
        if !self.uploads.is_empty() {
            self.send_after_uploads = true;
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
        let text = self.composer.read(cx).plain_text();
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
        let prompt = self.composer_prompt(cx);
        self.composer
            .update(cx, |composer, cx| composer.set_text("", cx));
        self.mentions.clear();
        self.attachment_error = None;
        // Messages typed while the agent works wait in the queue the server keeps, which
        // sends them one at a time as each turn ends, like Zed's.
        let thread = self.thread.read(cx);
        if thread.is_working() || !thread.state.queued_messages.is_empty() {
            self.thread
                .update(cx, |thread, cx| thread.queue_message(prompt, cx));
            cx.notify();
            return;
        }
        self.list_state.scroll_to_end();
        self.thread.update(cx, |thread, cx| thread.send(prompt, cx));
    }

    fn stop(&mut self, _: &menu::Cancel, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(query) = &self.mention_query {
            self.mention_dismissed_at = Some(query.at);
            self.mention_query = None;
            cx.notify();
            return;
        }
        if !self.matching_commands(cx).is_empty() {
            self.command_menu_dismissed_for = Some(self.composer.read(cx).text().clone());
            cx.notify();
            return;
        }
        self.thread.update(cx, |thread, cx| thread.cancel(cx));
    }

    /// Zed's paste: images become Image chips and copied files mentions, unless pasted as
    /// plain text.
    fn paste_into_composer(&mut self, item: &ClipboardItem, plain: bool, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = item
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                ClipboardEntry::ExternalPaths(paths) => Some(paths.paths().to_vec()),
                _ => None,
            })
            .flatten()
            .collect();
        let images: Vec<gpui::Image> = item
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                ClipboardEntry::Image(image) => Some(image.clone()),
                _ => None,
            })
            .collect();
        if plain || (paths.is_empty() && images.is_empty()) {
            if let Some(text) = item.text() {
                self.composer
                    .update(cx, |composer, cx| composer.insert(&text, cx));
            }
            return;
        }
        for path in paths {
            self.mention_path(path, cx);
        }
        if self.thread.read(cx).supports_images() {
            for image in images {
                self.insert_image(image.format, image.bytes, cx);
            }
        }
    }

    /// A pasted or dropped file (one copied in Finder too): an image becomes an Image chip,
    /// anything else a mention of it. Another machine's thread can't see this Mac's files, so a
    /// file goes to that machine first. A folder can't, so it goes as its path.
    fn mention_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(format) = image_format(&path)
            && self.thread.read(cx).supports_images()
        {
            match std::fs::read(&path) {
                Ok(bytes) => self.insert_image(format, bytes, cx),
                Err(error) => {
                    self.attachment_failed(format!("Couldn't read {}: {error}", path.display()), cx)
                }
            }
            return;
        }
        let label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let is_dir = path.is_dir();
        if self.client.read(cx).machine() != MachineId::Local {
            if is_dir {
                let text = format!("{} ", path.display());
                self.composer
                    .update(cx, |composer, cx| composer.insert(&text, cx));
            } else {
                self.upload_file(path, label, cx);
            }
            return;
        }
        let icon = if is_dir {
            IconName::Folder
        } else {
            IconName::File
        };
        let preview = ChipPreview::Text(compact_path(&path).into());
        self.insert_mention(None, &label, icon, preview, Mention::Path(path), cx);
    }

    /// A pasted or dropped image, which the thread's server keeps for its messages. Its chip
    /// shows this Mac's copy at once, and mentions the server's once it's there.
    fn insert_image(&mut self, format: gpui::ImageFormat, bytes: Vec<u8>, cx: &mut Context<Self>) {
        let mime_type = format.mime_type();
        if !agentz_protocol::attachments::is_image_type(mime_type) {
            self.attachment_failed(format!("{mime_type} images can't be attached."), cx);
            return;
        }
        if bytes.len() > MAX_ATTACHMENT_SIZE {
            self.attachment_failed(
                format!("The image is larger than {} MB.", MAX_ATTACHMENT_SIZE >> 20),
                cx,
            );
            return;
        }
        let image = Arc::new(gpui::Image::from_bytes(format, bytes));
        let preview = ChipPreview::Image(ImageSource::Image(image.clone()));
        let chip = self.insert_pending_chip("Image", IconName::Image, preview, cx);
        let client = self.client.clone();
        let thread_id = self.thread_id;
        let upload = cx.spawn(async move |this, cx| {
            let result = async {
                let data = cx
                    .background_spawn(async move { encode_base64(&image.bytes) })
                    .await;
                // Not `send`, which would write the whole request into its log line.
                let response = client
                    .read_with(cx, |client, _| {
                        client.request(Request::AddAttachment {
                            thread_id,
                            mime_type: mime_type.to_string(),
                            data,
                        })
                    })
                    .await?;
                match response {
                    Response::Attachment(id) => Ok(Mention::Image(id)),
                    response => Err(anyhow::anyhow!("unexpected response: {response:?}")),
                }
            }
            .await;
            this.update(cx, |this, cx| {
                this.upload_finished(chip, result, "Couldn't attach the image", cx)
            })
            .log_err();
        });
        self.uploads.insert(chip, upload);
    }

    /// Sends a file of this Mac to the thread's machine, and mentions where it's kept there.
    fn upload_file(&mut self, path: PathBuf, label: String, cx: &mut Context<Self>) {
        let preview = ChipPreview::Text(compact_path(&path).into());
        let chip = self.insert_pending_chip(&label, IconName::File, preview, cx);
        let client = self.client.clone();
        let thread_id = self.thread_id;
        let upload = cx.spawn(async move |this, cx| {
            let result = async {
                let data = cx
                    .background_spawn({
                        let path = path.clone();
                        async move {
                            let bytes = std::fs::read(&path)?;
                            anyhow::ensure!(
                                bytes.len() <= MAX_ATTACHMENT_SIZE,
                                "it's larger than {} MB",
                                MAX_ATTACHMENT_SIZE >> 20
                            );
                            Ok(encode_base64(&bytes))
                        }
                    })
                    .await?;
                let response = client
                    .read_with(cx, |client, _| {
                        client.request(Request::UploadFile {
                            thread_id,
                            name: label,
                            data,
                        })
                    })
                    .await?;
                match response {
                    Response::UploadedFile(path) => Ok(Mention::Path(path)),
                    response => Err(anyhow::anyhow!("unexpected response: {response:?}")),
                }
            }
            .await;
            this.update(cx, |this, cx| {
                let message = format!("Couldn't send {} to the thread's machine", path.display());
                this.upload_finished(chip, result, &message, cx)
            })
            .log_err();
        });
        self.uploads.insert(chip, upload);
    }

    /// A chip whose mention follows once what it mentions is uploaded.
    fn insert_pending_chip(
        &mut self,
        label: &str,
        icon: IconName,
        preview: ChipPreview,
        cx: &mut Context<Self>,
    ) -> ChipId {
        let copy_text: SharedString = format!("@{label}").into();
        let chip = self.composer.update(cx, |composer, cx| {
            composer.insert_chip(None, label, icon.path().into(), preview, copy_text, cx)
        });
        self.mention_kind = MentionKind::Any;
        cx.notify();
        chip
    }

    fn upload_finished(
        &mut self,
        chip: ChipId,
        result: anyhow::Result<Mention>,
        failure: &str,
        cx: &mut Context<Self>,
    ) {
        // Its chip was deleted meanwhile.
        if self.uploads.remove(&chip).is_none() {
            return;
        }
        match result {
            Ok(mention) => {
                self.mentions.insert(chip, mention);
                self.save_unsent_text(cx);
                if self.send_after_uploads && self.uploads.is_empty() {
                    self.send_after_uploads = false;
                    self.send_message(cx);
                }
            }
            Err(error) => {
                self.send_after_uploads = false;
                self.composer
                    .update(cx, |composer, cx| composer.remove_chip(chip, cx));
                self.attachment_failed(format!("{failure}: {error:#}"), cx);
            }
        }
        cx.notify();
    }

    fn attachment_failed(&mut self, error: String, cx: &mut Context<Self>) {
        log::error!("{error}");
        self.attachment_error = Some(error.into());
        cx.notify();
    }

    /// Puts a mention's chip in place of `range` (or the selection).
    fn insert_mention(
        &mut self,
        range: Option<std::ops::Range<usize>>,
        label: &str,
        icon: IconName,
        preview: ChipPreview,
        mention: Mention,
        cx: &mut Context<Self>,
    ) {
        let copy_text: SharedString = format!("@{label}").into();
        let chip = self.composer.update(cx, |composer, cx| {
            composer.insert_chip(range, label, icon.path().into(), preview, copy_text, cx)
        });
        self.mentions.insert(chip, mention);
        self.mention_kind = MentionKind::Any;
        cx.notify();
    }

    /// Forgets what the chips no longer in the composer mentioned.
    fn forget_removed_mentions(&mut self, cx: &App) {
        let chips: HashSet<ChipId> = self
            .composer
            .read(cx)
            .chips()
            .iter()
            .map(|chip| chip.id)
            .collect();
        self.mentions.retain(|chip, _| chips.contains(chip));
        self.uploads.retain(|chip, _| chips.contains(chip));
    }

    /// The composer's message: its text, and what its chips mention, in order.
    fn composer_prompt(&self, cx: &App) -> Vec<PromptPart> {
        let composer = self.composer.read(cx);
        let text = composer.text();
        let mut prompt = Vec::new();
        let mut index = 0;
        for chip in composer.chips() {
            if chip.range.start > index {
                prompt.push(PromptPart::Text(text[index..chip.range.start].to_string()));
            }
            prompt.push(match self.mentions.get(&chip.id) {
                Some(mention) => mention.prompt_part(),
                None => PromptPart::Text(chip.copy_text.to_string()),
            });
            index = chip.range.end;
        }
        if index < text.len() {
            prompt.push(PromptPart::Text(text[index..].to_string()));
        }
        prompt
    }

    /// Follows the `@query` at the composer's cursor, finding what it mentions.
    fn sync_mention_query(&mut self, cx: &mut Context<Self>) {
        let composer = self.composer.read(cx);
        let query = mention_query(composer.text(), composer.cursor_offset())
            .filter(|query| Some(query.at) != self.mention_dismissed_at);
        if query.is_none() {
            if self.mention_query.take().is_some() {
                self.mention_kind = MentionKind::Any;
            }
            if self
                .mention_dismissed_at
                .is_some_and(|at| composer.text().get(at..at + 1) != Some("@"))
            {
                self.mention_dismissed_at = None;
            }
            return;
        }
        if query == self.mention_query && !self.files_changed {
            return;
        }
        self.files_changed = false;
        if self.mention_query.as_ref().map(|query| query.at) != query.as_ref().map(|query| query.at)
            || self.mention_query.as_ref().map(|query| &query.query)
                != query.as_ref().map(|query| &query.query)
        {
            self.mention_index = 0;
        }
        self.mention_query = query;
        if self.files.is_none() && !self.files_loading {
            self.load_files(cx);
        }
        let threads = self.mentionable_threads(cx);
        let query = self
            .mention_query
            .as_ref()
            .map(|query| query.query.clone())
            .unwrap_or_default();
        self.mention_matches =
            find_mentions(&query, self.mention_kind, self.files.as_ref(), &threads);
    }

    fn load_files(&mut self, cx: &mut Context<Self>) {
        self.files_loading = true;
        let request = self
            .client
            .read(cx)
            .request(Request::ListFiles(self.thread_id));
        self._files_load = cx.spawn(async move |this, cx| {
            let listing = match request.await {
                Ok(Response::Files(listing)) => Some(listing),
                Ok(response) => {
                    log::error!("unexpected answer to listing files: {response:?}");
                    None
                }
                Err(error) => {
                    log::error!("failed to list the thread's files: {error:#}");
                    None
                }
            };
            this.update(cx, |this, cx| {
                this.files_loading = false;
                this.files = listing.or(Some(agentz_protocol::FileListing::default()));
                this.files_changed = true;
                cx.notify();
            })
            .log_err();
        });
    }

    /// The project's other agent threads, the latest first.
    fn mentionable_threads(&self, cx: &App) -> Vec<MentionableThread> {
        let store = self.store.read(cx);
        let Some(project_id) = store.thread(self.thread_id).map(|thread| thread.project_id) else {
            return Vec::new();
        };
        let mut threads: Vec<&Thread> = store
            .threads()
            .iter()
            .filter(|thread| {
                thread.project_id == project_id
                    && thread.id != self.thread_id
                    && thread.terminal.is_none()
                    && !thread.is_draft
                    && thread.archived_at.is_none()
            })
            .collect();
        threads.sort_by_key(|thread| std::cmp::Reverse(thread.last_activity_at));
        let now = std::time::SystemTime::now();
        threads
            .into_iter()
            .map(|thread| MentionableThread {
                id: thread.id,
                title: thread.title.clone().into(),
                time: thread
                    .last_activity_at
                    .map(|time| crate::sidebar::format_relative_time(time, now))
                    .unwrap_or_default()
                    .into(),
            })
            .collect()
    }

    fn accept_mention(&mut self, index: usize, cx: &mut Context<Self>) {
        let (Some(found), Some(query)) = (
            self.mention_matches.get(index).cloned(),
            self.mention_query.clone(),
        ) else {
            return;
        };
        let cursor = self.composer.read(cx).cursor_offset();
        let icon = target_icon(&found.target);
        let (mention, preview) = match &found.target {
            MentionTarget::Path { path, .. } => {
                let root = self
                    .files
                    .as_ref()
                    .map(|files| files.root.clone())
                    .unwrap_or_default();
                let path = root.join(path);
                let preview = ChipPreview::Text(compact_path(&path).into());
                (Mention::Path(path), preview)
            }
            MentionTarget::Thread { id, title } => {
                (Mention::Thread(*id), ChipPreview::Text(title.clone()))
            }
        };
        self.mention_query = None;
        self.insert_mention(
            Some(query.at..cursor),
            &found.label,
            icon,
            preview,
            mention,
            cx,
        );
    }

    /// The + button's kinds: types `@`, narrowed to files or threads.
    fn start_mention(&mut self, kind: MentionKind, window: &mut Window, cx: &mut Context<Self>) {
        let composer = self.composer.read(cx);
        let cursor = composer.cursor_offset();
        let needs_space = composer
            .text()
            .get(..cursor)
            .and_then(|before| before.chars().next_back())
            .is_some_and(|character| !character.is_whitespace());
        let text = if needs_space { " @" } else { "@" };
        self.mention_dismissed_at = None;
        self.composer
            .update(cx, |composer, cx| composer.insert(text, cx));
        self.mention_kind = kind;
        self.files_changed = true;
        window.focus(&self.composer.focus_handle(cx), cx);
        cx.notify();
    }

    /// The + button's Image: picks image files to attach.
    fn pick_images(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Add Image".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            this.update_in(cx, |this, window, cx| {
                for path in paths {
                    if image_format(&path).is_some() {
                        this.mention_path(path, cx);
                    }
                }
                window.focus(&this.composer.focus_handle(cx), cx);
            })
            .log_err();
        })
        .detach();
    }

    /// Zed's Add Context button and menu.
    fn render_add_context_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        let supports_images = self.thread.read(cx).supports_images();
        PopoverMenu::new("add-context-menu")
            .trigger_with_tooltip(
                IconButton::new("add-context", IconName::Plus)
                    .icon_size(IconSize::Small)
                    .icon_color(Color::Muted),
                Tooltip::text("Add Context"),
            )
            .anchor(Anchor::BottomLeft)
            .offset(gpui::Point {
                x: px(0.0),
                y: px(-2.0),
            })
            .menu(move |window, cx| {
                let this = this.clone();
                Some(ContextMenu::build(window, cx, move |menu, _, _| {
                    let kind_entry = |label: &'static str, icon: IconName, kind: MentionKind| {
                        let this = this.clone();
                        ContextMenuEntry::new(label)
                            .icon(icon)
                            .icon_color(Color::Muted)
                            .icon_size(IconSize::XSmall)
                            .handler(move |window, cx| {
                                this.update(cx, |this, cx| this.start_mention(kind, window, cx))
                                    .ok();
                            })
                    };
                    let images = this.clone();
                    menu.item(kind_entry(
                        "Files & Directories",
                        IconName::File,
                        MentionKind::Files,
                    ))
                    .item(kind_entry(
                        "Threads",
                        IconName::Thread,
                        MentionKind::Threads,
                    ))
                    .item(
                        ContextMenuEntry::new("Image")
                            .icon(IconName::Image)
                            .icon_color(Color::Muted)
                            .icon_size(IconSize::XSmall)
                            .disabled(!supports_images)
                            .handler(move |window, cx| {
                                images
                                    .update(cx, |this, cx| this.pick_images(window, cx))
                                    .ok();
                            }),
                    )
                }))
            })
    }

    /// Zed's menu for its message editor.
    fn deploy_composer_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus_handle = self.composer.focus_handle(cx);
        let has_selection = self.composer.read(cx).has_selection();
        let menu = ContextMenu::build(window, cx, move |menu, _, _| {
            menu.context(focus_handle)
                .action("Cut", Box::new(text_input::Cut))
                .action_disabled_when(!has_selection, "Copy", Box::new(text_input::Copy))
                .action("Paste", Box::new(text_input::Paste))
                .action("Paste as Plain Text", Box::new(text_input::PasteRaw))
        });
        let subscription =
            cx.subscribe_in(&menu, window, |this, _, _: &DismissEvent, window, cx| {
                this.composer_menu = None;
                window.focus(&this.composer.focus_handle(cx), cx);
                cx.notify();
            });
        window.focus(&menu.focus_handle(cx), cx);
        self.composer_menu = Some((menu, position, subscription));
        cx.notify();
    }

    fn render_composer_menu(&self) -> Option<AnyElement> {
        let (menu, position, _) = self.composer_menu.as_ref()?;
        Some(
            deferred(
                anchored()
                    .position(*position)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(menu.clone()),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// One of the conversation list's rows: the head (where the thread came from), an entry,
    /// or the tail (what follows the entries).
    fn render_conversation_row(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entry_count = self.thread.read(cx).entries().len();
        let content = if index == 0 {
            let continued_from = (entry_count > 0)
                .then(|| self.render_continued_from(cx))
                .flatten();
            div().pt_2().children(continued_from).into_any_element()
        } else if index <= entry_count {
            let Some(entry) = self.thread.read(cx).entries().get(index - 1).cloned() else {
                return div().into_any_element();
            };
            self.render_entry(index - 1, &entry, index == entry_count, window, cx)
        } else {
            let needs_login = self.needs_login(cx);
            v_flex()
                .w_full()
                .pb_4()
                .children(self.render_tail_rows(cx))
                .when(needs_login, |this| {
                    this.child(
                        v_flex()
                            .debug_selector(|| "thread-login".into())
                            .w_full()
                            .px_5()
                            .py_8()
                            .items_center()
                            .child(self.login.clone()),
                    )
                })
                .into_any_element()
        };
        div()
            .debug_selector(|| format!("conversation-row-{index}"))
            .w_full()
            .flex()
            .justify_center()
            .child(div().w_full().max_w(MAX_CONTENT_WIDTH).child(content))
            .into_any_element()
    }

    /// What follows the entries: where the thread went on, permission requests without a tool
    /// call or from subthreads, requests for input, and the working indicator.
    fn render_tail_rows(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let entry_count = self.thread.read(cx).entries().len();
        let mut rows = self.render_continuations(cx);
        let orphans: Vec<(acp::ToolCallId, String)> = self
            .thread
            .read(cx)
            .orphan_permission_requests()
            .map(|request| (request.tool_call_id.clone(), request.title.clone()))
            .collect();
        for (offset, (tool_call_id, title)) in orphans.iter().enumerate() {
            if let Some(element) =
                self.render_orphan_permission(entry_count + offset, tool_call_id, title, cx)
            {
                rows.push(element);
            }
        }
        rows.extend(self.render_subthread_permissions(cx));
        rows.extend(
            self.elicitation_cards
                .iter()
                .filter(|card| !card.read(cx).is_for_request())
                .map(|card| div().px_5().py_1p5().child(card.clone()).into_any_element()),
        );
        if let Some(generating) = self.render_generating(cx) {
            rows.push(generating);
        }
        rows
    }

    fn render_mention_menu(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        self.mention_query.as_ref()?;
        let this = cx.entity().downgrade();
        Some(render_mention_menu(
            &self.mention_matches,
            self.mention_index,
            self.files_loading,
            move |index, _, cx| {
                this.update(cx, |this, cx| this.accept_mention(index, cx))
                    .ok();
            },
            cx,
        ))
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
        let icon = self
            .agent_id
            .as_ref()
            .and_then(|agent_id| agent_icon(agent_id, cx));
        match icon {
            Some(markup) => Icon::from_svg_markup(markup),
            None => Icon::new(IconName::Terminal),
        }
    }

    /// The agent's icon along a conversation: muted, or in the thread's account's color. The
    /// new thread screen shows the account in the strip under its composer instead.
    fn thread_agent_icon(&self, cx: &App) -> Icon {
        let account_color = self.agent_id.as_ref().and_then(|agent_id| {
            let account = self.store.read(cx).thread(self.thread_id)?.account;
            self.account_icon_color(agent_id, account, cx)
        });
        self.agent_icon(cx)
            .color(account_color.unwrap_or(Color::Muted))
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

    /// t3code's breadcrumb: the project, where a click starts a new thread, then the title,
    /// which opens the thread's menu (a double-click renames it in place). The branch and the
    /// thread's buttons follow.
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let store = self.store.read(cx);
        let project = store
            .thread(self.thread_id)
            .and_then(|thread| store.project(thread.project_id))
            .cloned();
        // A Workspaces thread has no project: its folder stands in.
        let folder = project
            .is_none()
            .then(|| store.thread_folder(self.thread_id))
            .flatten();
        let title = if self.renaming {
            self.render_rename_input(cx)
        } else {
            self.render_title_menu(cx)
        };
        h_flex()
            .h(TOOLBAR_HEIGHT)
            .flex_none()
            .px_2()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .children(project.map(|project| self.render_project_crumb(&project, cx)))
            .children(folder.map(render_folder_crumb))
            // From the title's own width, so a narrow header shrinks it and the project's name
            // together.
            .child(h_flex().flex_auto().min_w_0().child(title))
            .children(self.render_branch(cx))
            .child(self.render_toolbar_buttons(cx))
    }

    fn render_project_crumb(
        &self,
        project: &projects::Project,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let project_id = project.id;
        let name = project.name();
        let new_thread = cx.listener(move |_, _: &ClickEvent, _, cx| {
            cx.emit(AgentViewEvent::NewThreadInProject(project_id))
        });
        let machine = self.store.read(cx).machine();
        let info_store = ProjectInfoStore::global(cx);
        let icon = render_project_icon(
            project,
            info_store.read(cx).info(machine, project_id),
            px(14.),
            cx,
        );
        let hover = cx.theme().colors().ghost_element_hover;
        // Shrinks with the title when the header is narrow, so neither takes all the room.
        h_flex()
            .min_w_0()
            .gap_1()
            .child(
                h_flex()
                    .id("thread-header-project")
                    .debug_selector(|| "thread-header-project".into())
                    .min_w_0()
                    .gap_1p5()
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .child(div().flex_none().child(icon))
                    .child(
                        div().min_w_0().max_w(px(160.)).child(
                            Label::new(name.clone())
                                .size(LabelSize::Small)
                                .color(Color::Muted)
                                .truncate(),
                        ),
                    )
                    .tooltip(Tooltip::text(format!("New Thread in {name}")))
                    .on_click(new_thread),
            )
            .child(Label::new("/").size(LabelSize::Small).color(Color::Muted))
            .into_any_element()
    }

    /// The title as the trigger of the thread's menu: Rename, Continue with Another Agent,
    /// Archive and Delete. A Workspaces thread isn't archived.
    fn render_title_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.weak_entity();
        let thread_id = self.thread_id;
        let store = self.store.clone();
        let client = self.client.clone();
        let registry = self.registry.clone();
        let current_agent = self.agent_id.clone();
        let is_archived = self.is_archived;
        let in_workspaces = self
            .store
            .read(cx)
            .thread(thread_id)
            .is_some_and(Thread::in_workspaces);
        // A draft has no conversation to continue, and its agent picker changes the agent.
        let is_draft = self.is_draft(cx);
        let title = self.title.clone();
        let rename = {
            let view = view.clone();
            Rc::new(move |window: &mut Window, cx: &mut App| {
                view.update(cx, |view, cx| view.start_rename(window, cx))
                    .log_err();
            })
        };
        let rename_from_menu = rename.clone();
        PopoverMenu::new("thread-title-menu")
            .with_handle(self.title_menu.clone())
            // As wide as the header leaves it, so a long title truncates rather than running
            // over the branch and buttons.
            .full_width(true)
            .menu(move |window, cx| {
                let agents: Vec<(AgentId, SharedString)> = {
                    let registry = registry.read(cx);
                    registry
                        .agents()
                        .iter()
                        .filter(|agent| {
                            Some(agent.id()) != current_agent.as_ref()
                                && agent.supports_current_platform()
                                && matches!(
                                    registry.install_state(agent.id()),
                                    InstallState::Installed { .. }
                                )
                        })
                        .map(|agent| (agent.id().clone(), agent.name().clone()))
                        .collect()
                };
                // Accounts share no sessions, so going on with another of the agent's accounts
                // is continuing too.
                let own_accounts = current_agent.as_ref().and_then(|agent_id| {
                    let entries = account_entries(&client.read(cx).accounts(agent_id));
                    if entries.len() < 2 {
                        return None;
                    }
                    let name = registry
                        .read(cx)
                        .agent(agent_id)
                        .map_or_else(|| agent_id.0.clone(), |agent| agent.name().clone());
                    Some((agent_id.clone(), name, entries))
                });
                let thread_account = store
                    .read(cx)
                    .thread(thread_id)
                    .map(|thread| thread.account);
                let now = SystemTime::now();
                let view = view.clone();
                let store = store.clone();
                let title = title.clone();
                let rename = rename_from_menu.clone();
                // Drafts, Workspaces threads and agents' subthreads aren't pinned.
                let is_pinned = store
                    .read(cx)
                    .thread(thread_id)
                    .filter(|thread| thread.can_pin())
                    .map(Thread::is_pinned);
                Some(ContextMenu::build(window, cx, move |menu, _, _| {
                    let rename = rename.clone();
                    let toggle_pinned = {
                        let store = store.clone();
                        move |is_pinned: bool| {
                            let store = store.clone();
                            move |_: &mut Window, cx: &mut App| {
                                if is_pinned {
                                    store.update(cx, |store, cx| store.unpin_thread(thread_id, cx));
                                } else {
                                    let key = ThreadKey {
                                        machine: store.read(cx).machine(),
                                        thread: thread_id,
                                    };
                                    Machines::pin_thread(&Machines::global(cx), key, cx);
                                }
                            }
                        }
                    };
                    let continue_with = {
                        let view = view.clone();
                        let agents = agents.clone();
                        let own_accounts = own_accounts.clone();
                        move |mut menu: ContextMenu,
                              _: &mut Window,
                              _: &mut Context<ContextMenu>| {
                            if agents.is_empty() && own_accounts.is_none() {
                                return menu.label("No other agents are installed");
                            }
                            // The thread's agent first, its accounts beneath it.
                            if let Some((agent_id, name, entries)) = &own_accounts {
                                let render_id = agent_id.clone();
                                let name = name.clone();
                                menu = menu.custom_row(move |_, cx| {
                                    render_agent_item(&render_id, name.clone(), cx)
                                });
                                for entry in entries {
                                    let account = entry.account;
                                    let is_own = thread_account == Some(account);
                                    let entry = entry.clone();
                                    let render = move |_: &mut Window, cx: &mut App| {
                                        let selector = account_selector(entry.account);
                                        account_row(
                                            &entry,
                                            is_own.then(|| "This thread's account".into()),
                                            is_own,
                                            now,
                                            cx,
                                        )
                                        .debug_selector(move || {
                                            format!("continue-on-account-{selector}")
                                        })
                                        .pl(px(18.))
                                        .child(div().flex_none().w(px(14.)))
                                        .into_any_element()
                                    };
                                    if is_own {
                                        menu = menu.custom_row(render);
                                        continue;
                                    }
                                    let view = view.clone();
                                    let agent_id = agent_id.clone();
                                    menu = menu.custom_entry(render, move |_, cx| {
                                        let agent_id = agent_id.clone();
                                        view.update(cx, |view, cx| {
                                            view.continue_with(
                                                agent_id,
                                                AccountChoice::of(account),
                                                cx,
                                            )
                                        })
                                        .log_err();
                                    });
                                }
                            }
                            for (agent_id, name) in &agents {
                                let view = view.clone();
                                let agent_id = agent_id.clone();
                                let render_id = agent_id.clone();
                                let name = name.clone();
                                menu = menu.custom_entry(
                                    move |_, cx| render_agent_item(&render_id, name.clone(), cx),
                                    move |_, cx| {
                                        let agent_id = agent_id.clone();
                                        view.update(cx, |view, cx| {
                                            view.continue_with(agent_id, AccountChoice::Default, cx)
                                        })
                                        .log_err();
                                    },
                                );
                            }
                            menu
                        }
                    };
                    let archive = {
                        let store = store.clone();
                        move |_: &mut Window, cx: &mut App| {
                            store.update(cx, |store, cx| {
                                if is_archived {
                                    store.unarchive_thread(thread_id, cx)
                                } else {
                                    store.archive_thread(thread_id, cx)
                                }
                            })
                        }
                    };
                    let delete = {
                        let store = store.clone();
                        let title = title.clone();
                        move |window: &mut Window, cx: &mut App| {
                            confirm_delete_thread(&store, thread_id, &title, window, cx)
                        }
                    };
                    let mut menu = menu
                        .when_some(is_pinned, |menu, is_pinned| {
                            menu.item(
                                ContextMenuEntry::new(if is_pinned { "Unpin" } else { "Pin" })
                                    .icon(if is_pinned {
                                        IconName::Unpin
                                    } else {
                                        IconName::Pin
                                    })
                                    .icon_color(Color::Muted)
                                    .handler(toggle_pinned(is_pinned)),
                            )
                        })
                        .item(
                            ContextMenuEntry::new("Rename")
                                .icon(IconName::Pencil)
                                .icon_color(Color::Muted)
                                .handler(move |window, cx| rename(window, cx)),
                        );
                    if !is_draft {
                        menu = menu.submenu_with_icon(
                            "Continue with Another Agent",
                            IconName::ArrowRight,
                            continue_with,
                        );
                    }
                    menu.separator()
                        .when(!in_workspaces, |menu| {
                            menu.item(
                                ContextMenuEntry::new(if is_archived {
                                    "Unarchive"
                                } else {
                                    "Archive"
                                })
                                .icon(if is_archived {
                                    IconName::Undo
                                } else {
                                    IconName::Archive
                                })
                                .icon_color(Color::Muted)
                                .handler(archive),
                            )
                        })
                        .item(
                            ContextMenuEntry::new("Delete…")
                                .icon(IconName::Trash)
                                .icon_color(Color::Muted)
                                .handler(delete),
                        )
                }))
            })
            .trigger(TitleButton::new(self.title.clone(), move |window, cx| {
                rename(window, cx)
            }))
            .anchor(gpui::Anchor::TopLeft)
            .offset(gpui::point(px(0.), px(4.)))
            .into_any_element()
    }

    fn render_rename_input(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        div()
            .flex_1()
            .min_w_0()
            .max_w(px(480.))
            .key_context(RENAME_KEY_CONTEXT)
            .on_action(
                cx.listener(|this, _: &menu::Confirm, window, cx| this.end_rename(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &menu::Cancel, window, cx| this.end_rename(window, cx)),
            )
            .px_1()
            .rounded_sm()
            .border_1()
            .border_color(colors.border_focused)
            .text_ui_sm(cx)
            .child(self.rename_input.clone())
            .into_any_element()
    }

    /// The branch the thread works on, the icon of its worktree or pasture (a branch's in the
    /// project's own folder), and its folder.
    fn thread_branch(&self, cx: &App) -> Option<(SharedString, IconName, PathBuf)> {
        let store = self.store.read(cx);
        let folder = store.thread_folder(self.thread_id)?;
        let workspace = store.thread_workspace(self.thread_id).cloned();
        let branch = store
            .git_head(&folder)
            .map(|head| head.branch.clone())
            .or_else(|| workspace.as_ref()?.branch.clone())?;
        let icon = workspace.as_ref().map_or(IconName::GitBranch, |workspace| {
            workspace_icon(workspace.kind)
        });
        Some((branch.into(), icon, folder))
    }

    /// The branch the thread works on, marked as a worktree or pasture's.
    fn render_branch(&self, cx: &App) -> Option<AnyElement> {
        let (branch, icon, folder) = self.thread_branch(cx)?;
        Some(
            h_flex()
                .id("thread-header-branch")
                .flex_none()
                .max_w(px(200.))
                .h(px(22.))
                .px_1p5()
                .gap_1()
                .rounded_sm()
                .border_1()
                .border_color(cx.theme().colors().border_variant)
                .child(Icon::new(icon).size(IconSize::XSmall).color(Color::Muted))
                .child(
                    Label::new(branch)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                        .truncate(),
                )
                .tooltip(Tooltip::text(folder.display().to_string()))
                .into_any_element(),
        )
    }

    pub(crate) fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The double-click's first click opened the menu.
        self.title_menu.hide(cx);
        // Cleared while the text is set, so that change doesn't count as a rename.
        self.renaming = false;
        let title = self.title.clone();
        self.rename_input.update(cx, |input, cx| {
            input.set_text(title, cx);
            input.select_all_text(cx);
        });
        self.renaming = true;
        cx.notify();
        // A menu takes focus two frames after it opens, and gives it back as it closes: the
        // field takes it after both.
        let view = cx.weak_entity();
        window.on_next_frame(move |window, _| {
            window.on_next_frame(move |window, _| {
                window.on_next_frame(move |window, cx| {
                    view.update(cx, |view, cx| view.focus_rename(window, cx))
                        .log_err();
                });
            });
        });
    }

    fn focus_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.renaming {
            return;
        }
        let focus_handle = self.rename_input.focus_handle(cx);
        window.focus(&focus_handle, cx);
        // Clicking elsewhere ends it, as in the sidebar.
        self._rename_blur = Some(cx.on_blur(&focus_handle, window, |this, _, cx| {
            this.renaming = false;
            this._rename_blur = None;
            cx.notify();
        }));
    }

    /// Renames as you type, as the sidebar does; an empty title goes back to the automatic one.
    fn apply_rename(&mut self, cx: &mut Context<Self>) {
        if !self.renaming {
            return;
        }
        let title = self.rename_input.read(cx).text().trim().to_string();
        let is_unchanged = self
            .store
            .read(cx)
            .thread(self.thread_id)
            .is_none_or(|thread| thread.title == title);
        if !is_unchanged {
            let thread_id = self.thread_id;
            self.store
                .update(cx, |store, cx| store.set_custom_title(thread_id, title, cx));
        }
    }

    /// Enter or Escape: the title is already saved, and typing goes back to the composer.
    fn end_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.renaming = false;
        self._rename_blur = None;
        window.focus(&self.focus_handle(cx), cx);
        cx.notify();
    }

    /// Starts a thread with `agent_id` on `account` that continues this one, and opens it.
    fn continue_with(&mut self, agent_id: AgentId, account: AccountChoice, cx: &mut Context<Self>) {
        self.continue_error = None;
        let agent_name = self
            .registry
            .read(cx)
            .agent(&agent_id)
            .map(|agent| agent.name().clone())
            .unwrap_or_else(|| agent_id.0.clone());
        let thread_id = self.thread_id;
        let task = self.store.update(cx, |store, cx| {
            store.continue_thread(thread_id, agent_id, account, cx)
        });
        self._continuing = cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| match result {
                Ok(thread_id) => cx.emit(AgentViewEvent::OpenThread(thread_id)),
                Err(error) => {
                    log::error!("couldn't continue the thread: {error:#}");
                    this.continue_error =
                        Some(format!("Couldn't continue with {agent_name}: {error:#}").into());
                    cx.notify();
                }
            })
            .log_err();
        });
    }

    /// The changes, terminal and agent options buttons.
    fn render_toolbar_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .gap_1p5()
            .child(render_changes_button(self.changes, self.is_diff_open))
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
            .child(self.render_agent_options(cx))
    }

    /// Zed's agent options: log in again, log out, or restart the agent. A workspace pane's
    /// header shows them too.
    pub(crate) fn render_agent_options(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let thread = self.thread.clone();
        let view = cx.weak_entity();
        let agent_name = self.agent_name(cx);
        PopoverMenu::new("thread-options")
            .menu(move |window, cx| {
                let thread = thread.clone();
                let view = view.clone();
                let agent_name = agent_name.clone();
                let (has_auth_methods, supports_logout, can_reload) = {
                    let thread = thread.read(cx);
                    (
                        !thread.auth_methods().is_empty(),
                        thread.supports_logout() && thread.status() == &ConnectionStatus::Ready,
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
                        let view = view.clone();
                        let agent_name = agent_name.clone();
                        // Asks first: logging out affects every thread with the agent.
                        menu = menu.entry("Log Out…", None, move |_, cx| {
                            let thread = thread.clone();
                            let request = ConfirmRequest::logout(&agent_name, move |_, cx| {
                                thread.update(cx, |thread, cx| thread.logout(cx))
                            });
                            view.update(cx, |_, cx| cx.emit(AgentViewEvent::Confirm(request)))
                                .log_err();
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
            .anchor(gpui::Anchor::TopRight)
    }

    fn render_entry(
        &self,
        index: usize,
        entry: &Entry,
        is_last: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
                let sent_at = self
                    .thread
                    .read(cx)
                    .sent_time(index)
                    .map(|time| day_aware_time(time, SystemTime::now()));
                let group = SharedString::from(format!("user-message-{index}"));
                // t3code's bubble: on the right, soft, at most four fifths wide; its time and
                // Copy under it on hover.
                v_flex()
                    .id(("user-message", index))
                    .group(group.clone())
                    .pt_3()
                    .pb_1()
                    .px_5()
                    .w_full()
                    .items_end()
                    .gap_1()
                    .when_some(sent_by, |this, sent_by| {
                        this.child(
                            h_flex()
                                .px_1()
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
                            .max_w(relative(0.8))
                            .px_3()
                            .py_2()
                            .rounded_xl()
                            .bg(user_message_background(cx))
                            .text_ui(cx)
                            .children(self.markdown((index, 0), style, cx)),
                    )
                    .child(
                        h_flex()
                            .h_5()
                            .gap_2()
                            .visible_on_hover(group)
                            .children(sent_at.map(|time| {
                                Label::new(time).size(LabelSize::XSmall).color(Color::Muted)
                            }))
                            .child(
                                IconButton::new(("copy-user-message", index), IconName::Copy)
                                    .icon_size(IconSize::XSmall)
                                    .icon_color(Color::Muted)
                                    .tooltip(Tooltip::text("Copy Message"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(Entry::UserMessage(text)) =
                                            this.thread.read(cx).entries().get(index)
                                        {
                                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                                without_handoff(text).to_string(),
                                            ));
                                        }
                                    })),
                            ),
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
                            .children(self.markdown((index, 0), style, cx)),
                    )
                    .when(show_controls, |this| {
                        this.child(self.render_thread_controls(index, cx))
                    })
                    .into_any_element()
            }
            Entry::AgentThought(_) | Entry::ToolCall(_) | Entry::Plan => {
                self.render_work_entry(index, entry, is_last, window, cx)
            }
        }
    }

    /// A tool call, thought or the plan's marker. Once a message follows it, a run of them
    /// folds into one line, as t3code folds a work group: the line draws at the run's first
    /// entry, and the rest draw nothing until it's opened. While the turn runs, its last run
    /// folds into a live line instead, t3code's `work-live` row.
    fn render_work_entry(
        &self,
        index: usize,
        entry: &Entry,
        is_last: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let live_line = self.live_line(cx).filter(|line| line.run.contains(&index));
        let run = match &live_line {
            Some(line) => Some(line.run.clone()),
            None => self.folded_run(index, cx),
        };
        let is_open = run
            .as_ref()
            .is_none_or(|run| self.opened_runs.contains(&run.start));
        let element =
            is_open.then(|| self.render_work_row(index, entry, is_last, None, window, cx));
        match run {
            Some(run) if run.start == index => {
                // Open, a live run reads as any other: what it did, then its rows.
                let header = match live_line {
                    Some(line) if !is_open => {
                        let entries = self.thread.read(cx).entries();
                        let is_last = line.entry + 1 == entries.len();
                        match entries.get(line.entry).cloned() {
                            Some(entry) => self.render_work_row(
                                line.entry,
                                &entry,
                                is_last,
                                Some(run),
                                window,
                                cx,
                            ),
                            None => div().into_any_element(),
                        }
                    }
                    _ => self.render_work_run_header(run, is_open, cx),
                };
                v_flex()
                    .w_full()
                    .child(header)
                    .children(element)
                    .into_any_element()
            }
            _ => element.unwrap_or_else(|| div().into_any_element()),
        }
    }

    /// A thought's or a tool call's own row. As a live line, it stands for `live_run`, and
    /// clicking it opens the run instead of the entry.
    fn render_work_row(
        &self,
        index: usize,
        entry: &Entry,
        is_last: bool,
        live_run: Option<Range<usize>>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match entry {
            Entry::AgentThought(_) => {
                self.render_thinking_block(index, is_last, live_run, window, cx)
            }
            Entry::ToolCall(tool_call) => {
                self.render_tool_call(index, tool_call, live_run, window, cx)
            }
            // In Zed the plan lives in the activity bar above the message editor.
            _ => div().into_any_element(),
        }
    }

    /// The run of work `index` is in, if it folds: a run of at least two tool calls or thoughts
    /// that a message follows, or that ended its turn. A lone one stays as it is, as in t3code.
    /// The running turn's last run folds into its live line instead (`live_line`).
    fn folded_run(&self, index: usize, cx: &App) -> Option<Range<usize>> {
        let thread = self.thread.read(cx);
        let entries = thread.entries();
        let run = work_run(entries, index)?;
        if thread.is_working() && run.end == entries.len() {
            return None;
        }
        let needs_confirmation = run.clone().any(|index| self.awaits_confirmation(index, cx));
        (shown_work(&entries[run.clone()]) >= 2 && !needs_confirmation).then_some(run)
    }

    /// The running turn's last run of work, when it has two tool calls or thoughts, folded into
    /// one live line, as t3code's `activeWorkRow`: the line shows the tool call awaiting
    /// confirmation, with its buttons, or else the latest entry still running, or else the
    /// latest one.
    fn live_line(&self, cx: &App) -> Option<LiveLine> {
        let thread = self.thread.read(cx);
        let entries = thread.entries();
        let last = entries.len().checked_sub(1)?;
        let run = work_run(entries, last).filter(|_| thread.is_working())?;
        if shown_work(&entries[run.clone()]) < 2 {
            return None;
        }
        let awaiting: Vec<usize> = run
            .clone()
            .filter(|&index| self.awaits_confirmation(index, cx))
            .collect();
        let entry = match awaiting.as_slice() {
            [] => run
                .clone()
                .rev()
                .find(|&index| match &entries[index] {
                    Entry::ToolCall(tool_call) => matches!(
                        tool_call.status,
                        acp::ToolCallStatus::InProgress | acp::ToolCallStatus::Pending
                    ),
                    Entry::AgentThought(_) => index == last,
                    _ => false,
                })
                .or_else(|| {
                    run.clone()
                        .rev()
                        .find(|&index| !matches!(entries[index], Entry::Plan))
                })?,
            [entry] => *entry,
            // One line can't hold several requests' buttons, so the rows show.
            _ => return None,
        };
        Some(LiveLine {
            run,
            entry,
            awaits_confirmation: !awaiting.is_empty(),
        })
    }

    fn awaits_confirmation(&self, index: usize, cx: &App) -> bool {
        let thread = self.thread.read(cx);
        matches!(thread.entries().get(index), Some(Entry::ToolCall(tool_call))
            if thread.permission_request(&tool_call.id).is_some())
    }

    /// Opens or folds a run of work.
    fn toggle_run(&mut self, run: Range<usize>, cx: &mut Context<Self>) {
        let Range { start, end } = run;
        if !self.opened_runs.remove(&start) {
            // Its rows open as they first showed, also those opened before it folded on its
            // own as the agent wrote after them.
            let entries = self.thread.read(cx).entries();
            for (index, entry) in entries.iter().enumerate().take(end).skip(start) {
                match entry {
                    Entry::ToolCall(tool_call) => {
                        self.toggled_tool_calls.remove(&tool_call.id);
                    }
                    Entry::AgentThought(_) => {
                        self.toggled_thoughts.remove(&index);
                    }
                    _ => {}
                }
            }
            self.opened_runs.insert(start);
        }
        self.list_state.remeasure_items(start + 1..end + 1);
        cx.notify();
    }

    /// t3code's work group header: what the run did, opening to its rows.
    fn render_work_run_header(
        &self,
        run: Range<usize>,
        is_open: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let color = work_row_color(cx);
        let summary = summarize_work(&self.thread.read(cx).entries()[run.clone()]);
        let start = run.start;
        let toggle =
            cx.listener(move |this, _: &gpui::ClickEvent, _, cx| this.toggle_run(run.clone(), cx));
        div()
            .mx_5()
            .py_1()
            .child(
                h_flex()
                    .id(("work-run", start))
                    .debug_selector(|| format!("work-run-{start}"))
                    .min_h(px(24.))
                    .gap_1p5()
                    .px_0p5()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
                    .on_click(toggle)
                    .child(
                        h_flex().w(px(24.)).flex_none().justify_center().child(
                            Icon::new(if is_open {
                                IconName::ChevronDown
                            } else {
                                IconName::ChevronRight
                            })
                            .size(IconSize::XSmall)
                            .color(Color::Custom(color)),
                        ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(rems_from_px(13_f32))
                            .text_color(color)
                            .child(summary),
                    ),
            )
            .into_any_element()
    }

    /// Zed's controls under a finished reply: copy it, jump to the prompt, jump to the top.
    fn render_thread_controls(&self, index: usize, cx: &mut Context<Self>) -> AnyElement {
        let turn_time = self.thread.read(cx).turn_time(index);
        h_flex()
            .w_full()
            .px_5()
            .pb_3()
            .gap_2()
            .children(turn_time.map(|duration| {
                Label::new(format!("Worked for {}", format_duration(duration)))
                    .size(LabelSize::Small)
                    .color(Color::Muted)
            }))
            .child(
                IconButton::new(("copy-agent-response", index), IconName::Copy)
                    .icon_size(IconSize::XSmall)
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
            .into_any_element()
    }

    /// A thought, as a row among the tool calls (t3code's reasoning row): "Thinking" with a
    /// shimmer while the agent thinks, then "Thought", opening to the text. Settings › General's
    /// "Show thinking" opens every thought by default (Zed's `thinking_display`).
    fn render_thinking_block(
        &self,
        index: usize,
        is_last: bool,
        live_run: Option<Range<usize>>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_thinking = is_last && self.thread.read(cx).is_working();
        let open_by_default = crate::app_settings::AppSettingsStore::global(cx)
            .read(cx)
            .settings()
            .show_thinking;
        let is_live = live_run.is_some();
        let is_open = open_by_default != (!is_live && self.toggled_thoughts.contains(&index));
        let color = work_row_color(cx);
        let row_group = SharedString::from(format!("thinking-row-{index}"));
        let label = if is_thinking {
            shimmering_label(("thinking-shimmer", index), "Thinking", color, cx).into_any_element()
        } else {
            div().child("Thought").into_any_element()
        };
        let row = h_flex()
            .id(("thinking-block", index))
            .debug_selector(|| format!("thinking-row-{index}"))
            .group(row_group.clone())
            .min_h(px(24.))
            .gap_1p5()
            .px_0p5()
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(cx.theme().colors().ghost_element_hover))
            .on_click(cx.listener(move |this, _, _, cx| match &live_run {
                Some(run) => this.toggle_run(run.clone(), cx),
                None => {
                    if !this.toggled_thoughts.remove(&index) {
                        this.toggled_thoughts.insert(index);
                    }
                    cx.notify();
                }
            }))
            .child(
                h_flex().w(px(24.)).flex_none().justify_center().child(
                    Icon::new(IconName::ToolThink)
                        .size(IconSize::Small)
                        .color(Color::Custom(color)),
                ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(rems_from_px(13_f32))
                    .text_color(color)
                    .child(label),
            )
            .child(
                div().flex_none().visible_on_hover(row_group).child(
                    Icon::new(if is_open && !is_live {
                        IconName::ChevronUp
                    } else {
                        IconName::ChevronDown
                    })
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
                ),
            );
        let style = MarkdownStyle::themed(MarkdownFont::Agent, window, cx).with_muted_text(cx);
        // The thoughts under the row, their left border under its icon, their text where a
        // tool call's output starts.
        let thoughts = is_open.then(|| {
            div()
                .id(("thinking-content", index))
                .debug_selector(|| format!("thinking-content-{index}"))
                .ml(px(13.))
                .pl(px(16.))
                .py_1()
                .border_l_1()
                .border_color(Self::tool_card_border_color(cx))
                .children(self.markdown((index, 0), style, cx))
        });

        v_flex()
            .mx_5()
            .child(row)
            .children(thoughts)
            .into_any_element()
    }

    fn render_tool_call(
        &self,
        index: usize,
        tool_call: &ToolCall,
        live_run: Option<Range<usize>>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = cx.theme().colors();
        let failed = matches!(tool_call.status, acp::ToolCallStatus::Failed);
        let in_progress = matches!(
            tool_call.status,
            acp::ToolCallStatus::InProgress | acp::ToolCallStatus::Pending
        );
        let needs_confirmation = self
            .thread
            .read(cx)
            .permission_request(&tool_call.id)
            .is_some();
        let is_execute = matches!(tool_call.kind, acp::ToolKind::Execute);
        let has_content = !tool_call.text.is_empty()
            || !tool_call.diffs.is_empty()
            || !tool_call.terminals.is_empty()
            || !tool_call.images.is_empty()
            || tool_call.raw_input.is_some();
        let is_live = live_run.is_some();
        let is_openable = is_live || (has_content && !needs_confirmation);
        let is_open =
            needs_confirmation || (!is_live && self.toggled_tool_calls.contains(&tool_call.id));
        let (added, removed) = tool_call.diffs.iter().map(FileDiff::line_counts).fold(
            (0, 0),
            |(added, removed), (more_added, more_removed)| {
                (added + more_added, removed + more_removed)
            },
        );
        let row_group = SharedString::from(format!("tool-call-row-{index}"));
        let toggle = {
            let tool_call_id = tool_call.id.clone();
            cx.listener(move |this, _: &gpui::ClickEvent, _, cx| match &live_run {
                Some(run) => this.toggle_run(run.clone(), cx),
                None => {
                    if !this.toggled_tool_calls.remove(&tool_call_id) {
                        this.toggled_tool_calls.insert(tool_call_id.clone());
                    }
                    cx.notify();
                }
            })
        };
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
        .color(Color::Custom(work_row_color(cx)));
        let row = h_flex()
            .id(("tool-call-row", index))
            .debug_selector(|| format!("tool-call-row-{index}"))
            .group(row_group.clone())
            .min_h(px(24.))
            .gap_1p5()
            .px_0p5()
            .rounded_md()
            .when(is_openable, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(colors.ghost_element_hover))
                    .on_click(toggle)
            })
            .child(h_flex().w(px(24.)).flex_none().justify_center().child(icon))
            .child(self.render_tool_call_label(tool_call, in_progress, cx))
            .when(added + removed > 0, |this| {
                this.child(div().flex_none().child(diff_stat(added, removed)))
            })
            .when(in_progress, |this| {
                this.child(
                    Icon::new(IconName::LoadCircle)
                        .size(IconSize::XSmall)
                        .color(Color::Muted)
                        .with_rotate_animation(2),
                )
            })
            .when(failed, |this| {
                this.child(
                    Label::new("Failed")
                        .size(LabelSize::Small)
                        .color(Color::Error),
                )
            })
            .when(is_openable, |this| {
                this.child(
                    div().flex_none().visible_on_hover(row_group).child(
                        Icon::new(if is_open && !is_live {
                            IconName::ChevronUp
                        } else {
                            IconName::ChevronDown
                        })
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                    ),
                )
            });

        // What it read, ran or wrote, under the row past its icon, scrolling past 24rem.
        let mut output = Vec::new();
        if is_open && has_content {
            let is_edit =
                matches!(tool_call.kind, acp::ToolKind::Edit) || !tool_call.diffs.is_empty();
            if !is_execute && !is_edit && tool_call.raw_input.is_some() {
                output.extend(
                    self.markdown(
                        (index, RAW_INPUT_PART),
                        tool_output_style(true, window, cx),
                        cx,
                    )
                    .map(|markdown| div().text_xs().child(markdown).into_any_element()),
                );
            }
            for (diff_index, diff) in tool_call.diffs.iter().enumerate() {
                output.push(render_diff(diff, (index, diff_index), cx));
            }
            for terminal_id in &tool_call.terminals {
                if let Some(terminal) = self.tool_terminals.get(terminal_id) {
                    output.push(
                        div()
                            .w_full()
                            .py_1()
                            .rounded_md()
                            .bg(colors.terminal_background)
                            .child(terminal.clone())
                            .into_any_element(),
                    );
                }
            }
            for part in 0..tool_call.text.len() {
                let style = tool_output_style(is_execute, window, cx);
                if let Some(markdown) = self.markdown((index, part + 1), style, cx) {
                    output.push(div().text_xs().child(markdown).into_any_element());
                }
            }
            for (image_index, id) in tool_call.images.iter().enumerate() {
                output.push(self.render_tool_image(index, image_index, id.clone(), cx));
            }
        }
        let details = (!output.is_empty()).then(|| {
            v_flex()
                .id(("tool-call-output", index))
                .debug_selector(|| format!("tool-call-output-{index}"))
                .ml(px(30.))
                .max_h(rems(24.))
                .overflow_y_scroll()
                .py_1()
                .gap_1()
                .children(output)
        });

        v_flex()
            .mx_5()
            .child(row)
            .children(details)
            .children(self.render_permission_buttons(index, &tool_call.id, cx))
            .into_any_element()
    }

    /// What the tool call did, in t3code's words where its kind says what: "Ran" and the
    /// command in the code font, "Edited" and the file; otherwise the agent's own title. Paths in
    /// the thread's folder read relative to it.
    fn render_tool_call_label(
        &self,
        tool_call: &ToolCall,
        in_progress: bool,
        cx: &App,
    ) -> AnyElement {
        let folder = self
            .store
            .read(cx)
            .thread_folder(self.thread_id)
            .map(|folder| format!("{}/", folder.display()));
        let relative = |text: &str| -> String {
            match &folder {
                Some(folder) => text.replace(folder.as_str(), ""),
                None => text.to_string(),
            }
        };
        let (verb, subject) = if matches!(tool_call.kind, acp::ToolKind::Execute) {
            // A backslash before a newline continues the command on the next line, so on one
            // line it's a space.
            let command = tool_call
                .title
                .trim()
                .trim_matches('`')
                .replace("\\\r\n", " ")
                .replace("\\\n", " ");
            (
                Some(if in_progress { "Running" } else { "Ran" }),
                Some(command),
            )
        } else if let [diff] = tool_call.diffs.as_slice() {
            (
                Some("Edited"),
                Some(relative(&diff.path.display().to_string())),
            )
        } else if tool_call.diffs.len() > 1 {
            (
                None,
                Some(format!("Edited {} files", tool_call.diffs.len())),
            )
        } else {
            (None, Some(relative(&tool_call.title)))
        };
        let subject = one_line(&subject.unwrap_or_default());
        // One dim gray for the whole row, so rows read apart from the agent's messages; the
        // command keeps the code font.
        h_flex()
            .flex_1()
            .min_w_0()
            .gap_1()
            .text_size(rems_from_px(13_f32))
            .text_color(work_row_color(cx))
            .children(verb.map(|verb| div().flex_none().child(verb)))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .when(verb.is_some(), |this| {
                        this.font_buffer(cx).text_size(rems_from_px(12_f32))
                    })
                    .child(subject),
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
                .debug_selector(|| format!("permission-buttons-{index}"))
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
            images: Vec::new(),
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
                    div().p_0p5().bg(Self::tool_card_header_bg(cx)).child(
                        h_flex()
                            .px_1()
                            .min_h(px(24.))
                            .child(self.render_tool_call_label(&tool_call, false, cx)),
                    ),
                )
                .child(buttons)
                .into_any_element(),
        )
    }

    fn render_generating(&self, cx: &App) -> Option<AnyElement> {
        let thread = self.thread.read(cx);
        let started_at = thread.turn_started_at()?;
        // Any permission request, whether its tool call is shown or not.
        let awaiting_confirmation = !thread.state.permission_requests.is_empty();
        // t3code's "Working for 1m 12s".
        let elapsed_label = format!(
            "Working for {}",
            format_elapsed(started_at.elapsed().unwrap_or_default())
        );
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

    /// Requests for input that belong to a request rather than the conversation, such as a
    /// login's, sit above the composer, as in Zed.
    fn render_request_elicitations(&self, cx: &App) -> Vec<AnyElement> {
        self.elicitation_cards
            .iter()
            .filter(|card| card.read(cx).is_for_request())
            .map(|card| div().px_2().pb_2().child(card.clone()).into_any_element())
            .collect()
    }

    /// The thread stopped with an error while its account's last read has a window used up:
    /// agentZ takes that as the limit, however the agent worded it.
    fn limit_reached(&self, now: SystemTime, cx: &App) -> Option<LimitReached> {
        if self.is_archived {
            return None;
        }
        let agent_id = self.agent_id.clone()?;
        let thread = self.thread.read(cx);
        if thread.is_working() {
            return None;
        }
        let error = thread.turn_error()?.clone();
        let account = self.store.read(cx).thread(self.thread_id)?.account;
        let accounts = self.client.read(cx).accounts(&agent_id);
        let window = used_up_window(&accounts.status(account)?.status.windows, now)?.clone();
        Some(LimitReached {
            agent_id,
            accounts,
            account,
            window,
            error,
        })
    }

    /// t3code's banner for a thread stopped by a usage limit, as Zed's warning callout over the
    /// composer: whose limit ran out and when it resets, then Continue on the agent's account
    /// with the most left, its arrow listing the others.
    fn render_limit_notice(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if self.limit_notice_dismissed {
            return None;
        }
        let now = SystemTime::now();
        let reached = self.limit_reached(now, cx)?;
        let has_accounts = reached.accounts.listed().len() > 1;
        let own = account_entry(&reached.accounts, reached.account);
        let who = has_accounts.then(|| account_phrase(&own, true));
        let (title, body) =
            limit_notice_text(who.as_deref(), &self.agent_name(cx), &reached.window, now);
        let others = if has_accounts {
            accounts_to_continue_on(&reached.accounts, reached.account, now)
        } else {
            Vec::new()
        };
        let usage_page = self
            .registry
            .read(cx)
            .agent(&reached.agent_id)
            .and_then(|agent| agent.accounts.as_ref())
            .and_then(|support| support.usage_page.clone());

        let continue_button = others.split_first().map(|(first, rest)| {
            let label = format!(
                "Continue on {}{}",
                account_phrase(first, false),
                room_note(first, now)
            );
            let agent_id = reached.agent_id.clone();
            let account = first.account;
            let on_click = cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.continue_with(agent_id.clone(), AccountChoice::of(account), cx)
            });
            if rest.is_empty() {
                return div()
                    .debug_selector(|| "limit-continue".into())
                    .child(
                        Button::new("limit-continue", label)
                            .style(ButtonStyle::Outlined)
                            .label_size(LabelSize::Small)
                            .on_click(on_click),
                    )
                    .into_any_element();
            }
            let view = cx.weak_entity();
            let agent_id = reached.agent_id.clone();
            let rest = rest.to_vec();
            let menu = PopoverMenu::new("limit-continue-menu")
                .trigger_with_tooltip(
                    IconButton::new("limit-continue-others", IconName::ChevronDown)
                        .icon_size(IconSize::XSmall),
                    Tooltip::text("Continue on Another Account"),
                )
                .menu(move |window, cx| {
                    let view = view.clone();
                    let agent_id = agent_id.clone();
                    let rest = rest.clone();
                    Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                        for entry in &rest {
                            let account = entry.account;
                            let entry = entry.clone();
                            let view = view.clone();
                            let agent_id = agent_id.clone();
                            menu = menu.custom_entry(
                                move |_, cx| {
                                    let selector = account_selector(entry.account);
                                    account_row(&entry, None, false, now, cx)
                                        .debug_selector(move || {
                                            format!("limit-continue-on-{selector}")
                                        })
                                        .into_any_element()
                                },
                                move |_, cx| {
                                    let agent_id = agent_id.clone();
                                    view.update(cx, |view, cx| {
                                        view.continue_with(agent_id, AccountChoice::of(account), cx)
                                    })
                                    .log_err();
                                },
                            );
                        }
                        menu
                    }))
                })
                .anchor(Anchor::TopRight)
                .offset(gpui::point(px(0.), px(2.)));
            div()
                .debug_selector(|| "limit-continue".into())
                .child(
                    SplitButton::new(
                        ButtonLike::new("limit-continue")
                            .child(Label::new(label).size(LabelSize::Small))
                            .on_click(on_click),
                        div()
                            .debug_selector(|| "limit-continue-others".into())
                            .child(menu)
                            .into_any_element(),
                    )
                    .style(SplitButtonStyle::Outlined),
                )
                .into_any_element()
        });
        let usage_button = usage_page.map(|url| {
            Button::new("limit-usage-page", "Usage")
                .label_size(LabelSize::Small)
                .color(Color::Muted)
                .end_icon(
                    Icon::new(IconName::ArrowUpRight)
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                )
                .on_click(move |_, _, cx| cx.open_url(&url))
        });
        let has_buttons = continue_button.is_some() || usage_button.is_some();
        let error = reached.error;
        let callout = Callout::new()
            .severity(Severity::Warning)
            .icon(IconName::Warning)
            .title(title)
            .description_slot(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .id("limit-notice-body")
                            .text_color(cx.theme().colors().text_muted)
                            .child(body)
                            // In the agent's own words.
                            .tooltip(Tooltip::text(error)),
                    )
                    .when(has_buttons, |this| {
                        this.child(
                            h_flex()
                                .flex_wrap()
                                .gap_1()
                                .children(continue_button)
                                .children(usage_button),
                        )
                    }),
            )
            .dismiss_action(
                div()
                    .debug_selector(|| "limit-notice-dismiss".into())
                    .child(
                        IconButton::new("limit-notice-dismiss", IconName::Close)
                            .icon_size(IconSize::Small)
                            .tooltip(Tooltip::text("Dismiss"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.limit_notice_dismissed = true;
                                cx.notify();
                            })),
                    ),
            );
        Some(
            div()
                .debug_selector(|| "limit-notice".into())
                .px_2()
                .pb_2()
                .child(callout)
                .into_any_element(),
        )
    }

    fn render_errors(&self, cx: &App) -> Option<AnyElement> {
        // The limit notice says it instead, even once closed.
        let at_limit = self.limit_reached(SystemTime::now(), cx).is_some();
        let thread = self.thread.read(cx);
        let callout = if let ConnectionStatus::Failed(error) = thread.status() {
            Callout::new()
                .severity(Severity::Error)
                .icon(IconName::XCircle)
                .title(format!("{} couldn't start", self.agent_name(cx)))
                .description(error.clone())
        } else if let Some(error) = thread.turn_error().filter(|_| !at_limit) {
            Callout::new()
                .severity(Severity::Error)
                .icon(IconName::XCircle)
                .title("The agent stopped with an error")
                .description(error.clone())
        } else if let Some(error) = &self.continue_error {
            Callout::new()
                .severity(Severity::Error)
                .icon(IconName::XCircle)
                .title(error.clone())
        } else if let Some(error) = &self.replace_error {
            Callout::new()
                .severity(Severity::Error)
                .icon(IconName::XCircle)
                .title("Couldn't start the thread that way")
                .description(error.clone())
        } else if let Some(error) = &self.attachment_error {
            Callout::new()
                .severity(Severity::Error)
                .icon(IconName::XCircle)
                .title(error.clone())
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

    fn render_queue_section(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let thread = self.thread.read(cx);
        let messages = thread.state.queued_messages.clone();
        let is_steering = thread.state.steering_queued;
        if messages.is_empty() {
            return None;
        }
        let colors = cx.theme().colors();
        let count = messages.len();
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
                        this.thread.update(cx, |thread, cx| thread.clear_queue(cx));
                    })),
            );

        let mut rows = Vec::new();
        for (index, message) in messages.iter().enumerate() {
            let is_next = index == 0;
            let id = message.id;
            let text = self.prompt_text(&message.prompt, cx);
            let images: Vec<AnyElement> = message
                .prompt
                .iter()
                .filter_map(|part| match part {
                    PromptPart::Image(id) => Some(id.clone()),
                    _ => None,
                })
                .enumerate()
                .map(|(image_index, id)| self.render_queued_image((index, image_index), id, cx))
                .collect();
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
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .child(text.lines().next().unwrap_or_default().to_string()),
                            )
                            .children(images),
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
                                        this.thread.update(cx, |thread, cx| {
                                            thread.remove_queued_message(id, cx)
                                        });
                                    })),
                            )
                            .child(
                                IconButton::new(("edit-queued", index), IconName::Pencil)
                                    .icon_size(IconSize::Small)
                                    .tooltip(Tooltip::text("Edit"))
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.edit_queued_message(id, window, cx);
                                    })),
                            )
                            .child(
                                Button::new(("steer-queued", index), "Steer")
                                    .label_size(LabelSize::Small)
                                    .style(ButtonStyle::Outlined)
                                    .toggle_state(is_next && is_steering)
                                    .selected_style(ButtonStyle::Tinted(ui::TintColor::Accent))
                                    .tooltip(Tooltip::text(
                                        "Send once the agent finishes the step it's on",
                                    ))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.list_state.scroll_to_end();
                                        this.thread.update(cx, |thread, cx| {
                                            thread.steer_queued_message(id, cx)
                                        });
                                    })),
                            )
                            .child(
                                Button::new(("send-queued-now", index), "Send Now")
                                    .label_size(LabelSize::Small)
                                    .when(is_next, |this| this.style(ButtonStyle::Outlined))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.list_state.scroll_to_end();
                                        this.thread.update(cx, |thread, cx| {
                                            thread.send_queued_message_now(id, cx)
                                        });
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

    /// An image in a queued message: a small thumbnail, larger on hover, whole on click.
    fn render_queued_image(
        &self,
        element_index: (usize, usize),
        id: AttachmentId,
        cx: &Context<Self>,
    ) -> AnyElement {
        let image = self.attachment_image(id, cx);
        let thumbnail = image.thumbnail();
        let (message_index, image_index) = element_index;
        let name = format!("queued-image-{message_index}-{image_index}");
        div()
            .id(SharedString::from(name.clone()))
            .debug_selector(move || name)
            .flex_none()
            .size(px(18.))
            .rounded_sm()
            .overflow_hidden()
            .border_1()
            .border_color(cx.theme().colors().border_variant)
            .cursor_pointer()
            .child(
                img(thumbnail.clone())
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .tooltip(move |_, cx| {
                let thumbnail = thumbnail.clone();
                cx.new(|_| ImagePreviewTooltip(thumbnail)).into()
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.view_image(image.original(), cx);
            }))
            .into_any_element()
    }

    /// An image a tool gave back, as Zed shows one in a tool call's output; whole on click.
    fn render_tool_image(
        &self,
        entry_index: usize,
        image_index: usize,
        id: AttachmentId,
        cx: &Context<Self>,
    ) -> AnyElement {
        let image = self.attachment_image(id, cx);
        let name = format!("tool-image-{entry_index}-{image_index}");
        div()
            .id(SharedString::from(name.clone()))
            .debug_selector(move || name)
            .cursor_pointer()
            .child(FittedImage::new(image.thumbnail(), TOOL_IMAGE_SIZE))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.view_image(image.original(), cx);
            }))
            .into_any_element()
    }

    /// Puts a queued message back in the composer, its mentions as chips again, and takes it
    /// out of the queue.
    fn edit_queued_message(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(message) = self
            .thread
            .read(cx)
            .state
            .queued_messages
            .iter()
            .find(|message| message.id == id)
            .cloned()
        else {
            return;
        };
        self.thread
            .update(cx, |thread, cx| thread.remove_queued_message(id, cx));
        self.restore_prompt(&message.prompt, cx);
        window.focus(&self.composer.focus_handle(cx), cx);
        cx.notify();
    }

    /// Puts a message in the composer, its mentions as chips.
    fn restore_prompt(&mut self, prompt: &[PromptPart], cx: &mut Context<Self>) {
        self.composer
            .update(cx, |composer, cx| composer.set_text("", cx));
        self.mentions.clear();
        self.uploads.clear();
        let mut after_chip = false;
        for part in prompt {
            if let PromptPart::Text(text) = part {
                // A chip brings the space after it, which the text after it starts with.
                let text = if after_chip {
                    text.strip_prefix(' ').unwrap_or(text)
                } else {
                    text
                };
                self.composer
                    .update(cx, |composer, cx| composer.insert(text, cx));
                after_chip = false;
                continue;
            }
            let (label, icon, preview, mention) = match part {
                PromptPart::Path(path) => {
                    let label = path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string());
                    let is_dir =
                        self.client.read(cx).machine() == MachineId::Local && path.is_dir();
                    let icon = if is_dir {
                        IconName::Folder
                    } else {
                        IconName::File
                    };
                    let preview = ChipPreview::Text(compact_path(path).into());
                    (label, icon, preview, Mention::Path(path.clone()))
                }
                PromptPart::Thread(thread_id) => {
                    let title: SharedString = self.thread_title(*thread_id, cx).into();
                    (
                        title.to_string(),
                        IconName::Thread,
                        ChipPreview::Text(title),
                        Mention::Thread(*thread_id),
                    )
                }
                PromptPart::Image(id) => {
                    let image = self.attachment_image(id.clone(), cx);
                    (
                        "Image".to_string(),
                        IconName::Image,
                        ChipPreview::Image(image.thumbnail()),
                        Mention::Image(id.clone()),
                    )
                }
                PromptPart::Text(_) => continue,
            };
            self.insert_mention(None, &label, icon, preview, mention, cx);
            after_chip = true;
        }
    }

    /// A message's text as the queue shows it, its mentions as copying their chips gives.
    fn prompt_text(&self, prompt: &[PromptPart], cx: &App) -> String {
        prompt
            .iter()
            .map(|part| match part {
                PromptPart::Text(text) => text.clone(),
                PromptPart::Path(path) => format!(
                    "@{}",
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.display().to_string())
                ),
                PromptPart::Thread(thread_id) => format!("@{}", self.thread_title(*thread_id, cx)),
                PromptPart::Image(_) => "@Image".to_string(),
            })
            .collect()
    }

    fn thread_title(&self, thread_id: ThreadId, cx: &App) -> String {
        self.store
            .read(cx)
            .thread(thread_id)
            .map_or_else(|| "Thread".to_string(), |thread| thread.title.clone())
    }

    /// An image kept for this thread, on its machine.
    fn attachment_image(&self, id: AttachmentId, cx: &App) -> AttachmentImage {
        AttachmentImage {
            machine: self.client.read(cx).machine(),
            thread_id: self.thread_id,
            id,
        }
    }

    /// Opens the image viewer at the next render, which has the window to focus it in.
    fn view_image(&mut self, source: ImageSource, cx: &mut Context<Self>) {
        self.hovered_image = None;
        self.pending_image_viewer = Some(source);
        cx.notify();
    }

    fn open_image_viewer(
        &mut self,
        source: ImageSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let viewer = cx.new(|cx| ImageViewer::new(source, window, cx));
        let subscription =
            cx.subscribe_in(&viewer, window, |this, _, _: &DismissEvent, window, cx| {
                this.image_viewer = None;
                window.focus(&this.focus_handle(cx), cx);
                cx.notify();
            });
        self.image_viewer = Some((viewer, subscription));
    }

    fn render_image_viewer(&self) -> Option<AnyElement> {
        let (viewer, _) = self.image_viewer.as_ref()?;
        Some(
            deferred(
                anchored()
                    .position(gpui::point(px(0.), px(0.)))
                    .child(viewer.clone()),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }

    /// The thumbnail of the image link under the mouse, above where the mouse came onto it.
    fn render_image_hover(&self, cx: &App) -> Option<AnyElement> {
        let hovered = self.hovered_image.as_ref()?;
        let image = self.attachment_image(hovered.id.clone(), cx);
        Some(
            deferred(
                anchored()
                    .position(hovered.position - gpui::point(px(0.), px(8.)))
                    .anchor(Anchor::BottomLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .debug_selector(|| "image-hover-preview".into())
                            .child(render_hover_preview(image.thumbnail(), cx)),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }

    /// A message's markdown: its image links show their thumbnail on hover and open the
    /// viewer on click.
    fn markdown(
        &self,
        key: MarkdownKey,
        style: MarkdownStyle,
        cx: &Context<Self>,
    ) -> Option<MarkdownElement> {
        let markdown = self.markdowns.get(&key)?;
        let hovered = cx.weak_entity();
        let clicked = cx.weak_entity();
        Some(
            MarkdownElement::new(markdown.clone(), style)
                .on_url_hover(move |url, window, cx| {
                    let id = url.as_deref().and_then(AttachmentId::from_uri);
                    let position = window.mouse_position();
                    hovered
                        .update(cx, |this, cx| this.hover_image_link(key, id, position, cx))
                        .ok();
                })
                .on_url_click(move |url, _, cx| match AttachmentId::from_uri(&url) {
                    Some(id) => {
                        clicked
                            .update(cx, |this, cx| {
                                let image = this.attachment_image(id, cx);
                                this.view_image(image.original(), cx);
                            })
                            .ok();
                    }
                    None => cx.open_url(&url),
                }),
        )
    }

    /// Follows the image link the mouse is on in one of the messages. Every message hears
    /// every move, so only the one the mouse is in, or was last in, changes it.
    fn hover_image_link(
        &mut self,
        key: MarkdownKey,
        id: Option<AttachmentId>,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        match id {
            Some(id) => {
                let is_same = self
                    .hovered_image
                    .as_ref()
                    .is_some_and(|hovered| hovered.key == key && hovered.id == id);
                if !is_same {
                    self.hovered_image = Some(HoveredImage { key, id, position });
                    cx.notify();
                }
            }
            None => {
                if self
                    .hovered_image
                    .as_ref()
                    .is_some_and(|hovered| hovered.key == key)
                {
                    self.hovered_image = None;
                    cx.notify();
                }
            }
        }
    }

    /// The bar above the message editor: agents, plan, edited files and queued messages.
    fn render_activity_bar(&self, window: &Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let sections: Vec<AnyElement> = [
            self.render_agents_section(cx),
            self.render_plan_section(window, cx),
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

    /// The thread's account's window closest to running out, which the composer's gauge shows.
    fn gauged_window(&self, cx: &App) -> Option<LimitWindow> {
        let agent_id = self.agent_id.as_ref()?;
        let account = self.store.read(cx).thread(self.thread_id)?.account;
        let accounts = self.client.read(cx).accounts(agent_id);
        // Its last read is from before it logged out.
        if accounts.logged_in(account) == Some(false) {
            return None;
        }
        tightest_window(&accounts.status(account)?.status.windows).cloned()
    }

    /// Beside the agent, what's left of the thread's account's window closest to running out,
    /// opening that account's windows.
    fn render_usage_gauge(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let window = self.gauged_window(cx)?;
        let agent_id = self.agent_id.clone()?;
        let account = self.store.read(cx).thread(self.thread_id)?.account;
        let left = window.left_percent();
        let color = if left == 0 {
            Color::Error
        } else if left <= LOW_PERCENT {
            Color::Warning
        } else {
            Color::Muted
        };
        let tooltip = format!("{}: {left}% left", window.label);
        let usage_page = self
            .registry
            .read(cx)
            .agent(&agent_id)
            .and_then(|agent| agent.accounts.as_ref())
            .and_then(|support| support.usage_page.clone());
        let client = self.client.clone();
        Some(
            div()
                .debug_selector(|| "usage-gauge".into())
                .child(
                    PopoverMenu::new("usage-gauge-menu")
                        .trigger_with_tooltip(
                            ButtonLike::new("usage-gauge")
                                .style(ButtonStyle::Filled)
                                .size(ButtonSize::Compact)
                                .child(
                                    h_flex()
                                        .gap_0p5()
                                        .child(
                                            Icon::new(IconName::Gauge)
                                                .size(IconSize::XSmall)
                                                .color(color),
                                        )
                                        .child(
                                            Label::new(format!("{left}%"))
                                                .size(LabelSize::Small)
                                                .color(color),
                                        ),
                                ),
                            Tooltip::text(tooltip),
                        )
                        .menu(move |window, cx| {
                            Some(cx.new(|cx| {
                                UsagePopover::new(
                                    client.clone(),
                                    agent_id.clone(),
                                    account,
                                    usage_page.clone(),
                                    window,
                                    cx,
                                )
                            }))
                        })
                        .anchor(Anchor::BottomLeft)
                        .offset(gpui::point(px(0.), px(-4.))),
                )
                .into_any_element(),
        )
    }

    /// The thread this one continues, with its agent's name and icon, while it's kept.
    fn continued_from(&self, cx: &App) -> Option<(ThreadId, SharedString, SharedString, Icon)> {
        let store = self.store.read(cx);
        let from = store.thread(store.thread(self.thread_id)?.continued_from?)?;
        let (name, icon) = self.thread_agent(from, cx);
        Some((from.id, from.title.clone().into(), name, icon))
    }

    /// A plain draft with nothing typed, which New Thread opens again rather than making
    /// another, as t3code does.
    pub(crate) fn is_untouched_draft(&self, cx: &App) -> bool {
        self.is_draft(cx)
            && self.typed_draft(cx).text.is_none()
            && self.replacing.is_none()
            && self.thread.read(cx).pending_handoff().is_none()
    }

    /// A new thread nothing has been sent in yet (t3code's draft thread).
    pub(crate) fn is_draft(&self, cx: &App) -> bool {
        self.store
            .read(cx)
            .thread(self.thread_id)
            .is_some_and(|thread| thread.is_draft)
            && !self
                .thread
                .read(cx)
                .entries()
                .iter()
                .any(|entry| matches!(entry, Entry::UserMessage(_)))
    }

    /// The title of the thread this one continues, until its first message is sent.
    fn continued_title(&self, cx: &App) -> Option<SharedString> {
        let handoff = self.thread.read(cx).pending_handoff()?;
        Some(handoff.from_title.clone().into())
    }

    /// A thread's agent, by name, and its icon: muted, or in the thread's account's color.
    fn thread_agent(&self, thread: &projects::Thread, cx: &App) -> (SharedString, Icon) {
        let agent_id = thread.agent_id.clone().map(AgentId::new);
        let name = agent_id
            .as_ref()
            .and_then(|agent_id| self.registry.read(cx).agent(agent_id))
            .map(|agent| agent.name().clone())
            .or_else(|| thread.agent_id.clone().map(SharedString::from))
            .unwrap_or_else(|| "Agent".into());
        let icon = agent_id
            .as_ref()
            .and_then(|agent_id| agent_icon(agent_id, cx))
            .map(Icon::from_svg_markup)
            .unwrap_or_else(|| Icon::new(IconName::Sparkle));
        let account_color = agent_id
            .as_ref()
            .and_then(|agent_id| self.account_icon_color(agent_id, thread.account, cx));
        (name, icon.color(account_color.unwrap_or(Color::Muted)))
    }

    /// The color the agent's icon takes on the account's threads
    /// ([`agentz_protocol::accounts::AgentAccounts::thread_color`]).
    fn account_icon_color(
        &self,
        agent_id: &AgentId,
        account: Option<AccountId>,
        cx: &App,
    ) -> Option<Color> {
        let hex = self.client.read(cx).thread_color(agent_id, account)?;
        account_color(hex, cx).map(Color::Custom)
    }

    /// The divider that opens a continued thread: where it came from.
    fn render_continued_from(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (from_id, title, agent_name, icon) = self.continued_from(cx)?;
        let line = cx.theme().colors().border;
        let rule = move || div().flex_1().h_px().bg(line);
        Some(
            h_flex()
                .debug_selector(|| "thread-continued-from".into())
                .w_full()
                .px_5()
                .pt_2()
                .pb_1()
                .gap_2()
                .child(rule())
                .child(
                    h_flex()
                        .flex_none()
                        .gap_1()
                        .child(
                            Label::new("Continued from")
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        )
                        .child(icon.size(IconSize::XSmall))
                        .child(
                            Label::new(format!("{agent_name} ·"))
                                .size(LabelSize::Small)
                                .color(Color::Muted),
                        )
                        .child(
                            div()
                                .id("continued-from-thread")
                                .max_w(px(320.))
                                .cursor_pointer()
                                .child(
                                    Label::new(title)
                                        .size(LabelSize::Small)
                                        .color(Color::Muted)
                                        .underline()
                                        .truncate(),
                                )
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    cx.emit(AgentViewEvent::OpenThread(from_id))
                                })),
                        ),
                )
                .child(rule())
                .into_any_element(),
        )
    }

    /// A card for each thread that continues this one with another agent, after its end.
    fn render_continuations(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let continuations: Vec<(ThreadId, SharedString, SharedString, Icon)> = {
            let store = self.store.read(cx);
            store
                .continuations(self.thread_id)
                .map(|thread| {
                    let (name, icon) = self.thread_agent(thread, cx);
                    (thread.id, thread.title.clone().into(), name, icon)
                })
                .collect()
        };
        let colors = cx.theme().colors().clone();
        continuations
            .into_iter()
            .map(|(thread_id, title, agent_name, icon)| {
                let open = cx.listener(move |_, _: &ClickEvent, _, cx| {
                    cx.emit(AgentViewEvent::OpenThread(thread_id))
                });
                div()
                    .px_5()
                    .pt_2()
                    .pb_1()
                    .child(
                        h_flex()
                            .debug_selector(|| "thread-continued-in".into())
                            .px_3()
                            .py_2p5()
                            .gap_3()
                            .rounded_lg()
                            .border_1()
                            .border_color(colors.border)
                            .child(icon.size(IconSize::Small))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(Label::new(format!("Continued in {agent_name}")))
                                    .child(
                                        Label::new(title)
                                            .size(LabelSize::Small)
                                            .color(Color::Muted)
                                            .truncate(),
                                    ),
                            )
                            .child(
                                Button::new(("open-continuation", thread_id.0), "Open")
                                    .style(ButtonStyle::Outlined)
                                    .label_size(LabelSize::Small)
                                    .on_click(open),
                            ),
                    )
                    .into_any_element()
            })
            .collect()
    }

    /// The conversation a continued thread brings with its first message, as an attachment in
    /// the composer: a click shows exactly what goes to the agent, × starts without it.
    fn render_handoff_chip(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let handoff = self.thread.read(cx).pending_handoff()?.clone();
        let title = SharedString::from(handoff.from_title.clone());
        let agent_name = SharedString::from(handoff.from_agent.clone());
        let icon = {
            let store = self.store.read(cx);
            store
                .thread(handoff.from)
                .map(|from| self.thread_agent(from, cx).1)
                .unwrap_or_else(|| Icon::new(IconName::Sparkle).color(Color::Muted))
        };
        let colors = cx.theme().colors().clone();
        let messages = match handoff.messages {
            1 => "1 message".to_string(),
            count => format!("{count} messages"),
        };
        let chip = h_flex()
            .id("handoff-chip")
            .debug_selector(|| "handoff-chip".into())
            .max_w_full()
            .min_w_0()
            .gap_1p5()
            .pl_2()
            .pr_1()
            .py_0p5()
            .rounded_md()
            .border_1()
            .border_color(colors.border)
            .bg(colors.element_background)
            .cursor_pointer()
            .hover(|style| style.bg(colors.element_hover))
            .when(self.handoff_expanded, |chip| {
                chip.bg(colors.element_selected)
            })
            .child(
                Icon::new(IconName::Return)
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            )
            .child(icon.size(IconSize::XSmall))
            .child(
                div()
                    .min_w_0()
                    .child(Label::new(title).size(LabelSize::Small).truncate()),
            )
            .child(
                Label::new(format!("· {agent_name} · {messages}"))
                    .size(LabelSize::Small)
                    .color(Color::Muted)
                    .single_line(),
            )
            .child(
                IconButton::new("drop-handoff", IconName::Close)
                    .icon_size(IconSize::XSmall)
                    .icon_color(Color::Muted)
                    .tooltip(Tooltip::text("Start Without It"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.handoff_expanded = false;
                        this.thread.update(cx, |thread, cx| thread.drop_handoff(cx));
                    })),
            )
            .tooltip(Tooltip::text("Show What Goes with Your Message"))
            .on_click(cx.listener(|this, _, _, cx| {
                this.handoff_expanded = !this.handoff_expanded;
                cx.notify();
            }));
        let preview = self.handoff_expanded.then(|| {
            div()
                .id("handoff-preview")
                .debug_selector(|| "handoff-preview".into())
                .max_h(px(200.))
                .overflow_y_scroll()
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(colors.border_variant)
                .bg(colors.editor_background)
                .font_buffer(cx)
                .text_xs()
                .text_color(colors.text_muted)
                .child(handoff.text.clone())
        });
        Some(
            v_flex()
                .w_full()
                .pt_1()
                .gap_1()
                .child(h_flex().w_full().child(chip))
                .children(preview)
                .into_any_element(),
        )
    }

    fn render_message_editor(&self, style: ComposerStyle, cx: &mut Context<Self>) -> AnyElement {
        let colors = cx.theme().colors();
        let thread = self.thread.read(cx);
        let agent_name = self.agent_name(cx);
        let is_generating = thread.is_working();
        let is_editor_empty = self.composer.read(cx).text().trim().is_empty();
        let has_failed = matches!(thread.status(), ConnectionStatus::Failed(_));
        let needs_login = thread.status() == &ConnectionStatus::AuthRequired;
        // The agent's options come with its session, which opens only once the agent has
        // started, so until then their place says they're coming.
        let settings_loading = (thread.status() == &ConnectionStatus::Connecting).then(|| {
            h_flex()
                .debug_selector(|| "session-settings-loading".into())
                .px_1()
                .gap_1()
                .child(
                    SpinnerLabel::dots()
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    Label::new("Loading options…")
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
        });

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
                if is_editor_empty || has_failed || needs_login {
                    this.disabled(true).icon_color(Color::Muted)
                } else {
                    this.icon_color(Color::Accent)
                }
            })
            .tooltip(Tooltip::text(if needs_login {
                "Log In to Send"
            } else if is_editor_empty {
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
            .key_context({
                let mut context = gpui::KeyContext::new_with_defaults();
                context.add(KEY_CONTEXT);
                if crate::app_settings::AppSettingsStore::global(cx)
                    .read(cx)
                    .settings()
                    .use_modifier_to_send
                {
                    context.add("use_modifier_to_send");
                }
                context
            })
            .on_action(cx.listener(Self::send))
            .on_action(cx.listener(Self::stop))
            .on_action(cx.listener(Self::select_next_command))
            .on_action(cx.listener(Self::accept_slash_command))
            .on_action(cx.listener(Self::select_previous_command))
            .justify_center()
            .map(|this| match style {
                ComposerStyle::Bar => this
                    .py_2()
                    .bg(colors.editor_background)
                    .border_t_1()
                    .border_color(colors.border),
                ComposerStyle::Card => this.w_full(),
            })
            .child(
                v_flex()
                    .w_full()
                    .max_w(MAX_CONTENT_WIDTH)
                    .min_w_0()
                    .px_2()
                    .gap_2()
                    .when(style == ComposerStyle::Card, |this| {
                        this.p_2()
                            .rounded_lg()
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.editor_background)
                            .shadow_md()
                    })
                    // A draft can still be typed, but the composer reads as waiting on the login.
                    .when(needs_login, |this| this.opacity(0.55))
                    .children(self.render_handoff_chip(cx))
                    .child(
                        v_flex()
                            .relative()
                            .w_full()
                            .pt_1()
                            .pr_2p5()
                            .text_ui(cx)
                            .when(style == ComposerStyle::Card, |this| this.min_h(px(44.)))
                            .debug_selector(|| "composer".into())
                            .child(self.composer.clone())
                            .children(self.render_command_menu(cx))
                            .children(self.render_mention_menu(cx))
                            .children(self.render_composer_menu())
                            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                                for path in paths.paths() {
                                    this.mention_path(path.clone(), cx);
                                }
                                window.focus(&this.composer.focus_handle(cx), cx);
                            })),
                    )
                    // Zed's footer: when it's too narrow, the controls go to their own row as a
                    // whole, then wrap within it.
                    .child(
                        h_flex()
                            .w_full()
                            .min_w_0()
                            .flex_none()
                            .flex_wrap()
                            .justify_between()
                            .child(
                                h_flex()
                                    .min_w_0()
                                    .flex_wrap()
                                    .gap_0p5()
                                    .child(self.render_add_context_button(cx))
                                    .child(match style {
                                        ComposerStyle::Bar => h_flex()
                                            .gap_1()
                                            .px_1()
                                            .child(
                                                self.thread_agent_icon(cx).size(IconSize::XSmall),
                                            )
                                            .child(
                                                Label::new(agent_name)
                                                    .size(LabelSize::Small)
                                                    .color(Color::Muted),
                                            )
                                            .children(self.render_usage_gauge(cx))
                                            .into_any_element(),
                                        ComposerStyle::Card => self.render_agent_picker(cx),
                                    }),
                            )
                            .child(
                                h_flex()
                                    .min_w_0()
                                    .flex_wrap()
                                    .gap_1()
                                    .children(self.render_context_usage(cx))
                                    .children(settings_loading)
                                    .children(self.render_session_settings(cx))
                                    .child(send_button),
                            ),
                    ),
            )
            .into_any_element()
    }

    /// The new thread screen (t3code's): a headline, the composer as a card, and under it
    /// where the thread works. Until the first message, the agent, checkout and machine can
    /// still change.
    fn render_new_thread(&self, cx: &mut Context<Self>) -> AnyElement {
        let headline: SharedString = match self.continued_title(cx) {
            Some(title) => format!("Continue “{title}”").into(),
            None => "What should we work on?".into(),
        };
        v_flex()
            .id("new-thread")
            .debug_selector(|| "new-thread".into())
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .px_4()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .w_full()
                    .max_w(NEW_THREAD_WIDTH)
                    .gap_6()
                    .child(
                        div()
                            .text_center()
                            .text_2xl()
                            .text_color(cx.theme().colors().text)
                            .child(headline),
                    )
                    .child(
                        v_flex()
                            .gap_2()
                            .child(self.render_message_editor(ComposerStyle::Card, cx))
                            .child(self.render_new_thread_strip(cx))
                            .children(self.render_errors(cx)),
                    ),
            )
            .into_any_element()
    }

    /// The agent, in the new thread's composer: a menu of the machine's installed agents.
    fn render_agent_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.weak_entity();
        let registry = self.registry.clone();
        let current_agent = self.agent_id.clone();
        // A continuation brings a conversation, which only an agent can take, and a Workspaces
        // draft sits in a workspace, beside its shells.
        let offers_terminal =
            self.thread.read(cx).pending_handoff().is_none() && !self.in_workspaces(cx);
        PopoverMenu::new("new-thread-agent")
            .menu(move |window, cx| {
                let agents: Vec<(AgentId, SharedString)> = {
                    let registry = registry.read(cx);
                    registry
                        .agents()
                        .iter()
                        .filter(|agent| {
                            agent.supports_current_platform()
                                && matches!(
                                    registry.install_state(agent.id()),
                                    InstallState::Installed { .. }
                                )
                        })
                        .map(|agent| (agent.id().clone(), agent.name().clone()))
                        .collect()
                };
                let view = view.clone();
                let current_agent = current_agent.clone();
                Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                    for (agent_id, name) in &agents {
                        let view = view.clone();
                        let render_id = agent_id.clone();
                        let agent_id = agent_id.clone();
                        let name = name.clone();
                        let is_current = current_agent.as_ref() == Some(&agent_id);
                        menu = menu.custom_entry(
                            move |_, cx| {
                                h_flex()
                                    .w_full()
                                    .gap_1p5()
                                    .child(
                                        agent_icon(&render_id, cx)
                                            .map(Icon::from_svg_markup)
                                            .unwrap_or_else(|| Icon::new(IconName::Sparkle))
                                            .size(IconSize::Small)
                                            .color(Color::Muted),
                                    )
                                    .child(div().flex_1().child(Label::new(name.clone())))
                                    .when(is_current, |row| {
                                        row.child(
                                            Icon::new(IconName::Check)
                                                .size(IconSize::Small)
                                                .color(Color::Accent),
                                        )
                                    })
                                    .into_any_element()
                            },
                            move |_, cx| {
                                let starter = Starter::Agent(agent_id.clone());
                                view.update(cx, |view, cx| {
                                    view.change_new_thread_starter(starter, cx)
                                })
                                .log_err();
                            },
                        );
                    }
                    menu = menu.separator();
                    if offers_terminal {
                        let view = view.clone();
                        menu = menu.item(
                            ContextMenuEntry::new("Terminal")
                                .icon(IconName::Terminal)
                                .icon_color(Color::Muted)
                                .handler(move |_, cx| {
                                    view.update(cx, |view, cx| {
                                        view.change_new_thread_starter(Starter::Terminal, cx)
                                    })
                                    .log_err();
                                }),
                        );
                    }
                    menu.item(
                        ContextMenuEntry::new("Manage Agents…")
                            .icon(IconName::Settings)
                            .icon_color(Color::Muted)
                            .handler(move |_, cx| {
                                view.update(cx, |_, cx| cx.emit(AgentViewEvent::OpenAgentSettings))
                                    .log_err();
                            }),
                    )
                }))
            })
            .trigger_with_tooltip(
                picker_chip(
                    "new-thread-agent-trigger",
                    self.agent_icon(cx),
                    self.agent_name(cx),
                )
                .disabled(self.replacing.is_some()),
                Tooltip::text("Change Agent"),
            )
            .anchor(gpui::Anchor::TopLeft)
            .offset(gpui::point(px(0.), px(4.)))
            .into_any_element()
    }

    /// Under the new thread's composer: its checkout, machine and account, and its branch.
    fn render_new_thread_strip(&self, cx: &mut Context<Self>) -> AnyElement {
        let left = match &self.replacing {
            Some(status) => h_flex()
                .h(px(22.))
                .px_1p5()
                .child(
                    Label::new(status.clone())
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                        .with_animation(
                            "new-thread-replacing",
                            Animation::new(Duration::from_secs(2))
                                .repeat()
                                .with_easing(pulsating_between(0.4, 0.8)),
                            |label, delta| label.alpha(delta),
                        ),
                )
                .into_any_element(),
            None if self.in_workspaces(cx) => h_flex()
                .gap_1()
                .child(self.render_folder_picker(cx))
                .children(self.render_account_picker(cx))
                .into_any_element(),
            None => h_flex()
                .gap_1()
                .child(self.render_checkout_picker(cx))
                .children(self.render_machine_picker(cx))
                .children(self.render_account_picker(cx))
                .into_any_element(),
        };
        let branch = self.thread_branch(cx).map(|(branch, _, folder)| {
            h_flex()
                .id("new-thread-branch")
                .min_w_0()
                .h(px(22.))
                .px_1p5()
                .gap_1()
                .child(
                    Icon::new(IconName::GitBranch)
                        .size(IconSize::XSmall)
                        .color(Color::Muted),
                )
                .child(
                    Label::new(branch)
                        .size(LabelSize::Small)
                        .color(Color::Muted)
                        .truncate(),
                )
                .tooltip(Tooltip::text(folder.display().to_string()))
        });
        h_flex()
            .w_full()
            .justify_between()
            .gap_2()
            .child(left)
            .children(branch)
            .into_any_element()
    }

    /// Where the new thread works: the project's own folder, a new worktree or pasture, or
    /// one of its worktrees and pastures.
    fn render_checkout_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let current = store
            .thread(self.thread_id)
            .and_then(|thread| thread.workspace.clone());
        let (icon, label) = match store.thread_workspace(self.thread_id) {
            Some(workspace) => (workspace_icon(workspace.kind), workspace.kind.label()),
            None => (IconName::Folder, "Local"),
        };
        let existing: Vec<(PathBuf, WorkspaceKind, SharedString)> = {
            store
                .thread(self.thread_id)
                .and_then(|thread| store.project(thread.project_id))
                .map(|project| project.workspaces.clone())
                .unwrap_or_default()
                .into_iter()
                .map(|workspace| {
                    let branch = store
                        .git_head(&workspace.path)
                        .map(|head| head.branch.clone())
                        .or_else(|| workspace.branch.clone())
                        .unwrap_or_else(|| workspace.kind.label().to_string());
                    (workspace.path, workspace.kind, branch.into())
                })
                .collect()
        };
        // Until the repository is read, a new worktree or pasture is offered and the server
        // says if it can't be made.
        let is_repository = self.draft_git.as_ref().is_none_or(|git| git.is_repository);
        let pasture_unsupported = matches!(
            self.draft_git.as_ref().map(|git| &git.pastures),
            Some(PastureSupport::Unsupported(_))
        );
        if !is_repository && existing.is_empty() {
            return static_chip("new-thread-checkout", Icon::new(icon), label.into())
                .tooltip(Tooltip::text(
                    "Not a git repository: threads work in its folder",
                ))
                .into_any_element();
        }
        let chip = picker_chip("new-thread-checkout-trigger", Icon::new(icon), label.into());
        let view = cx.weak_entity();
        PopoverMenu::new("new-thread-checkout")
            .menu(move |window, cx| {
                let view = view.clone();
                let current = current.clone();
                let existing = existing.clone();
                Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                    let choose = |choice: WorkspaceChoice| {
                        let view = view.clone();
                        move |_: &mut Window, cx: &mut App| {
                            view.update(cx, |view, cx| {
                                view.change_new_thread_checkout(choice.clone(), cx)
                            })
                            .log_err();
                        }
                    };
                    menu = menu
                        .header("Where It Works")
                        .item(
                            ContextMenuEntry::new("Local checkout")
                                .icon(IconName::Folder)
                                .icon_color(Color::Muted)
                                .toggleable(IconPosition::End, current.is_none())
                                .handler(choose(WorkspaceChoice::Checkout)),
                        )
                        .item(
                            ContextMenuEntry::new("New worktree")
                                .icon(workspace_icon(WorkspaceKind::Worktree))
                                .icon_color(Color::Muted)
                                .disabled(!is_repository)
                                .handler(choose(WorkspaceChoice::New {
                                    kind: WorkspaceKind::Worktree,
                                    base: None,
                                    branch: None,
                                })),
                        )
                        .item(
                            ContextMenuEntry::new("New pasture")
                                .icon(workspace_icon(WorkspaceKind::Pasture))
                                .icon_color(Color::Muted)
                                .disabled(!is_repository || pasture_unsupported)
                                .handler(choose(WorkspaceChoice::New {
                                    kind: WorkspaceKind::Pasture,
                                    base: None,
                                    branch: None,
                                })),
                        );
                    if !existing.is_empty() {
                        menu = menu.separator().header("Existing");
                        for (path, kind, branch) in &existing {
                            menu = menu.item(
                                ContextMenuEntry::new(branch.clone())
                                    .icon(workspace_icon(*kind))
                                    .icon_color(Color::Muted)
                                    .toggleable(IconPosition::End, current.as_ref() == Some(path))
                                    .handler(choose(WorkspaceChoice::Existing(path.clone()))),
                            );
                        }
                    }
                    menu
                }))
            })
            .trigger_with_tooltip(chip, Tooltip::text("Where the Thread Works"))
            .anchor(gpui::Anchor::TopLeft)
            .offset(gpui::point(px(0.), px(4.)))
            .into_any_element()
    }

    /// Where a Workspaces thread works: the folder it was started in, shown as its path, or a
    /// new worktree or pasture of that folder's repository.
    fn render_folder_picker(&self, cx: &mut Context<Self>) -> AnyElement {
        let store = self.store.read(cx);
        let (Some(thread), Some(folder)) = (
            store.thread(self.thread_id),
            store.thread_folder(self.thread_id),
        ) else {
            return div().into_any_element();
        };
        let in_starting_folder = thread.started_in.is_none();
        let icon = store
            .thread_workspace(self.thread_id)
            .filter(|_| !in_starting_folder)
            .map_or(IconName::Folder, |workspace| workspace_icon(workspace.kind));
        let label: SharedString = compact_path(&folder).into();
        let tooltip = folder.display().to_string();
        // Offered only once the folder is known to be in git.
        let is_repository = self.draft_git.as_ref().is_some_and(|git| git.is_repository);
        if !is_repository {
            return static_chip("new-thread-folder", Icon::new(icon), label)
                .debug_selector(|| "new-thread-folder".into())
                .tooltip(Tooltip::text(tooltip))
                .into_any_element();
        }
        let pasture_unsupported = matches!(
            self.draft_git.as_ref().map(|git| &git.pastures),
            Some(PastureSupport::Unsupported(_))
        );
        let chip = picker_chip("new-thread-folder-trigger", Icon::new(icon), label);
        let view = cx.weak_entity();
        let menu = PopoverMenu::new("new-thread-folder")
            .menu(move |window, cx| {
                let view = view.clone();
                Some(ContextMenu::build(window, cx, move |menu, _, _| {
                    let choose = |choice: WorkspaceChoice| {
                        let view = view.clone();
                        move |_: &mut Window, cx: &mut App| {
                            view.update(cx, |view, cx| {
                                view.change_new_thread_checkout(choice.clone(), cx)
                            })
                            .log_err();
                        }
                    };
                    menu.header("Where It Works")
                        .item(
                            ContextMenuEntry::new("Current checkout")
                                .icon(IconName::Folder)
                                .icon_color(Color::Muted)
                                .toggleable(IconPosition::End, in_starting_folder)
                                .handler(choose(WorkspaceChoice::Checkout)),
                        )
                        .item(
                            ContextMenuEntry::new("New worktree")
                                .icon(workspace_icon(WorkspaceKind::Worktree))
                                .icon_color(Color::Muted)
                                .handler(choose(WorkspaceChoice::New {
                                    kind: WorkspaceKind::Worktree,
                                    base: None,
                                    branch: None,
                                })),
                        )
                        .item(
                            ContextMenuEntry::new("New pasture")
                                .icon(workspace_icon(WorkspaceKind::Pasture))
                                .icon_color(Color::Muted)
                                .disabled(pasture_unsupported)
                                .handler(choose(WorkspaceChoice::New {
                                    kind: WorkspaceKind::Pasture,
                                    base: None,
                                    branch: None,
                                })),
                        )
                }))
            })
            .trigger_with_tooltip(chip, Tooltip::text(tooltip))
            .anchor(gpui::Anchor::TopLeft)
            .offset(gpui::point(px(0.), px(4.)));
        div()
            .debug_selector(|| "new-thread-folder-menu".into())
            .child(menu)
            .into_any_element()
    }

    /// The machine the new thread runs on, with the project's checkouts on other machines to
    /// move it to. Shown only when there are other machines, and not in a workspace pane,
    /// which belongs to its machine.
    fn render_machine_picker(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let machines = Machines::global(cx).read(cx);
        if !machines.has_remotes() || self.is_in_pane {
            return None;
        }
        let store = self.store.read(cx);
        let machine = store.machine();
        let project_id = store.thread(self.thread_id)?.project_id;
        let agent_id = self.agent_id.clone()?;
        let agent_name = self.agent_name(cx);
        let members = machines
            .group_of(machine, project_id, cx)
            .map(|group| group.members)
            .unwrap_or_default();
        let icon = Icon::new(machines.machine_icon(machine, cx));
        let label = machines.label(machine, cx);
        if members.len() < 2 {
            return Some(static_chip("new-thread-machine", icon, label).into_any_element());
        }
        // Each checkout of the project, and whether the thread can move there.
        let rows: Vec<(ProjectKey, IconName, SharedString, bool)> = members
            .iter()
            .map(|(member_machine, project)| {
                let mut label = machines.label(*member_machine, cx).to_string();
                // Two checkouts on one machine are told apart by folder.
                if members
                    .iter()
                    .filter(|(other, _)| other == member_machine)
                    .count()
                    > 1
                {
                    label = format!("{label} · {}", project.path.display());
                }
                let client = machines.client(*member_machine, cx);
                let is_usable = match &client {
                    Some(client) if client.read(cx).is_online() => {
                        let registry = client.read(cx).registry().read(cx);
                        let is_installed = matches!(
                            registry.install_state(&agent_id),
                            InstallState::Installed { .. }
                        );
                        if !is_installed {
                            label = format!("{label} · {agent_name} isn't installed");
                        }
                        is_installed
                    }
                    _ => {
                        label = format!("{label} · offline");
                        false
                    }
                };
                (
                    ProjectKey {
                        machine: *member_machine,
                        project: project.id,
                    },
                    machines.machine_icon(*member_machine, cx),
                    label.into(),
                    is_usable,
                )
            })
            .collect();
        let chip = picker_chip("new-thread-machine-trigger", icon, label);
        let current = ProjectKey {
            machine,
            project: project_id,
        };
        let view = cx.weak_entity();
        Some(
            PopoverMenu::new("new-thread-machine")
                .menu(move |window, cx| {
                    let view = view.clone();
                    let rows = rows.clone();
                    Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                        for (project, icon, label, is_usable) in &rows {
                            let view = view.clone();
                            let project = *project;
                            menu = menu.item(
                                ContextMenuEntry::new(label.clone())
                                    .icon(*icon)
                                    .icon_color(Color::Muted)
                                    .toggleable(IconPosition::End, project == current)
                                    .disabled(!is_usable)
                                    .handler(move |_, cx| {
                                        view.update(cx, |view, cx| {
                                            view.change_new_thread_machine(project, cx)
                                        })
                                        .log_err();
                                    }),
                            );
                        }
                        menu
                    }))
                })
                .trigger_with_tooltip(chip, Tooltip::text("Machine"))
                .anchor(gpui::Anchor::TopLeft)
                .offset(gpui::point(px(0.), px(4.)))
                .into_any_element(),
        )
    }

    /// The account the new thread runs on, shown only when its agent has more than one: its
    /// avatar and name, with a menu of the accounts, their plans and the windows closest to
    /// running out, then Add Account… and Manage Accounts….
    fn render_account_picker(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let agent_id = self.agent_id.clone()?;
        let accounts = self.client.read(cx).accounts(&agent_id);
        let entries = account_entries(&accounts);
        if entries.len() < 2 {
            return None;
        }
        // A draft made before its account was found logged out still shows it.
        let current = self.store.read(cx).thread(self.thread_id)?.account;
        let shown = account_entry(&accounts, current);
        let chip = ButtonLike::new("new-thread-account-trigger")
            .style(ButtonStyle::Subtle)
            .size(ButtonSize::Compact)
            .child(
                h_flex()
                    .min_w_0()
                    .gap_1()
                    .child(render_entry_avatar(&shown, px(14.), cx))
                    .child(
                        Label::new(shown.name.clone())
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    )
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .size(IconSize::XSmall)
                            .color(Color::Muted),
                    ),
            );
        let view = cx.weak_entity();
        let now = SystemTime::now();
        let menu = PopoverMenu::new("new-thread-account")
            .menu(move |window, cx| {
                let view = view.clone();
                let entries = entries.clone();
                let agent_id = agent_id.clone();
                Some(ContextMenu::build(window, cx, move |mut menu, _, _| {
                    for entry in &entries {
                        let account = entry.account;
                        let is_current = account == current;
                        let entry = entry.clone();
                        let view = view.clone();
                        menu = menu.custom_entry(
                            move |_, cx| render_account_row(&entry, is_current, now, cx),
                            move |_, cx| {
                                view.update(cx, |view, cx| {
                                    view.change_new_thread_account(account, cx)
                                })
                                .log_err();
                            },
                        );
                    }
                    let open_accounts = |add_account: bool| {
                        let view = view.clone();
                        let agent_id = agent_id.clone();
                        move |_: &mut Window, cx: &mut App| {
                            view.update(cx, |_, cx| {
                                cx.emit(AgentViewEvent::OpenAgentAccounts {
                                    agent_id: agent_id.clone(),
                                    add_account,
                                })
                            })
                            .log_err();
                        }
                    };
                    menu.separator()
                        .item(
                            ContextMenuEntry::new("Add Account…")
                                .icon(IconName::Plus)
                                .icon_color(Color::Muted)
                                .handler(open_accounts(true)),
                        )
                        .item(
                            ContextMenuEntry::new("Manage Accounts…")
                                .icon(IconName::Settings)
                                .icon_color(Color::Muted)
                                .handler(open_accounts(false)),
                        )
                }))
            })
            .trigger_with_tooltip(chip, Tooltip::text("Account"))
            .anchor(gpui::Anchor::TopLeft)
            .offset(gpui::point(px(0.), px(4.)));
        Some(
            div()
                .debug_selector(|| "new-thread-account".into())
                .child(menu)
                .into_any_element(),
        )
    }

    /// Asks for the project's repository once, for the checkout picker.
    fn load_new_thread_git(&mut self, cx: &mut Context<Self>) {
        if self._draft_git_load.is_some() {
            return;
        }
        let store = self.store.read(cx);
        let Some(thread) = store.thread(self.thread_id) else {
            return;
        };
        let git = if thread.in_workspaces() {
            let Some(folder) = thread.starting_folder().cloned() else {
                return;
            };
            let checkouts = store.repository_checkouts(folder, cx);
            // Outside git, the server answers with an error.
            cx.spawn(async move |_, _| anyhow::Ok(checkouts.await?.git))
        } else {
            store.project_git(thread.project_id, cx)
        };
        self._draft_git_load = Some(cx.spawn(async move |this, cx| {
            // An older server, or one that can't read the repository, offers only the checkout.
            let git = git.await.unwrap_or_default();
            this.update(cx, |this, cx| {
                this.draft_git = Some(git);
                cx.notify();
            })
            .ok();
        }));
    }

    /// Where the thread works now, as a choice for a thread made in its place. A Workspaces
    /// thread's checkout is the folder it was started in.
    fn current_workspace_choice(&self, cx: &App) -> WorkspaceChoice {
        let store = self.store.read(cx);
        let Some(thread) = store.thread(self.thread_id) else {
            return WorkspaceChoice::Checkout;
        };
        match &thread.workspace {
            Some(_) if thread.in_workspaces() && thread.started_in.is_none() => {
                WorkspaceChoice::Checkout
            }
            Some(path) => WorkspaceChoice::Existing(path.clone()),
            None => WorkspaceChoice::Checkout,
        }
    }

    fn in_workspaces(&self, cx: &App) -> bool {
        self.store
            .read(cx)
            .thread(self.thread_id)
            .is_some_and(Thread::in_workspaces)
    }

    fn current_project(&self, cx: &App) -> Option<ProjectKey> {
        let store = self.store.read(cx);
        Some(ProjectKey {
            machine: store.machine(),
            project: store.thread(self.thread_id)?.project_id,
        })
    }

    fn change_new_thread_starter(&mut self, starter: Starter, cx: &mut Context<Self>) {
        if let Starter::Agent(agent_id) = &starter
            && self.agent_id.as_ref() == Some(agent_id)
        {
            return;
        }
        let Some(project) = self.current_project(cx) else {
            return;
        };
        let workspace = self.current_workspace_choice(cx);
        // Another agent's accounts are its own, so it starts on its account for new threads.
        self.replace_new_thread(project, starter, workspace, AccountChoice::Default, cx);
    }

    fn change_new_thread_checkout(&mut self, workspace: WorkspaceChoice, cx: &mut Context<Self>) {
        if workspace == self.current_workspace_choice(cx) {
            return;
        }
        let (Some(project), Some(agent_id)) = (self.current_project(cx), self.agent_id.clone())
        else {
            return;
        };
        let account = self.current_account_choice(cx);
        self.replace_new_thread(project, Starter::Agent(agent_id), workspace, account, cx);
    }

    /// The thread moves to the project's checkout on another machine, in its own folder there,
    /// on that machine's account for new threads: each machine has its own accounts.
    fn change_new_thread_machine(&mut self, project: ProjectKey, cx: &mut Context<Self>) {
        if Some(project) == self.current_project(cx) {
            return;
        }
        let Some(agent_id) = self.agent_id.clone() else {
            return;
        };
        self.replace_new_thread(
            project,
            Starter::Agent(agent_id),
            WorkspaceChoice::Checkout,
            AccountChoice::Default,
            cx,
        );
    }

    /// `account` being `None` for the External one.
    fn change_new_thread_account(&mut self, account: Option<AccountId>, cx: &mut Context<Self>) {
        let current = self
            .store
            .read(cx)
            .thread(self.thread_id)
            .map(|thread| thread.account);
        if current == Some(account) {
            return;
        }
        let (Some(project), Some(agent_id)) = (self.current_project(cx), self.agent_id.clone())
        else {
            return;
        };
        let workspace = self.current_workspace_choice(cx);
        self.replace_new_thread(
            project,
            Starter::Agent(agent_id),
            workspace,
            AccountChoice::of(account),
            cx,
        );
    }

    /// The thread's account, for a thread made in its place on this machine; the account for
    /// new threads once it's no longer listed.
    fn current_account_choice(&self, cx: &App) -> AccountChoice {
        let (Some(agent_id), Some(thread)) = (
            self.agent_id.as_ref(),
            self.store.read(cx).thread(self.thread_id),
        ) else {
            return AccountChoice::Default;
        };
        let accounts = self.client.read(cx).accounts(agent_id);
        if accounts.listed().contains(&thread.account) {
            AccountChoice::of(thread.account)
        } else {
            AccountChoice::Default
        }
    }

    /// ACP can't change a session's agent, account or folder, so the new thread is made again
    /// with what was picked, shown in its place, and this one is deleted. A continuation stays
    /// one, in the workspace of the thread it continues.
    fn replace_new_thread(
        &mut self,
        project: ProjectKey,
        starter: Starter,
        workspace: WorkspaceChoice,
        account: AccountChoice,
        cx: &mut Context<Self>,
    ) {
        if self.replacing.is_some() {
            return;
        }
        let Some(store) = Machines::global(cx).read(cx).projects(project.machine, cx) else {
            return;
        };
        let continued_from = self
            .store
            .read(cx)
            .thread(self.thread_id)
            .and_then(|thread| thread.continued_from)
            .filter(|_| project.machine == self.store.read(cx).machine());
        let starting_folder = self
            .store
            .read(cx)
            .thread(self.thread_id)
            .filter(|thread| thread.in_workspaces())
            .and_then(|thread| thread.starting_folder().cloned());
        self.replacing = Some(match &workspace {
            WorkspaceChoice::New { kind, .. } => {
                format!("Making a {}…", kind.label().to_lowercase()).into()
            }
            _ => "Starting…".into(),
        });
        self.replace_error = None;
        cx.notify();
        let created = store.update(cx, |store, cx| match (starter, continued_from) {
            (Starter::Agent(agent_id), Some(from)) => {
                store.continue_thread(from, agent_id, account, cx)
            }
            (Starter::Agent(agent_id), None) => match starting_folder {
                Some(folder) => {
                    store.create_workspaces_thread(folder, agent_id, workspace, account, cx)
                }
                None => store.create_thread(project.project, agent_id, workspace, account, cx),
            },
            (Starter::Terminal, _) => store.create_terminal_thread(
                project.project,
                TerminalCommand::default(),
                workspace,
                cx,
            ),
        });
        self._replacing = cx.spawn(async move |this, cx| {
            let created = created.await;
            this.update(cx, |this, cx| {
                this.replacing = None;
                match created {
                    Ok(thread) => {
                        let text = this.composer.read(cx).text().clone();
                        cx.emit(AgentViewEvent::Replaced {
                            thread: ThreadKey {
                                machine: project.machine,
                                thread,
                            },
                            text,
                        });
                        // After the event: a workspace pane shows the new thread before this
                        // one is deleted, which would close the pane.
                        this.forget_unsent_text(cx);
                        let store = this.store.clone();
                        let old = this.thread_id;
                        cx.defer(move |cx| {
                            store.update(cx, |store, cx| store.delete_thread(old, cx))
                        });
                    }
                    Err(error) => {
                        log::error!("couldn't make the new thread again: {error:#}");
                        this.replace_error = Some(format!("{error:#}").into());
                    }
                }
                cx.notify();
            })
            .log_err();
        });
    }

    /// What was typed in a thread this one replaced.
    pub(crate) fn set_composer_text(&mut self, text: SharedString, cx: &mut Context<Self>) {
        self.composer
            .update(cx, |composer, cx| composer.set_text(text, cx));
    }

    /// What's typed: the text with each chip as what copying it gives, and what the chips
    /// mention.
    fn typed_draft(&self, cx: &App) -> UnsentDraft {
        let composer = self.composer.read(cx);
        let content = composer.text();
        let mut text = String::new();
        let mut mentions = Vec::new();
        let mut index = 0;
        for chip in composer.chips() {
            text.push_str(&content[index..chip.range.start]);
            let start = text.len();
            text.push_str(&chip.copy_text);
            if let Some(mention) = self.mentions.get(&chip.id) {
                mentions.push(UnsentMention {
                    range: start..text.len(),
                    target: mention.target(),
                });
            }
            index = chip.range.end;
        }
        text.push_str(&content[index..]);
        if text.trim().is_empty() {
            return UnsentDraft::default();
        }
        UnsentDraft {
            text: Some(text),
            mentions,
        }
    }

    /// Keeps what's typed on the thread's machine once typing pauses, so it's there after the
    /// user leaves the thread or quits, as t3code keeps composer drafts. Emptied, as sending
    /// does, it's saved at once.
    fn save_unsent_text(&mut self, cx: &mut Context<Self>) {
        let draft = self.typed_draft(cx);
        if draft == self.saved_unsent {
            self._save_unsent_text = Task::ready(());
            return;
        }
        if draft.text.is_none() {
            self.save_unsent_text_now(cx);
            return;
        }
        self._save_unsent_text = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(UNSENT_TEXT_SAVE_DELAY).await;
            this.update(cx, |this, cx| this.save_unsent_text_now(cx))
                .log_err();
        });
    }

    fn save_unsent_text_now(&mut self, cx: &mut App) {
        self._save_unsent_text = Task::ready(());
        let draft = self.typed_draft(cx);
        if draft == self.saved_unsent {
            return;
        }
        self.saved_unsent = draft.clone();
        self.store
            .read(cx)
            .set_unsent_text(self.thread_id, draft.text, draft.mentions, cx);
    }

    /// Empties the composer when its text was discarded elsewhere, such as from the sidebar.
    fn follow_discarded_unsent_text(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.store.read(cx).thread(self.thread_id) else {
            return;
        };
        let current = thread.unsent_text.clone();
        if current == self.observed_unsent_text {
            return;
        }
        // Only a change from what was seen counts: a save of this view's own may not be back.
        self.observed_unsent_text = current.clone();
        if current.is_none() && self.saved_unsent.text.is_some() {
            self.saved_unsent = UnsentDraft::default();
            self._save_unsent_text = Task::ready(());
            self.composer
                .update(cx, |composer, cx| composer.set_text("", cx));
        }
    }

    /// Forgets the composer's text without saving it, for a thread that's going away.
    fn forget_unsent_text(&mut self, cx: &App) {
        self._save_unsent_text = Task::ready(());
        self.saved_unsent = self.typed_draft(cx);
    }
}

/// What's typed in a composer, as a thread keeps it ([`projects::Thread::unsent_text`]).
#[derive(Clone, Debug, Default, PartialEq)]
struct UnsentDraft {
    text: Option<String>,
    mentions: Vec<UnsentMention>,
}

/// A composer draft as a message, its mentions in place of their `@name`.
fn unsent_prompt(text: &str, mentions: &[UnsentMention]) -> Vec<PromptPart> {
    let mut prompt = Vec::new();
    let mut index = 0;
    for mention in mentions {
        let Some(part) = Mention::from_target(&mention.target).map(|mention| mention.prompt_part())
        else {
            continue;
        };
        if mention.range.start < index || text.get(mention.range.clone()).is_none() {
            continue;
        }
        if mention.range.start > index {
            prompt.push(PromptPart::Text(
                text[index..mention.range.start].to_string(),
            ));
        }
        prompt.push(part);
        index = mention.range.end;
    }
    if index < text.len() {
        prompt.push(PromptPart::Text(text[index..].to_string()));
    }
    prompt
}

/// How the composer is drawn: along the bottom of a conversation, or as a card in the middle
/// of the new thread screen.
#[derive(Clone, Copy, PartialEq)]
enum ComposerStyle {
    Bar,
    Card,
}

/// The new thread screen's column, narrower than a conversation, as in t3code.
const NEW_THREAD_WIDTH: Pixels = px(680.);

/// A Workspaces thread's folder where a project thread's title bar has its project. It isn't a
/// link: there's no project to start a thread in.
fn render_folder_crumb(folder: PathBuf) -> AnyElement {
    let name = folder.file_name().map_or_else(
        || folder.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    h_flex()
        .flex_none()
        .gap_1()
        .child(
            h_flex()
                .id("thread-header-folder")
                .debug_selector(|| "thread-header-folder".into())
                .gap_1p5()
                .px_1()
                .py_0p5()
                .child(
                    Icon::new(IconName::Folder)
                        .size(IconSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    div().max_w(px(160.)).child(
                        Label::new(name)
                            .size(LabelSize::Small)
                            .color(Color::Muted)
                            .truncate(),
                    ),
                )
                .tooltip(Tooltip::text(folder.display().to_string())),
        )
        .child(Label::new("/").size(LabelSize::Small).color(Color::Muted))
        .into_any_element()
}

/// An account in the new thread's account picker: its avatar, its name over its plan, the
/// window closest to running out, and a check when the thread runs on it.
fn render_account_row(
    entry: &AccountEntry,
    is_current: bool,
    now: SystemTime,
    cx: &App,
) -> AnyElement {
    let selector = account_selector(entry.account);
    account_row(entry, None, false, now, cx)
        .debug_selector(move || format!("new-thread-account-{selector}"))
        .child(div().flex_none().w(px(14.)).when(is_current, |slot| {
            slot.child(
                Icon::new(IconName::Check)
                    .size(IconSize::Small)
                    .color(Color::Accent),
            )
        }))
        .into_any_element()
}

/// An agent in Continue with Another Agent: its icon and name.
fn render_agent_item(agent_id: &AgentId, name: SharedString, cx: &App) -> AnyElement {
    h_flex()
        .gap_1p5()
        .child(
            agent_icon(agent_id, cx)
                .map(Icon::from_svg_markup)
                .unwrap_or_else(|| Icon::new(IconName::Sparkle))
                .size(IconSize::Small)
                .color(Color::Muted),
        )
        .child(Label::new(name))
        .into_any_element()
}

/// An account as the menus that pick one show it: its avatar, its name over `note` (else its
/// plan), and the window closest to running out. Greyed, it can't be picked.
fn account_row(
    entry: &AccountEntry,
    note: Option<SharedString>,
    is_greyed: bool,
    now: SystemTime,
    cx: &App,
) -> Div {
    let note = note.or_else(|| {
        if entry.is_logged_out {
            Some("Logged out".into())
        } else {
            entry.plan.clone().map(SharedString::from)
        }
    });
    let (name_color, note_color) = if is_greyed {
        (Color::Disabled, Color::Disabled)
    } else {
        (Color::Default, Color::Muted)
    };
    // Unnamed, the External account is already called "Outside agentZ".
    let is_outside = entry.account.is_none() && entry.is_named;
    h_flex()
        .w_full()
        .gap_2()
        .child(render_entry_avatar(entry, px(18.), cx))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    h_flex()
                        .gap_1()
                        .child(Label::new(entry.name.clone()).color(name_color).truncate())
                        .when(is_outside, |name| {
                            name.child(
                                Label::new("· outside agentZ")
                                    .size(LabelSize::XSmall)
                                    .color(Color::Placeholder),
                            )
                        }),
                )
                .children(
                    note.map(|note| Label::new(note).size(LabelSize::XSmall).color(note_color)),
                ),
        )
        .children(tightest_window(&entry.windows).map(|window| {
            let left = left_label(window, now).size(LabelSize::XSmall);
            div().pl(px(14.)).child(if is_greyed {
                left.color(Color::Disabled)
            } else {
                left
            })
        }))
}

/// What the limit notice shows: the thread's account, the window it ran out of, and the
/// agent's error.
struct LimitReached {
    agent_id: AgentId,
    accounts: AgentAccounts,
    /// `None` being the External one.
    account: Option<AccountId>,
    window: LimitWindow,
    error: SharedString,
}

/// The limit notice's title and body. `who` names the account whose limit ran out, when the
/// agent has more than one to tell apart.
fn limit_notice_text(
    who: Option<&str>,
    agent_name: &str,
    window: &LimitWindow,
    now: SystemTime,
) -> (String, String) {
    let title = format!(
        "{} reached its {} limit",
        who.unwrap_or("Your account"),
        label_in_sentence(&window.label)
    );
    let mut body = format!("{agent_name} stopped.");
    if let Some(resets_at) = window.resets_at {
        let remaining = resets_at.duration_since(now).unwrap_or_default();
        body.push_str(&format!(
            " The limit resets {}, in {}.",
            reset_phrase(resets_at, now),
            crate::usage_limits::format_duration(remaining)
        ));
    }
    (title, body)
}

/// A window's label inside a sentence: "Weekly" as "weekly", but "GPT-5" as it is.
fn label_in_sentence(label: &str) -> String {
    let first_word = label.split_whitespace().next().unwrap_or_default();
    let mut characters = first_word.chars();
    let is_capitalized = characters.next().is_some_and(char::is_uppercase)
        && characters.all(|character| !character.is_uppercase());
    if !is_capitalized {
        return label.to_string();
    }
    let mut characters = label.chars();
    characters
        .next()
        .map(|first| first.to_lowercase().chain(characters).collect())
        .unwrap_or_default()
}

/// How the limit notice names an account in a sentence.
fn account_phrase(entry: &AccountEntry, starts_sentence: bool) -> String {
    // Unnamed, the External account goes by "Outside agentZ", which isn't a name.
    if entry.account.is_none() && !entry.is_named {
        if starts_sentence {
            "The account outside agentZ".to_string()
        } else {
            "the account outside agentZ".to_string()
        }
    } else {
        entry.name.to_string()
    }
}

/// The agent's other listed accounts, the one with the most room first.
fn accounts_to_continue_on(
    accounts: &AgentAccounts,
    own: Option<AccountId>,
    now: SystemTime,
) -> Vec<AccountEntry> {
    let mut entries: Vec<AccountEntry> = account_entries(accounts)
        .into_iter()
        .filter(|entry| entry.account != own)
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(room(entry, now)));
    entries
}

/// How much room an account has to go on, for ordering: what's left of its tightest window,
/// then not read yet, then used up, then logged out.
fn room(entry: &AccountEntry, now: SystemTime) -> i16 {
    if entry.is_logged_out {
        return -2;
    }
    if used_up_window(&entry.windows, now).is_some() {
        return -1;
    }
    tightest_window(&entry.windows).map_or(0, |window| i16::from(window.left_percent()))
}

/// What the Continue button says of the account it continues on.
fn room_note(entry: &AccountEntry, now: SystemTime) -> String {
    if entry.is_logged_out {
        return " · Logged out".to_string();
    }
    if used_up_window(&entry.windows, now).is_some() {
        return " · Used up".to_string();
    }
    tightest_window(&entry.windows)
        .map(|window| format!(" · {}% left", window.left_percent()))
        .unwrap_or_default()
}

/// A borderless button that opens one of the new thread's pickers.
fn picker_chip(id: &'static str, icon: Icon, label: SharedString) -> ButtonLike {
    ButtonLike::new(id)
        .style(ButtonStyle::Subtle)
        .size(ButtonSize::Compact)
        .child(
            chip_content(icon, label).child(
                Icon::new(IconName::ChevronDown)
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            ),
        )
}

/// A picker's chip when there's nothing else to pick.
fn static_chip(id: &'static str, icon: Icon, label: SharedString) -> gpui::Stateful<Div> {
    chip_content(icon, label).id(id).h(px(22.)).px_1p5()
}

fn chip_content(icon: Icon, label: SharedString) -> Div {
    h_flex()
        .min_w_0()
        .gap_1()
        .child(icon.size(IconSize::XSmall).color(Color::Muted))
        .child(
            Label::new(label)
                .size(LabelSize::Small)
                .color(Color::Muted)
                .truncate(),
        )
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

/// A tool call's edit. Long lines scroll sideways, as in Zed's editor, rather than being cut
/// off.
fn render_diff(diff: &FileDiff, (entry, part): (usize, usize), cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let lines = v_flex()
        .min_w_full()
        .children(
            diff.hunk(DIFF_CONTEXT_LINES)
                .into_iter()
                .map(|(kind, line)| {
                    let (marker, background) = match kind {
                        DiffLineKind::Context => (" ", None),
                        DiffLineKind::Removed => {
                            ("-", Some(colors.editor_diff_hunk_deleted_background))
                        }
                        DiffLineKind::Added => {
                            ("+", Some(colors.editor_diff_hunk_added_background))
                        }
                    };
                    h_flex()
                        .min_w_full()
                        .px_2()
                        .whitespace_nowrap()
                        .when_some(background, |this, background| this.bg(background))
                        .when(kind == DiffLineKind::Context, |this| {
                            this.text_color(colors.text_muted)
                        })
                        .child(
                            div()
                                .w(px(14.))
                                .flex_none()
                                .text_color(colors.text_muted)
                                .child(marker),
                        )
                        .child(line.to_string())
                }),
        );
    div()
        .id(("tool-diff", entry * 1000 + part))
        .w_full()
        .overflow_x_scroll()
        .border_t_1()
        .border_color(colors.border.opacity(0.8))
        .font_buffer(cx)
        .text_size(rems_from_px(12_f32))
        .line_height(rems_from_px(18_f32))
        .child(lines)
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
        if let Some(position) = self.pending_composer_menu.take() {
            self.deploy_composer_menu(position, window, cx);
        }
        if let Some(source) = self.pending_image_viewer.take() {
            self.open_image_viewer(source, window, cx);
        }
        self.sync_mention_query(cx);
        let menu_open = self.mention_query.is_some() || !self.matching_commands(cx).is_empty();
        self.composer
            .update(cx, |composer, cx| composer.set_menu_open(menu_open, cx));
        let panel_background = cx.theme().colors().panel_background;
        let entry_count = self.thread.read(cx).entries().len();
        let has_rows = entry_count > 0 || !self.render_tail_rows(cx).is_empty();
        let is_connecting = self.thread.read(cx).status() == &ConnectionStatus::Connecting;
        let needs_login = self.needs_login(cx);

        let is_subthread = self.parent(cx).is_some();
        let is_drawer_full_screen =
            self.drawer_full_screen && self.is_drawer_open && self.drawer.is_some();
        // A thread that never had a session has no history to load, so it shows the new
        // thread screen while its agent starts too.
        let is_never_opened = self
            .store
            .read(cx)
            .thread(self.thread_id)
            .is_some_and(|thread| thread.session_id.is_none());
        let is_new_thread = !has_rows
            && self.thread.read(cx).state.queued_messages.is_empty()
            && (!is_connecting || is_never_opened)
            && !needs_login
            && !is_subthread
            && !self.is_archived
            && self.client.read(cx).is_online();
        if is_new_thread {
            self.load_new_thread_git(cx);
        }

        v_flex()
            // Otherwise clicking the conversation would take focus from the message editor.
            .when(is_subthread, |this| this.track_focus(&self.focus_handle))
            .size_full()
            .bg(panel_background)
            .when(!self.is_in_pane, |this| {
                this.on_action(cx.listener(Self::toggle_terminal_drawer))
            })
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
            .when(!self.is_in_pane, |this| this.child(self.render_toolbar(cx)))
            .when(!is_drawer_full_screen && is_new_thread, |this| {
                this.child(self.render_new_thread(cx))
            })
            // A full-screen terminal hides the conversation and the composer.
            .when(!is_drawer_full_screen && !is_new_thread, |this| {
                this.children(self.render_restore_notice(cx))
                    .child(
                        // Only the rows in view are drawn, so long threads stay quick (Zed's list).
                        v_flex()
                            .id("agent-conversation")
                            .flex_1()
                            .min_h_0()
                            // A link scrolled from under the mouse no longer shows its image.
                            .on_scroll_wheel(cx.listener(|this, _, _, cx| {
                                if this.hovered_image.take().is_some() {
                                    cx.notify();
                                }
                            }))
                            .when(!has_rows, |this| this.pt_2().pb_4())
                            .items_center()
                            .when(has_rows, |this| {
                                this.child(
                                    list(
                                        self.list_state.clone(),
                                        cx.processor(|this, index: usize, window, cx| {
                                            this.render_conversation_row(index, window, cx)
                                        }),
                                    )
                                    .with_sizing_behavior(gpui::ListSizingBehavior::Auto)
                                    .flex_1()
                                    .w_full(),
                                )
                            })
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
                            .when(!has_rows && !is_connecting && !needs_login, |this| {
                                let prompt = match self.continued_title(cx) {
                                    Some(title) => format!(
                                        "{} continues “{title}”. Tell it what to do next.",
                                        self.agent_name(cx)
                                    ),
                                    None => {
                                        format!(
                                            "Start a conversation with {}.",
                                            self.agent_name(cx)
                                        )
                                    }
                                };
                                this.child(
                                    div()
                                        .w_full()
                                        .max_w(MAX_CONTENT_WIDTH)
                                        .px_5()
                                        .py_8()
                                        .child(Label::new(prompt).color(Color::Muted)),
                                )
                            })
                            // The login takes the empty thread's middle, or follows its history
                            // as the list's tail.
                            .when(needs_login && !has_rows, |this| {
                                this.child(
                                    v_flex()
                                        .debug_selector(|| "thread-login".into())
                                        .w_full()
                                        .max_w(MAX_CONTENT_WIDTH)
                                        .px_5()
                                        .py_8()
                                        .items_center()
                                        .flex_1()
                                        .justify_center()
                                        .child(self.login.clone()),
                                )
                            }),
                    )
                    .children(self.render_request_elicitations(cx))
                    .children(self.render_limit_notice(cx))
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
                            this.child(self.render_message_editor(ComposerStyle::Bar, cx))
                        }
                    })
            })
            .children(self.render_drawer(cx))
            .children(self.render_image_hover(cx))
            .children(self.render_image_viewer())
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

/// A read's text is the file as it is. Claude fences it as code, but Droid sends it bare, and
/// as markdown its lines would join into paragraphs, `#` lines become headings and HTML show as
/// loose text.
fn as_code_block(text: &str) -> Cow<'_, str> {
    if text.trim().is_empty() || is_code_block(text) {
        return Cow::Borrowed(text);
    }
    // Longer than any run of backticks in the text, so none of its lines closes the block.
    let longest_run = text
        .split(|character: char| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest_run.max(2) + 1);
    let text = text.strip_suffix('\n').unwrap_or(text);
    Cow::Owned(format!("{fence}\n{text}\n{fence}"))
}

/// Whether the text is a single fenced code block: its first line opens one, and the first line
/// to close it is its last.
fn is_code_block(text: &str) -> bool {
    let lines: Vec<&str> = text.trim().lines().collect();
    let [opening, body @ .., closing] = lines.as_slice() else {
        return false;
    };
    let fence_length = opening.len() - opening.trim_start_matches('`').len();
    let closes = |line: &str| {
        let line = line.trim();
        line.len() >= fence_length && line.bytes().all(|byte| byte == b'`')
    };
    fence_length >= 3 && closes(closing) && !body.iter().any(|line| closes(line))
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

/// What a thread changed: files, and lines added and removed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChangeStat {
    files: usize,
    additions: usize,
    deletions: usize,
}

/// Asks the thread's server what the thread changed, for its header's button. `None` when it
/// couldn't say.
pub(crate) fn load_change_stat(
    client: &ServerClient,
    thread_id: ThreadId,
) -> impl std::future::Future<Output = Option<ChangeStat>> + use<> {
    let request = client.request(Request::ThreadDiff {
        thread_id,
        scope: DiffScope::All,
    });
    async move {
        match request.await {
            Ok(Response::ThreadDiff(diff)) => Some(ChangeStat {
                files: diff.files.len(),
                additions: diff.files.iter().map(|file| file.additions as usize).sum(),
                deletions: diff.files.iter().map(|file| file.deletions as usize).sum(),
            }),
            Ok(response) => {
                log::error!("expected a diff, got {response:?}");
                None
            }
            Err(error) => {
                log::warn!("couldn't load the thread's changes: {error:#}");
                None
            }
        }
    }
}

/// The header's changes button: what the thread changed, in lines, as t3code's header shows it,
/// or the changes icon while it changed nothing.
pub(crate) fn render_changes_button(changes: ChangeStat, is_open: bool) -> AnyElement {
    let tooltip: SharedString = match changes.files {
        0 => "Show Changes".into(),
        1 => "Show Changes · 1 file changed".into(),
        count => format!("Show Changes · {count} files changed").into(),
    };
    if changes.files > 0 {
        ButtonLike::new("toggle-diff")
            .style(ButtonStyle::Outlined)
            .size(ButtonSize::Compact)
            .toggle_state(is_open)
            .child(
                div()
                    .px_1()
                    .child(diff_stat(changes.additions, changes.deletions)),
            )
            .tooltip(move |_, cx| Tooltip::for_action(tooltip.clone(), &ToggleDiff, cx))
            .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleDiff), cx))
            .into_any_element()
    } else {
        IconButton::new("toggle-diff", IconName::Diff)
            .icon_size(IconSize::Small)
            .toggle_state(is_open)
            .tooltip(move |_, cx| Tooltip::for_action(tooltip.clone(), &ToggleDiff, cx))
            .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleDiff), cx))
            .into_any_element()
    }
}

/// The thread's title in its header, the trigger of the thread's menu. A double-click renames
/// it instead: its second click closes the menu the first opened, then lands here.
#[derive(IntoElement)]
struct TitleButton {
    title: SharedString,
    is_open: bool,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    on_double_click: Rc<dyn Fn(&mut Window, &mut App)>,
}

impl TitleButton {
    fn new(title: SharedString, on_double_click: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        Self {
            title,
            is_open: false,
            on_click: None,
            on_double_click: Rc::new(on_double_click),
        }
    }
}

impl Clickable for TitleButton {
    fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    fn cursor_style(self, _: gpui::CursorStyle) -> Self {
        self
    }
}

impl Toggleable for TitleButton {
    fn toggle_state(mut self, selected: bool) -> Self {
        self.is_open = selected;
        self
    }
}

impl RenderOnce for TitleButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors();
        let hover = colors.ghost_element_hover;
        let on_click = self.on_click;
        let on_double_click = self.on_double_click;
        let chip = h_flex()
            .id("thread-header-title")
            .debug_selector(|| "thread-header-title".into())
            .min_w_0()
            .gap_1()
            .px_1()
            .py_0p5()
            .rounded_sm()
            .cursor_pointer()
            .when(self.is_open, |this| this.bg(colors.ghost_element_selected))
            .hover(move |style| style.bg(hover))
            .child(Label::new(self.title).size(LabelSize::Small).truncate())
            .child(
                Icon::new(IconName::ChevronDown)
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            )
            .on_click(move |event, window, cx| {
                if event.click_count() >= 2 {
                    on_double_click(window, cx);
                } else if let Some(on_click) = &on_click {
                    on_click(event, window, cx);
                }
            });
        // The row takes the header's room; the chip fits its title, shrinking with it.
        h_flex().w_full().min_w_0().child(chip)
    }
}

/// Asks first, as the sidebar does.
fn confirm_delete_thread(
    store: &Entity<ProjectStore>,
    thread_id: ThreadId,
    title: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let answer = window.prompt(
        PromptLevel::Warning,
        &format!("Delete “{title}”?"),
        Some("The thread and its conversation will be removed. This can't be undone."),
        &["Delete", "Cancel"],
        cx,
    );
    let store = store.clone();
    cx.spawn(async move |cx| {
        if answer.await == Ok(0) {
            store.update(cx, |store, cx| store.delete_thread(thread_id, cx));
        }
    })
    .detach();
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

/// t3code's durations: "45s", "1m 12s", "1h 3m".
/// A finished turn's length, as t3code words it: milliseconds under a second,
/// tenths under ten seconds, then whole units ("1h 3m", "2m 5s").
fn format_duration(duration: Duration) -> String {
    let milliseconds = duration.as_millis();
    if milliseconds < 1_000 {
        return format!("{}ms", milliseconds.max(1));
    }
    if milliseconds < 10_000 {
        let tenths = (milliseconds + 50) / 100;
        return if tenths >= 100 {
            "10s".to_string()
        } else {
            format!("{}.{}s", tenths / 10, tenths % 10)
        };
    }
    let seconds = (milliseconds + 500) / 1_000;
    if seconds < 60 {
        return format!("{seconds}s");
    }
    let mut parts = Vec::new();
    if seconds >= 3_600 {
        parts.push(format!("{}h", seconds / 3_600));
    }
    if seconds % 3_600 >= 60 {
        parts.push(format!("{}m", (seconds % 3_600) / 60));
    }
    if !seconds.is_multiple_of(60) {
        parts.push(format!("{}s", seconds % 60));
    }
    parts.join(" ")
}

/// A message's time as t3code shows it: the time today, "yesterday at" it, or the date
/// before it.
fn day_aware_time(time: SystemTime, now: SystemTime) -> String {
    use chrono::Datelike as _;
    let time = chrono::DateTime::<chrono::Local>::from(time);
    let now = chrono::DateTime::<chrono::Local>::from(now);
    let clock = time.format("%H:%M");
    let days_ago = (now.date_naive() - time.date_naive()).num_days();
    if days_ago <= 0 {
        clock.to_string()
    } else if days_ago == 1 {
        format!("yesterday at {clock}")
    } else if time.year() == now.year() {
        format!("{} {clock}", time.format("%-m/%-d"))
    } else {
        format!("{} {clock}", time.format("%-m/%-d/%Y"))
    }
}

/// The running timer counts whole seconds, so it doesn't flicker through tenths.
fn format_elapsed(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format_duration(Duration::from_secs(seconds))
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

/// The gray of a tool call's row: t3code's secondary label, the muted gray a quarter of the way
/// toward the background, dimmer than the agent's messages.
fn work_row_color(cx: &App) -> Hsla {
    let colors = cx.theme().colors();
    // Opaque, since a text highlight keeps its text's alpha and the shimmer's band would only
    // reach three quarters of the text color.
    mix(colors.text_muted, colors.panel_background, 0.25)
}

/// The soft background of the user's bubble: the thread's background a tenth of the way toward
/// its text, so it shows in light and dark themes alike. Most themes (One, Ayu, Gruvbox) give
/// elements the panel's background or one shade off it, so `element_background` would vanish.
fn user_message_background(cx: &App) -> Hsla {
    let colors = cx.theme().colors();
    colors.panel_background.blend(colors.text.opacity(0.1))
}

/// t3code's `live-tool-shine`: a band of the text color sweeps across a label in the row's
/// gray, every 2.2 seconds, while the work it names goes on.
fn shimmering_label(
    id: impl Into<ElementId>,
    text: &'static str,
    color: Hsla,
    cx: &App,
) -> impl IntoElement {
    let bright = cx.theme().colors().text;
    // Half the band's width, in characters: t3code's band is a little wider than "Thinking".
    const HALF_BAND: f32 = 4.;
    div().with_animation(
        id,
        Animation::new(Duration::from_millis(2200)).repeat(),
        move |label, delta| {
            let length = text.chars().count() as f32;
            let center = -HALF_BAND + delta * (length + 2. * HALF_BAND);
            let mut highlights = Vec::new();
            for (position, (byte, character)) in text.char_indices().enumerate() {
                let distance = (position as f32 + 0.5 - center).abs();
                let strength = (1. - distance / HALF_BAND).max(0.);
                let style = gpui::HighlightStyle {
                    color: Some(mix(color, bright, strength)),
                    ..Default::default()
                };
                highlights.push((byte..byte + character.len_utf8(), style));
            }
            label.child(gpui::StyledText::new(text).with_highlights(highlights))
        },
    )
}

/// The color `amount` of the way from `from` to `to`, alpha included: the row's gray is
/// translucent, so blending over it would leave the band translucent too.
fn mix(from: Hsla, to: Hsla, amount: f32) -> Hsla {
    let (from, to) = (gpui::Rgba::from(from), gpui::Rgba::from(to));
    let channel = |from: f32, to: f32| from + (to - from) * amount;
    Hsla::from(gpui::Rgba {
        r: channel(from.r, to.r),
        g: channel(from.g, to.g),
        b: channel(from.b, to.b),
        a: channel(from.a, to.a),
    })
}

/// Text for a one-line row: every run of whitespace, newlines included, as one space, as t3code's
/// `truncate`d rows show a multi-line command. GPUI starts a new line at each newline even in
/// truncated text.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether an entry is the agent's work between messages, which t3code groups: tool calls and
/// thoughts. The plan's marker draws nothing, so it doesn't split a run.
fn is_work(entry: &Entry) -> bool {
    matches!(
        entry,
        Entry::ToolCall(_) | Entry::AgentThought(_) | Entry::Plan
    )
}

/// The run of work the entry at `index` is in: every entry between the messages around it.
fn work_run(entries: &[Entry], index: usize) -> Option<Range<usize>> {
    if !entries.get(index).is_some_and(is_work) {
        return None;
    }
    let start = entries[..index]
        .iter()
        .rposition(|entry| !is_work(entry))
        .map_or(0, |position| position + 1);
    let end = entries[index..]
        .iter()
        .position(|entry| !is_work(entry))
        .map_or(entries.len(), |position| index + position);
    Some(start..end)
}

/// How many of a run's entries draw a row: the plan's marker doesn't.
fn shown_work(entries: &[Entry]) -> usize {
    entries
        .iter()
        .filter(|entry| !matches!(entry, Entry::Plan))
        .count()
}

/// The running turn's last run of work, folded into the row of one of its entries.
#[derive(Clone, Debug, PartialEq)]
struct LiveLine {
    run: Range<usize>,
    /// The entry whose row the line is.
    entry: usize,
    /// Whether that entry shows a permission request's buttons, which a request's coming and
    /// going doesn't mark as a change to the entry.
    awaits_confirmation: bool,
}

/// Where the last turn's entries start: after the user's last message.
fn current_turn_start(entries: &[Entry]) -> usize {
    entries
        .iter()
        .rposition(|entry| matches!(entry, Entry::UserMessage(_)))
        .map_or(0, |position| position + 1)
}

/// What a kind of tool call did, as t3code names it in a work group's summary.
#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkAction {
    Read,
    Edit,
    Command,
    CodeSearch,
    Other,
}

impl WorkAction {
    /// t3code's `toolGroupAction` for ACP's tool kinds.
    fn of(tool_call: &ToolCall) -> Self {
        match tool_call.kind {
            acp::ToolKind::Read => Self::Read,
            acp::ToolKind::Edit | acp::ToolKind::Delete | acp::ToolKind::Move => Self::Edit,
            acp::ToolKind::Execute => Self::Command,
            acp::ToolKind::Search => Self::CodeSearch,
            _ if !tool_call.diffs.is_empty() => Self::Edit,
            _ => Self::Other,
        }
    }

    /// Commands and edits lead the summary; tools it can't name come last.
    fn priority(self) -> u8 {
        match self {
            Self::Command | Self::Edit => 0,
            Self::Read | Self::CodeSearch => 1,
            Self::Other => 2,
        }
    }

    fn label(self, tool_calls: &[&ToolCall]) -> String {
        let plural = |count: usize, one: &str, many: &str| {
            format!("{count} {}", if count == 1 { one } else { many })
        };
        match self {
            Self::Read => format!("Read {}", plural(tool_calls.len(), "file", "files")),
            Self::Edit => {
                // Each file once, and an edit that names none as one.
                let mut paths = HashSet::default();
                let mut unnamed = 0;
                for tool_call in tool_calls {
                    if tool_call.diffs.is_empty() {
                        unnamed += 1;
                    }
                    paths.extend(tool_call.diffs.iter().map(|diff| diff.path.clone()));
                }
                format!("Changed {}", plural(paths.len() + unnamed, "file", "files"))
            }
            Self::Command => format!("Ran {}", plural(tool_calls.len(), "command", "commands")),
            Self::CodeSearch => {
                format!(
                    "Searched code {}",
                    plural(tool_calls.len(), "time", "times")
                )
            }
            Self::Other => format!("Used {}", plural(tool_calls.len(), "tool", "tools")),
        }
    }
}

/// What a run of work did, in t3code's words (`summarizeToolGroup`): at most two kinds of tool
/// call, commands and edits first, in the order they came, and a count of the rest ("Read 2
/// files, changed 2 files, and performed 2 other actions"). Thoughts count only when there's
/// nothing else.
fn summarize_work(entries: &[Entry]) -> String {
    let mut groups: Vec<(WorkAction, Vec<&ToolCall>)> = Vec::new();
    let mut thoughts = 0;
    for entry in entries {
        match entry {
            Entry::ToolCall(tool_call) => {
                let action = WorkAction::of(tool_call);
                match groups.iter_mut().find(|(other, _)| *other == action) {
                    Some((_, tool_calls)) => tool_calls.push(tool_call),
                    None => groups.push((action, vec![tool_call])),
                }
            }
            Entry::AgentThought(_) => thoughts += 1,
            _ => {}
        }
    }
    if groups.is_empty() {
        return if thoughts == 1 {
            "Thought".to_string()
        } else {
            format!("Thought (×{thoughts})")
        };
    }
    let mut selected: Vec<usize> = (0..groups.len()).collect();
    selected.sort_by_key(|&index| (groups[index].0.priority(), index));
    selected.truncate(2);
    selected.sort();
    let mut labels: Vec<String> = selected
        .iter()
        .map(|&index| groups[index].0.label(&groups[index].1))
        .collect();
    let total: usize = groups.iter().map(|(_, tool_calls)| tool_calls.len()).sum();
    let summarized: usize = selected.iter().map(|&index| groups[index].1.len()).sum();
    let remaining = total - summarized;
    if remaining > 0 {
        labels.push(format!(
            "Performed {remaining} other {}",
            if remaining == 1 { "action" } else { "actions" }
        ));
    }
    for label in labels.iter_mut().skip(1) {
        if let Some(first) = label.get(..1) {
            let lowered = first.to_lowercase();
            label.replace_range(..1, &lowered);
        }
    }
    match labels.as_slice() {
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] if !rest.is_empty() => format!("{}, and {last}", rest.join(", ")),
        _ => labels.concat(),
    }
}

#[cfg(test)]
mod tests {
    use agentz_protocol::spaces::SpacesSnapshot;
    use agentz_protocol::thread::QueuedMessage;
    use gpui::{TestAppContext, VisualTestContext};
    use projects::{Project, ProjectsSnapshot};

    use super::*;

    #[test]
    fn durations_read_like_t3codes() {
        let cases = [
            (Duration::from_millis(0), "1ms"),
            (Duration::from_millis(420), "420ms"),
            (Duration::from_millis(8_040), "8.0s"),
            (Duration::from_millis(9_960), "10s"),
            (Duration::from_secs(22), "22s"),
            (Duration::from_secs(60), "1m"),
            (Duration::from_secs(72), "1m 12s"),
            (Duration::from_secs(3_780), "1h 3m"),
        ];
        for (duration, expected) in cases {
            assert_eq!(format_duration(duration), expected);
        }
        assert_eq!(format_elapsed(Duration::from_millis(8_900)), "8s");
        assert_eq!(format_elapsed(Duration::from_secs(72)), "1m 12s");
    }

    #[test]
    fn message_times_name_the_day_when_not_today() {
        use chrono::TimeZone as _;
        let local = |year, month, day, hour, minute| -> SystemTime {
            chrono::Local
                .with_ymd_and_hms(year, month, day, hour, minute, 0)
                .single()
                .expect("a local time")
                .into()
        };
        let now = local(2026, 10, 5, 15, 0);
        assert_eq!(day_aware_time(local(2026, 10, 5, 9, 7), now), "09:07");
        assert_eq!(
            day_aware_time(local(2026, 10, 4, 23, 30), now),
            "yesterday at 23:30"
        );
        assert_eq!(
            day_aware_time(local(2026, 8, 13, 12, 34), now),
            "8/13 12:34"
        );
        assert_eq!(
            day_aware_time(local(2025, 12, 31, 8, 0), now),
            "12/31/2025 08:00"
        );
    }

    fn thread(id: u64, session_id: Option<&str>) -> projects::Thread {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "project_id": 1,
            "title": "New thread",
            "agent_id": "mock",
            "session_id": session_id,
        }))
        .expect("a thread")
    }

    fn snapshot(unsent_text: Option<&str>) -> ProjectsSnapshot {
        let mut typed = thread(3, None);
        typed.is_draft = true;
        typed.unsent_text = unsent_text.map(str::to_string);
        let mut workspaces_draft = thread(4, None);
        workspaces_draft.project_id = ProjectId::WORKSPACES;
        workspaces_draft.workspace = Some("/tmp/docs".into());
        workspaces_draft.is_draft = true;
        ProjectsSnapshot {
            projects: vec![Project {
                id: ProjectId(1),
                path: "/tmp/demo".into(),
                custom_name: None,
                icon: None,
                workspaces: Vec::new(),
                repository: None,
            }],
            threads: vec![
                thread(1, None),
                thread(2, Some("session")),
                typed,
                workspaces_draft,
            ],
            ..Default::default()
        }
    }

    fn open(
        thread_id: u64,
        is_archived: bool,
        cx: &mut TestAppContext,
    ) -> (Entity<AgentView>, &mut VisualTestContext) {
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            client.update(cx, |client, cx| client.set_online_for_test(cx));
            let projects = client.read(cx).projects().clone();
            projects.update(cx, |store, cx| {
                store.set_snapshot(snapshot(Some("Fix the login")), cx)
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            crate::project_info::init(cx);
            client
        });
        let (view, cx) = cx.add_window_view(|_, cx| {
            let thread_id = ThreadId(thread_id);
            let thread = AgentThread::shared(&client, thread_id, cx);
            let mut view = AgentView::new(
                thread_id,
                thread,
                "New thread".into(),
                Some(AgentId::new("mock")),
                cx,
            );
            view.set_archived(is_archived, cx);
            view
        });
        cx.run_until_parked();
        (view, cx)
    }

    #[gpui::test]
    fn a_thread_without_messages_opens_on_the_new_thread_screen(cx: &mut TestAppContext) {
        let (_, cx) = open(1, false, cx);
        assert!(cx.debug_bounds("new-thread").is_some());
    }

    #[gpui::test]
    fn a_workspaces_draft_works_in_its_folder(cx: &mut TestAppContext) {
        let (view, cx) = open(4, false, cx);
        assert!(cx.debug_bounds("thread-header-folder").is_some());
        assert!(cx.debug_bounds("new-thread-folder").is_some());
        assert!(cx.debug_bounds("new-thread-folder-menu").is_none());
        // In git, it can start in a new worktree or pasture instead.
        view.update(cx, |view, cx| {
            view.draft_git = Some(ProjectGit {
                is_repository: true,
                ..Default::default()
            });
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("new-thread-folder").is_none());
        assert!(cx.debug_bounds("new-thread-folder-menu").is_some());
    }

    /// With two accounts, the strip under the composer shows the draft's account, and picking
    /// another makes the draft again on it. Another checkout keeps the account; another agent
    /// takes its own account for new threads.
    #[gpui::test]
    fn the_strip_picks_the_new_threads_account(cx: &mut TestAppContext) {
        use agentz_protocol::accounts::{AccountStatus, AgentAccounts, LimitWindow, StatusRead};
        use std::cell::RefCell;

        let (view, cx) = open(1, false, cx);
        assert!(cx.debug_bounds("new-thread-account").is_none());

        let mock = AgentId::new("mock");
        let mut accounts = AgentAccounts {
            external_logged_in: Some(true),
            external_status: Some(StatusRead {
                status: AccountStatus {
                    email: Some("alex@hey.com".into()),
                    plan: Some("Max 5x".into()),
                    windows: vec![LimitWindow {
                        label: "5-hour".into(),
                        used_percent: 38.,
                        resets_at: None,
                        length: None,
                    }],
                    ..AccountStatus::default()
                },
                read_at: SystemTime::now(),
            }),
            ..AgentAccounts::default()
        };
        let work = accounts.add();
        if let Some(account) = accounts.account_mut(work) {
            account.choices.label = Some("Work".into());
            account.logged_in = Some(true);
        }
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let events: Rc<RefCell<Vec<(AgentId, bool)>>> = Rc::default();
        let client = view.read_with(cx, |view, _| view.client.clone());
        client.update(cx, |client, cx| {
            let requests = requests.clone();
            client.answer_for_test(move |request| {
                requests.borrow_mut().push(request.clone());
                match request {
                    Request::CreateThread { .. } => Some(Response::ThreadCreated(ThreadId(9))),
                    _ => None,
                }
            });
            client.set_accounts_for_test([(mock.clone(), accounts)].into(), cx);
        });
        cx.update(|_, cx| {
            let events = events.clone();
            cx.subscribe(&view, move |_, event: &AgentViewEvent, _| {
                if let AgentViewEvent::OpenAgentAccounts {
                    agent_id,
                    add_account,
                } = event
                {
                    events.borrow_mut().push((agent_id.clone(), *add_account));
                }
            })
            .detach();
        });
        cx.run_until_parked();
        let click = |selector: &'static str, cx: &mut VisualTestContext| {
            let bounds = cx
                .debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} is shown"));
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        let created = |account: AccountChoice, workspace: WorkspaceChoice| Request::CreateThread {
            project_id: ProjectId(1),
            agent_id: AgentId::new("mock"),
            workspace,
            account,
        };

        // The External account first, then agentZ's.
        click("new-thread-account", cx);
        let external = cx
            .debug_bounds("new-thread-account-external")
            .expect("the External account is offered");
        let on_work = cx
            .debug_bounds("new-thread-account-1")
            .expect("Work is offered");
        assert!(external.top() < on_work.top());
        click("new-thread-account-1", cx);
        assert!(requests.borrow().contains(&created(
            AccountChoice::Account(work),
            WorkspaceChoice::Checkout
        )));

        // A draft on Work keeps it in a new worktree.
        let store = view.read_with(cx, |view, _| view.store.clone());
        let mut on_work = snapshot(Some("Fix the login"));
        on_work.threads[0].account = Some(work);
        store.update(cx, |store, cx| store.set_snapshot(on_work, cx));
        cx.run_until_parked();
        requests.borrow_mut().clear();
        let worktree = WorkspaceChoice::New {
            kind: WorkspaceKind::Worktree,
            base: None,
            branch: None,
        };
        view.update(cx, |view, cx| {
            view.change_new_thread_checkout(worktree.clone(), cx)
        });
        cx.run_until_parked();
        assert!(
            requests
                .borrow()
                .contains(&created(AccountChoice::Account(work), worktree.clone()))
        );
        requests.borrow_mut().clear();
        view.update(cx, |view, cx| {
            view.change_new_thread_starter(Starter::Agent(AgentId::new("other")), cx)
        });
        cx.run_until_parked();
        assert!(requests.borrow().iter().any(|request| matches!(
            request,
            Request::CreateThread {
                account: AccountChoice::Default,
                ..
            }
        )));

        // Add Account… opens the agent's accounts to add one there.
        click("new-thread-account", cx);
        click("MENU_ITEM-Add Account…", cx);
        assert_eq!(*events.borrow(), vec![(mock, true)]);
    }

    /// With two accounts, Continue with Another Agent lists the thread's agent first with its
    /// accounts beneath it, and continues the thread on the one picked. The thread's own can't
    /// be picked.
    #[gpui::test]
    fn a_thread_continues_on_another_of_its_agents_accounts(cx: &mut TestAppContext) {
        use agentz_protocol::accounts::AgentAccounts;
        use std::cell::RefCell;

        let (view, cx) = open(2, false, cx);
        let mock = AgentId::new("mock");
        let mut accounts = AgentAccounts {
            external_logged_in: Some(true),
            ..AgentAccounts::default()
        };
        let work = accounts.add();
        if let Some(account) = accounts.account_mut(work) {
            account.choices.label = Some("Work".into());
            account.logged_in = Some(true);
        }
        let store = view.read_with(cx, |view, _| view.store.clone());
        let mut on_work = snapshot(None);
        on_work.threads[1].account = Some(work);
        store.update(cx, |store, cx| store.set_snapshot(on_work, cx));
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let client = view.read_with(cx, |view, _| view.client.clone());
        client.update(cx, |client, cx| {
            let requests = requests.clone();
            client.answer_for_test(move |request| {
                requests.borrow_mut().push(request.clone());
                match request {
                    Request::ContinueThread { .. } => Some(Response::ThreadCreated(ThreadId(9))),
                    _ => None,
                }
            });
            client.set_accounts_for_test([(mock.clone(), accounts)].into(), cx);
        });
        cx.run_until_parked();
        let continued = |requests: &RefCell<Vec<Request>>| {
            requests
                .borrow()
                .iter()
                .filter_map(|request| match request {
                    Request::ContinueThread {
                        thread_id,
                        agent_id,
                        account,
                    } => Some((*thread_id, agent_id.clone(), *account)),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };

        view.update_in(cx, |view, window, cx| view.title_menu.show(window, cx));
        cx.run_until_parked();
        // The submenu opens on hovering the row under Rename. It's placed beside the row once a
        // frame has measured the row, which the pointer moving on draws.
        let rename = cx
            .debug_bounds("MENU_ITEM-Rename")
            .expect("the menu is open");
        for nudge in [px(0.), px(4.)] {
            cx.simulate_mouse_move(
                rename.center() + gpui::point(nudge, rename.size.height),
                None,
                gpui::Modifiers::none(),
            );
            cx.run_until_parked();
        }
        let external = cx
            .debug_bounds("continue-on-account-external")
            .expect("the External account is offered");
        let own = cx
            .debug_bounds("continue-on-account-1")
            .expect("the thread's own account is shown");
        assert!(external.top() < own.top());

        cx.simulate_click(own.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(continued(&requests).is_empty());
        cx.simulate_click(external.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            continued(&requests),
            vec![(ThreadId(2), mock, AccountChoice::External)]
        );
    }

    /// A thread that stopped while its account's last read has a window used up shows the limit
    /// notice in place of the agent's error. It continues on the account with the most left,
    /// its arrow lists the others, and once closed it stays closed until the next turn.
    #[gpui::test]
    fn a_thread_stopped_at_its_accounts_limit_offers_another_account(cx: &mut TestAppContext) {
        use agentz_protocol::accounts::{AccountStatus, StatusRead};
        use std::cell::RefCell;

        let (view, cx) = open(2, false, cx);
        let mock = AgentId::new("mock");
        let now = SystemTime::now();
        let read = |used_percent: f64| StatusRead {
            status: AccountStatus {
                windows: vec![LimitWindow {
                    label: "5-hour".into(),
                    used_percent,
                    resets_at: Some(now + Duration::from_secs(2 * 60 * 60)),
                    length: None,
                }],
                ..AccountStatus::default()
            },
            read_at: now,
        };
        let mut accounts = AgentAccounts {
            external_logged_in: Some(true),
            external_status: Some(read(60.)),
            ..AgentAccounts::default()
        };
        let work = accounts.add();
        let side = accounts.add();
        for (id, label, used_percent) in [(work, "Work", 100.), (side, "Side", 3.)] {
            if let Some(account) = accounts.account_mut(id) {
                account.choices.label = Some(label.into());
                account.logged_in = Some(true);
                account.status = Some(read(used_percent));
            }
        }
        let store = view.read_with(cx, |view, _| view.store.clone());
        let mut on_work = snapshot(None);
        on_work.threads[1].account = Some(work);
        store.update(cx, |store, cx| store.set_snapshot(on_work, cx));
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let client = view.read_with(cx, |view, _| view.client.clone());
        client.update(cx, |client, cx| {
            let requests = requests.clone();
            client.answer_for_test(move |request| {
                requests.borrow_mut().push(request.clone());
                match request {
                    Request::ContinueThread { .. } => Some(Response::ThreadCreated(ThreadId(9))),
                    _ => None,
                }
            });
            client.set_accounts_for_test([(mock.clone(), accounts.clone())].into(), cx);
        });
        cx.run_until_parked();
        let continued = |requests: &RefCell<Vec<Request>>| {
            requests
                .borrow()
                .iter()
                .filter_map(|request| match request {
                    Request::ContinueThread { account, .. } => Some(*account),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let at_limit = |view: &Entity<AgentView>, cx: &mut VisualTestContext| {
            view.read_with(cx, |view, cx| {
                view.limit_reached(SystemTime::now(), cx).is_some()
            })
        };

        // A used-up account alone isn't a stop.
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(vec![Entry::UserMessage("Fix the login".into())], cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("limit-notice").is_none());
        thread.update(cx, |thread, cx| {
            thread.set_turn_error_for_test("Usage limit reached", cx)
        });
        cx.run_until_parked();
        assert!(at_limit(&view, cx));
        assert!(cx.debug_bounds("limit-notice").is_some());

        let primary = cx
            .debug_bounds("limit-continue")
            .expect("Continue on Side is offered");
        cx.simulate_click(
            primary.origin + gpui::point(px(12.), primary.size.height / 2.),
            gpui::Modifiers::none(),
        );
        cx.run_until_parked();
        assert_eq!(continued(&requests), vec![AccountChoice::Account(side)]);

        let others = cx
            .debug_bounds("limit-continue-others")
            .expect("the other accounts have an arrow");
        cx.simulate_click(others.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("limit-continue-on-2").is_none());
        assert!(cx.debug_bounds("limit-continue-on-1").is_none());
        let external = cx
            .debug_bounds("limit-continue-on-external")
            .expect("the External account is listed");
        cx.simulate_click(external.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(
            continued(&requests),
            vec![AccountChoice::Account(side), AccountChoice::External]
        );

        // Closed, it comes back with the next turn's stop.
        let dismiss = cx
            .debug_bounds("limit-notice-dismiss")
            .expect("the notice can be closed");
        cx.simulate_click(dismiss.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("limit-notice").is_none());
        for working in [true, false] {
            thread.update(cx, |thread, cx| thread.set_working_for_test(working, cx));
            cx.run_until_parked();
        }
        assert!(cx.debug_bounds("limit-notice").is_some());

        // Once a read finds room again, the error is the agent's own.
        if let Some(account) = accounts.account_mut(work) {
            account.status = Some(read(40.));
        }
        client.update(cx, |client, cx| {
            client.set_accounts_for_test([(mock, accounts)].into(), cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("limit-notice").is_none());
        assert!(!at_limit(&view, cx));
    }

    /// The composer gauges the thread's account by its window closest to running out, and opens
    /// that account's windows. A logged-out account's last read isn't gauged.
    #[gpui::test]
    fn the_composer_gauges_the_threads_account(cx: &mut TestAppContext) {
        use agentz_protocol::accounts::{AccountStatus, StatusRead};

        let (view, cx) = open(2, false, cx);
        let mock = AgentId::new("mock");
        let now = SystemTime::now();
        let hour = Duration::from_secs(60 * 60);
        let window = |label: &str, used_percent: f64, length: Duration| LimitWindow {
            label: label.into(),
            used_percent,
            resets_at: Some(now + 2 * hour),
            length: Some(length),
        };
        let read = |windows: Vec<LimitWindow>| StatusRead {
            status: AccountStatus {
                plan: Some("Team".into()),
                windows,
                ..AccountStatus::default()
            },
            read_at: now,
        };
        let mut accounts = AgentAccounts {
            external_logged_in: Some(true),
            external_status: Some(read(vec![window("5-hour", 10., 5 * hour)])),
            ..AgentAccounts::default()
        };
        let work = accounts.add();
        if let Some(account) = accounts.account_mut(work) {
            account.choices.label = Some("Work".into());
            account.logged_in = Some(true);
            account.status = Some(read(vec![
                window("5-hour", 38., 5 * hour),
                window("Weekly", 91., 168 * hour),
            ]));
        }
        let store = view.read_with(cx, |view, _| view.store.clone());
        let mut on_work = snapshot(None);
        on_work.threads[1].account = Some(work);
        store.update(cx, |store, cx| store.set_snapshot(on_work, cx));
        let client = view.read_with(cx, |view, _| view.client.clone());
        client.update(cx, |client, cx| {
            client.set_accounts_for_test([(mock.clone(), accounts.clone())].into(), cx)
        });
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(vec![Entry::UserMessage("Fix the login".into())], cx)
        });
        cx.run_until_parked();

        let gauged = |cx: &mut VisualTestContext| {
            view.read_with(cx, |view, cx| {
                view.gauged_window(cx)
                    .map(|window| (window.label.clone(), window.left_percent()))
            })
        };
        assert_eq!(gauged(cx), Some(("Weekly".into(), 9)));
        let gauge = cx
            .debug_bounds("usage-gauge")
            .expect("the composer has the gauge");
        cx.simulate_click(gauge.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("usage-gauge-popover").is_some());
        assert!(cx.debug_bounds("limit-gauge-window-0").is_some());
        assert!(cx.debug_bounds("limit-gauge-window-1").is_some());
        // Escape closes it without stopping the thread. The popover takes focus once drawn.
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        thread.update(cx, |thread, cx| thread.set_working_for_test(true, cx));
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(cx.debug_bounds("usage-gauge-popover").is_none());
        assert!(thread.read_with(cx, |thread, _| thread.is_working()));
        thread.update(cx, |thread, cx| thread.set_working_for_test(false, cx));

        if let Some(account) = accounts.account_mut(work) {
            account.logged_in = Some(false);
        }
        client.update(cx, |client, cx| {
            client.set_accounts_for_test([(mock, accounts)].into(), cx)
        });
        cx.run_until_parked();
        assert_eq!(gauged(cx), None);
        assert!(cx.debug_bounds("usage-gauge").is_none());
    }

    #[test]
    fn the_limit_notice_names_the_window_and_its_reset() {
        let now = SystemTime::now();
        let window = LimitWindow {
            label: "Weekly".into(),
            used_percent: 100.,
            resets_at: Some(now + Duration::from_secs(112 * 60 + 30)),
            length: None,
        };
        let (title, body) = limit_notice_text(Some("Work"), "Claude Agent", &window, now);
        assert_eq!(title, "Work reached its weekly limit");
        assert!(body.starts_with("Claude Agent stopped. The limit resets "));
        assert!(body.ends_with(", in 1h 52m."));
        let (title, body) = limit_notice_text(
            None,
            "Claude Agent",
            &LimitWindow {
                label: "GPT-5 5-hour".into(),
                resets_at: None,
                ..window
            },
            now,
        );
        assert_eq!(title, "Your account reached its GPT-5 5-hour limit");
        assert_eq!(body, "Claude Agent stopped.");
    }

    /// An archived thread takes no messages, so there's nothing to start.
    #[gpui::test]
    fn an_archived_thread_shows_its_conversation(cx: &mut TestAppContext) {
        let (_, cx) = open(2, true, cx);
        assert!(cx.debug_bounds("new-thread").is_none());
    }

    #[gpui::test]
    fn options_show_as_loading_while_the_agent_starts(cx: &mut TestAppContext) {
        let (view, cx) = open(1, false, cx);
        let set_status = |status: ConnectionStatus, cx: &mut VisualTestContext| {
            let thread = view.read_with(cx, |view, _| view.thread.clone());
            thread.update(cx, |thread, cx| thread.set_status_for_test(status, cx));
            cx.run_until_parked();
        };
        set_status(ConnectionStatus::Connecting, cx);
        assert!(cx.debug_bounds("new-thread").is_some());
        assert!(cx.debug_bounds("session-settings-loading").is_some());
        set_status(ConnectionStatus::Ready, cx);
        assert!(cx.debug_bounds("session-settings-loading").is_none());
    }

    #[gpui::test]
    fn unsent_text_comes_back_and_goes_when_discarded(cx: &mut TestAppContext) {
        let (view, cx) = open(3, false, cx);
        let composer_text = |cx: &mut VisualTestContext| {
            view.read_with(cx, |view, cx| view.composer.read(cx).text().to_string())
        };
        assert_eq!(composer_text(cx), "Fix the login");
        assert!(view.read_with(cx, |view, cx| view.is_draft(cx)));
        assert!(!view.read_with(cx, |view, cx| view.is_untouched_draft(cx)));

        // Another window's save leaves this composer alone; a discard empties it.
        let store = view.read_with(cx, |view, _| view.store.clone());
        store.update(cx, |store, cx| {
            store.set_snapshot(snapshot(Some("Fix the login page")), cx)
        });
        cx.run_until_parked();
        assert_eq!(composer_text(cx), "Fix the login");
        store.update(cx, |store, cx| store.set_snapshot(snapshot(None), cx));
        cx.run_until_parked();
        assert_eq!(composer_text(cx), "");
        assert!(view.read_with(cx, |view, cx| view.is_untouched_draft(cx)));
    }

    #[gpui::test]
    fn the_composer_takes_several_lines_and_has_a_menu(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let focus = view.read_with(cx, |view, cx| view.composer.focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_input("one");
        cx.simulate_keystrokes("shift-enter");
        cx.simulate_input("two");
        let text = view.read_with(cx, |view, cx| view.composer.read(cx).text().to_string());
        assert_eq!(text, "one\ntwo");

        cx.run_until_parked();
        let bounds = cx.debug_bounds("composer").expect("the composer");
        let position = gpui::point(bounds.left() + px(4.), bounds.top() + px(8.));
        cx.simulate_mouse_down(position, gpui::MouseButton::Right, gpui::Modifiers::none());
        cx.run_until_parked();
        let menu = view.read_with(cx, |view, _| {
            view.composer_menu.as_ref().map(|(menu, _, _)| menu.clone())
        });
        assert!(menu.is_some());
    }

    #[gpui::test]
    fn enter_makes_a_new_line_when_a_modifier_sends(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        cx.update(|_, cx| {
            crate::app_settings::AppSettingsStore::global(cx).update(cx, |store, cx| {
                store.update(|settings| settings.use_modifier_to_send = true, cx)
            })
        });
        let focus = view.read_with(cx, |view, cx| view.composer.focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.run_until_parked();
        cx.simulate_input("one");
        cx.simulate_keystrokes("enter");
        cx.simulate_input("two");
        let text = view.read_with(cx, |view, cx| view.composer.read(cx).text().to_string());
        assert_eq!(text, "one\ntwo");
    }

    #[gpui::test]
    fn at_mentions_files_and_threads(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let client = view.read_with(cx, |view, _| view.client.clone());
        client.update(cx, |client, _| {
            client.answer_for_test(|request| match request {
                Request::ListFiles(_) => Some(Response::Files(agentz_protocol::FileListing {
                    root: "/tmp/demo".into(),
                    entries: ["src/total.ts", "README.md"]
                        .into_iter()
                        .map(|path| agentz_protocol::FileEntry {
                            path: path.into(),
                            is_dir: false,
                        })
                        .collect(),
                })),
                _ => None,
            })
        });
        let focus = view.read_with(cx, |view, cx| view.composer.focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_input("look at @tot");
        cx.run_until_parked();
        let labels = view.read_with(cx, |view, _| {
            view.mention_matches
                .iter()
                .map(|found| found.label.to_string())
                .collect::<Vec<_>>()
        });
        assert_eq!(labels, ["total.ts"]);
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        // Threads too: thread 1 is the project's other thread.
        cx.simulate_input("and @new");
        cx.run_until_parked();
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();

        let (text, prompt) = view.read_with(cx, |view, cx| {
            (
                view.composer.read(cx).plain_text(),
                view.composer_prompt(cx),
            )
        });
        assert_eq!(text, "look at @total.ts and @New thread ");
        assert_eq!(
            prompt,
            [
                PromptPart::Text("look at ".into()),
                PromptPart::Path("/tmp/demo/src/total.ts".into()),
                PromptPart::Text(" and ".into()),
                PromptPart::Thread(ThreadId(1)),
                PromptPart::Text(" ".into()),
            ]
        );
    }

    /// A long thread draws only the rows in view, at its end, and follows new output.
    #[gpui::test]
    fn a_long_thread_draws_only_the_rows_in_view(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let entries: Vec<Entry> = (0..300)
            .map(|index| Entry::AgentMessage(format!("Message {index}")))
            .collect();
        thread.update(cx, |thread, cx| thread.set_entries_for_test(entries, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("conversation-row-300").is_some());
        assert!(cx.debug_bounds("conversation-row-1").is_none());

        let mut entries: Vec<Entry> = (0..301)
            .map(|index| Entry::AgentMessage(format!("Message {index}")))
            .collect();
        entries.push(Entry::AgentMessage("The newest".into()));
        thread.update(cx, |thread, cx| thread.set_entries_for_test(entries, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("conversation-row-302").is_some());
    }

    fn tool_call(status: acp::ToolCallStatus) -> Entry {
        Entry::ToolCall(ToolCall {
            id: acp::ToolCallId::new("call-1"),
            title: "`npm test`".into(),
            kind: acp::ToolKind::Execute,
            status,
            text: vec!["PASS src/cart/total.test.ts".into()],
            diffs: Vec::new(),
            locations: Vec::new(),
            raw_input: None,
            terminals: Vec::new(),
            images: Vec::new(),
        })
    }

    /// t3code's compact row: closed until clicked, then its output shows under it.
    #[gpui::test]
    fn a_tool_call_is_a_row_that_opens_to_its_output(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(
                vec![
                    Entry::UserMessage("Run the tests".into()),
                    tool_call(acp::ToolCallStatus::Completed),
                ],
                cx,
            )
        });
        cx.run_until_parked();
        let row = cx
            .debug_bounds("tool-call-row-1")
            .expect("the tool call's row");
        assert!(cx.debug_bounds("tool-call-output-1").is_none());
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("tool-call-output-1").is_some());

        // A command over several lines still takes one.
        let mut multi_line = tool_call(acp::ToolCallStatus::Completed);
        if let Entry::ToolCall(tool_call) = &mut multi_line {
            tool_call.id = acp::ToolCallId::new("call-3");
            tool_call.title = "cd src &&\n  npm test \\\n    --watch".into();
        }
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(
                vec![Entry::UserMessage("Run the tests".into()), multi_line],
                cx,
            )
        });
        cx.run_until_parked();
        let multi_line_row = cx
            .debug_bounds("tool-call-row-1")
            .expect("the multi-line command's row");
        assert_eq!(multi_line_row.size.height, row.size.height);

        // An edit's row counts its lines.
        let edit = Entry::ToolCall(ToolCall {
            id: acp::ToolCallId::new("call-2"),
            title: "Edit total.ts".into(),
            kind: acp::ToolKind::Edit,
            status: acp::ToolCallStatus::Completed,
            text: Vec::new(),
            diffs: vec![FileDiff {
                path: "/tmp/demo/total.ts".into(),
                old_text: Some("a\nb\n".into()),
                new_text: "a\nc\nd\n".into(),
            }],
            locations: Vec::new(),
            raw_input: None,
            terminals: Vec::new(),
            images: Vec::new(),
        });
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(vec![Entry::UserMessage("Edit it".into()), edit], cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("tool-call-row-1").is_some());
    }

    #[test]
    fn a_read_file_is_one_code_block() {
        assert_eq!(
            as_code_block("<!doctype html>\n<html lang=\"en\">\n"),
            "```\n<!doctype html>\n<html lang=\"en\">\n```"
        );
        // A fence inside the file doesn't end the block.
        assert_eq!(
            as_code_block("# Notes\n```rust\nfn main() {}\n```"),
            "````\n# Notes\n```rust\nfn main() {}\n```\n````"
        );
        // Claude's read is fenced already.
        let fenced = "```\n1\tfn main() {}\n```";
        assert_eq!(as_code_block(fenced), fenced);
        // A markdown file that opens and ends with code isn't one block.
        assert_eq!(
            as_code_block("```\na\n```\nText\n```\nb\n```"),
            "````\n```\na\n```\nText\n```\nb\n```\n````"
        );
        assert_eq!(as_code_block(""), "");
    }

    #[gpui::test]
    fn a_read_shows_the_file_as_code(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let mut read = tool_call(acp::ToolCallStatus::Completed);
        if let Entry::ToolCall(tool_call) = &mut read {
            tool_call.kind = acp::ToolKind::Read;
            tool_call.title = "Read /tmp/demo/index.html".into();
            tool_call.text = vec!["<!doctype html>\n<html lang=\"en\">".into()];
        }
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(vec![Entry::UserMessage("Read it".into()), read], cx)
        });
        cx.run_until_parked();
        let source = view.read_with(cx, |view, cx| {
            view.markdowns
                .get(&(1, 1))
                .map(|markdown| markdown.read(cx).source().to_string())
        });
        assert_eq!(
            source.as_deref(),
            Some("```\n<!doctype html>\n<html lang=\"en\">\n```")
        );
    }

    fn work(id: &str, kind: acp::ToolKind, path: Option<&str>) -> Entry {
        Entry::ToolCall(ToolCall {
            id: acp::ToolCallId::new(id.to_string()),
            title: id.to_string(),
            kind,
            status: acp::ToolCallStatus::Completed,
            text: Vec::new(),
            diffs: path
                .map(|path| FileDiff {
                    path: path.into(),
                    old_text: None,
                    new_text: "a\n".into(),
                })
                .into_iter()
                .collect(),
            locations: Vec::new(),
            raw_input: None,
            terminals: Vec::new(),
            images: Vec::new(),
        })
    }

    /// t3code's summary of a work group: two kinds, commands and edits first, then the rest.
    #[test]
    fn a_run_of_work_is_summarized_as_t3code_does() {
        use acp::ToolKind::{Edit, Execute, Other, Read, Search};
        let commands: Vec<Entry> = (0..5)
            .map(|index| work(&format!("run-{index}"), Execute, None))
            .collect();
        assert_eq!(summarize_work(&commands), "Ran 5 commands");
        assert_eq!(
            summarize_work(&[
                work("read-1", Read, None),
                work("read-2", Read, None),
                work("search", Search, None),
            ]),
            "Read 2 files and searched code 1 time"
        );
        // An edited file counts once.
        assert_eq!(
            summarize_work(&[
                work("read-1", Read, None),
                work("edit-1", Edit, Some("/a.ts")),
                work("edit-2", Edit, Some("/a.ts")),
                work("edit-3", Edit, Some("/b.ts")),
                work("read-2", Read, None),
                work("run", Execute, None),
                work("search", Search, None),
                work("other", Other, None),
            ]),
            "Changed 2 files, ran 1 command, and performed 4 other actions"
        );
        assert_eq!(
            summarize_work(&[
                Entry::AgentThought("Hmm".into()),
                work("read", Read, None),
                work("read-2", Read, None),
            ]),
            "Read 2 files"
        );
        assert_eq!(
            summarize_work(&[
                Entry::AgentThought("Hmm".into()),
                Entry::AgentThought("Ah".into())
            ]),
            "Thought (×2)"
        );
    }

    /// The thread round's fold: while the turn runs, a run shows only its latest row, and once
    /// the agent writes after it, or the turn ends, it folds into a line that opens to its rows.
    #[gpui::test]
    fn work_folds_into_a_line_once_a_message_follows(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(
                vec![
                    Entry::UserMessage("Run the tests".into()),
                    work("run-1", acp::ToolKind::Execute, None),
                    work("run-2", acp::ToolKind::Execute, None),
                ],
                cx,
            );
            thread.set_working_for_test(true, cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("work-run-1").is_none());
        assert!(cx.debug_bounds("tool-call-row-1").is_none());
        assert!(cx.debug_bounds("tool-call-row-2").is_some());

        // Mid-turn, a message folds the run before it.
        thread.update(cx, |thread, cx| {
            thread.push_entry_for_test(Entry::AgentMessage("They pass.".into()), cx)
        });
        cx.run_until_parked();
        let header = cx.debug_bounds("work-run-1").expect("the folded run");
        assert!(cx.debug_bounds("tool-call-row-1").is_none());
        assert!(cx.debug_bounds("tool-call-row-2").is_none());

        cx.simulate_click(header.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("tool-call-row-1").is_some());
        assert!(cx.debug_bounds("tool-call-row-2").is_some());

        // The run after it shows its latest row until the turn ends.
        thread.update(cx, |thread, cx| {
            thread.push_entry_for_test(work("run-3", acp::ToolKind::Execute, None), cx);
            thread.push_entry_for_test(work("run-4", acp::ToolKind::Execute, None), cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("work-run-4").is_none());
        assert!(cx.debug_bounds("tool-call-row-4").is_none());
        assert!(cx.debug_bounds("tool-call-row-5").is_some());
        thread.update(cx, |thread, cx| thread.set_working_for_test(false, cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("work-run-4").is_some());
        assert!(cx.debug_bounds("tool-call-row-5").is_none());
    }

    /// A run opens with its rows closed, also those the user opened before it folded.
    #[gpui::test]
    fn a_run_opens_with_its_rows_closed(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let command = |id: &str| {
            let mut entry = tool_call(acp::ToolCallStatus::Completed);
            if let Entry::ToolCall(tool_call) = &mut entry {
                tool_call.id = acp::ToolCallId::new(id.to_string());
            }
            entry
        };
        let click = |name: &'static str, cx: &mut VisualTestContext| {
            let bounds = cx.debug_bounds(name).expect(name);
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(
                vec![
                    Entry::UserMessage("Run the tests".into()),
                    command("run-1"),
                    command("run-2"),
                    Entry::AgentThought("Both pass.".into()),
                ],
                cx,
            );
            thread.set_working_for_test(true, cx);
        });
        cx.run_until_parked();

        // Opened from the live line, its rows opened, and then the agent writes: the run stays
        // open as it folds, and opens again with its rows closed.
        click("thinking-row-3", cx);
        assert!(cx.debug_bounds("work-run-1").is_some());
        click("tool-call-row-1", cx);
        click("thinking-row-3", cx);
        assert!(cx.debug_bounds("tool-call-output-1").is_some());
        assert!(cx.debug_bounds("thinking-content-3").is_some());
        thread.update(cx, |thread, cx| {
            thread.push_entry_for_test(Entry::AgentMessage("They pass.".into()), cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("tool-call-output-1").is_some());
        click("work-run-1", cx);
        assert!(cx.debug_bounds("tool-call-row-1").is_none());
        click("work-run-1", cx);
        assert!(cx.debug_bounds("tool-call-row-1").is_some());
        assert!(cx.debug_bounds("tool-call-output-1").is_none());
        assert!(cx.debug_bounds("thinking-content-3").is_none());

        // Opened inside the open run, which is then folded and opened again.
        click("tool-call-row-2", cx);
        assert!(cx.debug_bounds("tool-call-output-2").is_some());
        click("work-run-1", cx);
        assert!(cx.debug_bounds("tool-call-row-2").is_none());
        click("work-run-1", cx);
        assert!(cx.debug_bounds("tool-call-row-2").is_some());
        assert!(cx.debug_bounds("tool-call-output-2").is_none());
    }

    /// t3code's `work-live` row: the running turn's last run is one line, the tool call awaiting
    /// confirmation, else the latest one running, else the latest entry; it opens to the run.
    #[gpui::test]
    fn the_running_turns_last_run_is_one_live_line(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let click = |name: &'static str, cx: &mut VisualTestContext| {
            let bounds = cx.debug_bounds(name).expect(name);
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        let mut running = work("run-1", acp::ToolKind::Execute, None);
        if let Entry::ToolCall(tool_call) = &mut running {
            tool_call.status = acp::ToolCallStatus::InProgress;
        }
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(
                vec![
                    Entry::UserMessage("Run the tests".into()),
                    running,
                    work("read-1", acp::ToolKind::Read, None),
                ],
                cx,
            );
            thread.set_working_for_test(true, cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("tool-call-row-1").is_some());
        assert!(cx.debug_bounds("tool-call-row-2").is_none());

        // Open, it's the run's line and its rows; folded again, the live line.
        click("tool-call-row-1", cx);
        assert!(cx.debug_bounds("work-run-1").is_some());
        assert!(cx.debug_bounds("tool-call-row-2").is_some());
        click("work-run-1", cx);
        assert!(cx.debug_bounds("work-run-1").is_none());
        assert!(cx.debug_bounds("tool-call-row-2").is_none());

        // A thought as the latest entry is the agent thinking.
        thread.update(cx, |thread, cx| {
            thread.push_entry_for_test(Entry::AgentThought("Next, the build.".into()), cx)
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("thinking-row-3").is_some());
        assert!(cx.debug_bounds("tool-call-row-1").is_none());

        // A request for permission shows its buttons on the line.
        thread.update(cx, |thread, cx| {
            thread.set_permission_requests_for_test(
                vec![agentz_protocol::thread::PermissionRequest {
                    tool_call_id: acp::ToolCallId::new("read-1"),
                    title: "Read the file".into(),
                    options: vec![agentz_protocol::thread::PermissionOption {
                        id: acp::PermissionOptionId::new("allow"),
                        name: "Allow".into(),
                        kind: acp::PermissionOptionKind::AllowOnce,
                    }],
                }],
                cx,
            )
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("tool-call-row-2").is_some());
        assert!(cx.debug_bounds("permission-buttons-2").is_some());
        assert!(cx.debug_bounds("thinking-row-3").is_none());
    }

    /// The rows above the view are measured before they're drawn, while their messages may
    /// still be parsing; scrolling up to them must move the conversation by the scroll alone.
    #[gpui::test]
    fn scrolling_up_moves_the_rows_evenly(cx: &mut TestAppContext) {
        const STEP: f32 = 12.;
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let messages = [
            "Now the tests.",
            "Now the requests that start an agent take the account, and the server keeps \
             them apart across restarts and machines.",
            "This failure is in a terminal test the change doesn't touch: the screen showed \
             the echoed command before the pid was printed, so it's rerun to see whether it's \
             flaky, and the rest of the suite runs once it passes.",
        ];
        let mut entries = vec![Entry::UserMessage("Go".into())];
        let mut headers = Vec::new();
        for run in 0..80 {
            entries.push(Entry::AgentMessage(messages[run % messages.len()].into()));
            headers.push(&*format!("work-run-{}", entries.len()).leak());
            entries.push(work(&format!("run-{run}"), acp::ToolKind::Execute, None));
            entries.push(work(&format!("read-{run}"), acp::ToolKind::Read, None));
        }
        entries.push(Entry::AgentMessage("Done.".into()));
        thread.update(cx, |thread, cx| thread.set_entries_for_test(entries, cx));
        cx.run_until_parked();

        let tops = |cx: &mut VisualTestContext| -> Vec<Option<f32>> {
            headers
                .iter()
                .map(|name| cx.debug_bounds(name).map(|bounds| f32::from(bounds.top())))
                .collect()
        };
        let mut before = tops(cx);
        // Past the 2048 pixels above the first view, which were measured with it.
        for step in 0..200 {
            cx.simulate_event(gpui::ScrollWheelEvent {
                position: gpui::point(px(400.), px(300.)),
                delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(STEP))),
                ..Default::default()
            });
            cx.run_until_parked();
            let after = tops(cx);
            for (header, (before, after)) in headers.iter().zip(before.iter().zip(&after)) {
                if let (Some(before), Some(after)) = (before, after) {
                    assert_eq!(after - before, STEP, "{header} at step {step}");
                }
            }
            before = after;
        }
        let top = view.read_with(cx, |view, _| view.list_state.logical_scroll_top());
        assert!(top.item_ix > 0, "the scroll stopped at the top");
    }

    /// An open tool call's output scrolls on its own: over it, the conversation stays put until
    /// the output reaches its end.
    #[gpui::test]
    fn scrolling_over_a_tool_calls_output_scrolls_only_the_output(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let mut read = tool_call(acp::ToolCallStatus::Completed);
        if let Entry::ToolCall(tool_call) = &mut read {
            tool_call.kind = acp::ToolKind::Read;
            tool_call.title = "Read /tmp/demo/total.ts".into();
            tool_call.text = vec![
                (1..=200)
                    .map(|line| format!("const line{line} = {line};"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ];
        }
        let mut entries = vec![Entry::UserMessage("Read it".into())];
        entries.extend((0..30).map(|index| Entry::AgentMessage(format!("Message {index}"))));
        entries.push(read);
        entries.push(Entry::AgentMessage("Done.".into()));
        thread.update(cx, |thread, cx| thread.set_entries_for_test(entries, cx));
        cx.run_until_parked();
        let row = cx.debug_bounds("tool-call-row-31").expect("the read's row");
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        cx.run_until_parked();

        let scroll = |name: &'static str, delta: f32, cx: &mut VisualTestContext| {
            let bounds = cx.debug_bounds(name).expect(name);
            cx.simulate_event(gpui::ScrollWheelEvent {
                position: bounds.center(),
                delta: gpui::ScrollDelta::Pixels(gpui::point(px(0.), px(delta))),
                ..Default::default()
            });
            cx.run_until_parked();
        };
        let row_top = |cx: &mut VisualTestContext| {
            f32::from(
                cx.debug_bounds("tool-call-row-31")
                    .expect("the read's row")
                    .top(),
            )
        };
        // Up from the end, so the conversation can scroll both ways.
        scroll("conversation-row-33", 100., cx);
        let top = row_top(cx);
        scroll("tool-call-output-31", -40., cx);
        assert_eq!(
            row_top(cx),
            top,
            "the conversation scrolled with the output"
        );
        // Back at the output's top, the scroll goes on to the conversation.
        scroll("tool-call-output-31", 40., cx);
        assert_eq!(row_top(cx), top);
        scroll("tool-call-output-31", 40., cx);
        assert_eq!(row_top(cx), top + 40.);
    }

    /// t3code's reasoning row: closed, opening on click, unless "Show thinking" is on.
    #[gpui::test]
    fn thoughts_are_closed_unless_show_thinking_is_on(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(
                vec![
                    Entry::UserMessage("Total it".into()),
                    Entry::AgentThought("Sum first, then round once.".into()),
                ],
                cx,
            );
            thread.set_working_for_test(true, cx);
        });
        cx.run_until_parked();
        let row = cx
            .debug_bounds("thinking-row-1")
            .expect("the thought's row");
        assert!(cx.debug_bounds("thinking-content-1").is_none());
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("thinking-content-1").is_some());
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("thinking-content-1").is_none());

        cx.update(|_, cx| {
            crate::app_settings::AppSettingsStore::global(cx).update(cx, |store, cx| {
                store.update(|settings| settings.show_thinking = true, cx)
            })
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("thinking-content-1").is_some());
    }

    const TINY_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    fn image_id() -> AttachmentId {
        AttachmentId::parse(&format!("{}.png", "ab".repeat(32))).expect("an id")
    }

    /// A 100 by 2000 pixel PNG.
    const TALL_PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAGQAAAfQAQAAAAAcZccUAAAAMklEQVR42u3BMQEAAADCoPVPbQ0PoAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD4MbWAAAVQlHfcAAAAASUVORK5CYII=";

    /// Answers for the images the view fetches, and for one it keeps.
    fn serve_images(client: &Entity<ServerClient>, cx: &mut VisualTestContext) {
        serve_image(client, TINY_PNG, cx);
    }

    /// Answers every image the view fetches with this PNG.
    fn serve_image(client: &Entity<ServerClient>, png: &'static str, cx: &mut VisualTestContext) {
        client.update(cx, |client, _| {
            client.answer_for_test(move |request| match request {
                Request::Attachment { .. } => Some(Response::AttachmentData(
                    agentz_protocol::attachments::AttachmentData {
                        mime_type: "image/png".into(),
                        data: png.into(),
                    },
                )),
                Request::AddAttachment { .. } => Some(Response::Attachment(image_id())),
                _ => None,
            })
        });
    }

    fn sent(client: &Entity<ServerClient>, cx: &mut VisualTestContext) -> Vec<Request> {
        client.read_with(cx, |client, _| client.sent_for_test())
    }

    /// A message typed while the agent works goes to the queue the server keeps.
    #[gpui::test]
    fn messages_typed_while_the_agent_works_queue_on_the_server(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let client = view.read_with(cx, |view, _| view.client.clone());
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(vec![Entry::UserMessage("Run the tests".into())], cx);
            thread.set_working_for_test(true, cx);
        });
        let focus = view.read_with(cx, |view, cx| view.composer.focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        cx.simulate_input("Then the docs");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let sent = sent(&client, cx);
        assert!(sent.iter().any(|request| matches!(request,
            Request::QueueMessage { prompt, .. } if *prompt == PromptPart::text("Then the docs"))));
        assert!(
            !sent
                .iter()
                .any(|request| matches!(request, Request::Prompt { .. }))
        );
    }

    /// The queue shows what the server keeps. Editing a message takes it out of the queue and
    /// brings it back to the composer as it was, its mentions as chips.
    #[gpui::test]
    fn editing_a_queued_message_brings_back_its_chips(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let client = view.read_with(cx, |view, _| view.client.clone());
        serve_images(&client, cx);
        let prompt = vec![
            PromptPart::Text("Compare ".into()),
            PromptPart::Image(image_id()),
            PromptPart::Text(" with ".into()),
            PromptPart::Path("/tmp/demo/README.md".into()),
            PromptPart::Text(" ".into()),
        ];
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(vec![Entry::UserMessage("Run the tests".into())], cx);
            thread.set_working_for_test(true, cx);
            thread.set_queued_messages_for_test(
                vec![QueuedMessage {
                    id: 7,
                    prompt: prompt.clone(),
                }],
                false,
                cx,
            );
        });
        view.update(cx, |view, cx| {
            view.queue_expanded = true;
            cx.notify();
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("queued-image-0-0").is_some());

        view.update_in(cx, |view, window, cx| {
            view.edit_queued_message(7, window, cx)
        });
        cx.run_until_parked();
        assert!(
            sent(&client, cx)
                .iter()
                .any(|request| matches!(request, Request::RemoveQueuedMessage { id: 7, .. }))
        );
        let (text, restored) = view.read_with(cx, |view, cx| {
            (
                view.composer.read(cx).plain_text(),
                view.composer_prompt(cx),
            )
        });
        assert_eq!(text, "Compare @Image with @README.md ");
        assert_eq!(restored, prompt);
    }

    /// What's typed is kept when the user leaves the thread, and comes back with its images
    /// and mentions as chips.
    #[gpui::test]
    fn a_draft_keeps_its_chips(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let client = view.read_with(cx, |view, _| view.client.clone());
        serve_images(&client, cx);
        let png = {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD
                .decode(TINY_PNG)
                .expect("a PNG")
        };
        view.update(cx, |view, cx| {
            view.composer
                .update(cx, |composer, cx| composer.insert("Compare ", cx));
            view.insert_image(gpui::ImageFormat::Png, png, cx);
            view.composer
                .update(cx, |composer, cx| composer.insert("with ", cx));
            view.mention_path("/tmp/demo/README.md".into(), cx);
        });
        cx.run_until_parked();
        cx.executor().advance_clock(UNSENT_TEXT_SAVE_DELAY * 2);
        cx.run_until_parked();
        let saves = |cx: &mut VisualTestContext| -> Vec<(Option<String>, Vec<UnsentMention>)> {
            sent(&client, cx)
                .into_iter()
                .filter_map(|request| match request {
                    Request::SetUnsentText { text, mentions, .. } => Some((text, mentions)),
                    _ => None,
                })
                .collect()
        };
        let (text, mentions) = saves(cx).pop().expect("the draft is saved");
        assert_eq!(text.as_deref(), Some("Compare @Image with @README.md "));
        assert_eq!(
            mentions,
            [
                UnsentMention {
                    range: 8..14,
                    target: projects::Mentioned::Image(image_id().as_str().into()),
                },
                UnsentMention {
                    range: 20..30,
                    target: projects::Mentioned::Path("/tmp/demo/README.md".into()),
                },
            ]
        );

        // The server keeps it, and the thread opens with it again.
        let store = view.read_with(cx, |view, _| view.store.clone());
        store.update(cx, |store, cx| {
            let mut snapshot = snapshot(None);
            let typed = snapshot
                .threads
                .iter_mut()
                .find(|thread| thread.id == ThreadId(2))
                .expect("the thread");
            typed.unsent_text = text;
            typed.unsent_mentions = mentions;
            store.set_snapshot(snapshot, cx)
        });
        let save_count = saves(cx).len();
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let reopened = cx.update(|_, cx| {
            cx.new(|cx| {
                AgentView::new(
                    ThreadId(2),
                    thread,
                    "New thread".into(),
                    Some(AgentId::new("mock")),
                    cx,
                )
            })
        });
        cx.run_until_parked();
        let (text, prompt) = reopened.read_with(cx, |view, cx| {
            (
                view.composer.read(cx).plain_text(),
                view.composer_prompt(cx),
            )
        });
        assert_eq!(text, "Compare @Image with @README.md ");
        assert_eq!(
            prompt,
            [
                PromptPart::Text("Compare ".into()),
                PromptPart::Image(image_id()),
                PromptPart::Text(" with ".into()),
                PromptPart::Path("/tmp/demo/README.md".into()),
                PromptPart::Text(" ".into()),
            ]
        );
        cx.executor().advance_clock(UNSENT_TEXT_SAVE_DELAY * 2);
        cx.run_until_parked();
        assert_eq!(saves(cx).len(), save_count, "nothing new to save");
    }

    /// A pasted image goes to the server first; a message sent meanwhile goes once it's there,
    /// naming the server's copy.
    #[gpui::test]
    fn a_message_waits_for_its_images(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let client = view.read_with(cx, |view, _| view.client.clone());
        serve_images(&client, cx);
        let png = {
            use base64::Engine as _;
            base64::engine::general_purpose::STANDARD
                .decode(TINY_PNG)
                .expect("a PNG")
        };
        view.update(cx, |view, cx| {
            view.insert_image(gpui::ImageFormat::Png, png, cx);
            view.send_message(cx);
        });
        assert!(
            !sent(&client, cx)
                .iter()
                .any(|request| matches!(request, Request::Prompt { .. }))
        );
        cx.run_until_parked();
        assert!(sent(&client, cx).iter().any(|request| matches!(request,
            Request::Prompt { prompt, .. }
                if *prompt == [PromptPart::Image(image_id()), PromptPart::Text(" ".into())])));
    }

    /// A tool's image shows in its output, and a click shows it whole until Escape.
    #[gpui::test]
    fn an_image_opens_whole_in_the_viewer(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let client = view.read_with(cx, |view, _| view.client.clone());
        serve_images(&client, cx);
        let Entry::ToolCall(mut screenshot) = tool_call(acp::ToolCallStatus::Completed) else {
            panic!("a tool call");
        };
        screenshot.text.clear();
        screenshot.images = vec![image_id()];
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(
                vec![
                    Entry::UserMessage("Take a screenshot".into()),
                    Entry::ToolCall(screenshot),
                ],
                cx,
            )
        });
        cx.run_until_parked();
        let row = cx
            .debug_bounds("tool-call-row-1")
            .expect("the tool call's row");
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        let image = cx.debug_bounds("tool-image-1-0").expect("the tool's image");
        cx.simulate_click(image.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("image-viewer").is_some());
        assert!(cx.debug_bounds("image-viewer-image").is_some());

        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(cx.debug_bounds("image-viewer").is_none());
    }

    /// An image taller than the room for it shrinks to fit, keeping its shape, in a tool's
    /// output and in the viewer.
    #[gpui::test]
    fn a_tall_image_fits_whole(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let client = view.read_with(cx, |view, _| view.client.clone());
        serve_image(&client, TALL_PNG, cx);
        let Entry::ToolCall(mut screenshot) = tool_call(acp::ToolCallStatus::Completed) else {
            panic!("a tool call");
        };
        screenshot.text.clear();
        screenshot.images = vec![image_id()];
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(
                vec![
                    Entry::UserMessage("Take a screenshot".into()),
                    Entry::ToolCall(screenshot),
                ],
                cx,
            )
        });
        cx.run_until_parked();
        let row = cx
            .debug_bounds("tool-call-row-1")
            .expect("the tool call's row");
        cx.simulate_click(row.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        let image = cx.debug_bounds("tool-image-1-0").expect("the tool's image");
        assert_eq!(image.size.height, TOOL_IMAGE_SIZE.height);

        cx.simulate_click(image.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        let viewport = cx.update(|window, _| window.viewport_size());
        let whole = cx
            .debug_bounds("image-viewer-image")
            .expect("the viewer's image");
        assert!(whole.top() >= px(0.) && whole.bottom() <= viewport.height);
        assert!(whole.size.height > viewport.height / 2.);
        let shape = whole.size.width / whole.size.height;
        assert!((shape - 0.05).abs() < 0.001, "{shape}");
    }

    /// An image link in a message shows the image's thumbnail while the mouse is on it.
    #[gpui::test]
    fn hovering_an_image_link_shows_its_thumbnail(cx: &mut TestAppContext) {
        let (view, cx) = open(2, false, cx);
        let thread = view.read_with(cx, |view, _| view.thread.clone());
        let client = view.read_with(cx, |view, _| view.client.clone());
        serve_images(&client, cx);
        thread.update(cx, |thread, cx| {
            thread.set_entries_for_test(
                vec![
                    Entry::UserMessage("Describe it".into()),
                    Entry::AgentMessage(format!("{} is a red dot.", image_id().markdown_link())),
                ],
                cx,
            )
        });
        cx.run_until_parked();
        let row = cx
            .debug_bounds("conversation-row-2")
            .expect("the agent's message");
        // The message is centered at most as wide as the content, its first line at its top.
        let left = row.left() + (row.size.width - row.size.width.min(MAX_CONTENT_WIDTH)) / 2.;
        let on_link = gpui::point(left + px(30.), row.top() + px(14.));
        cx.simulate_mouse_move(on_link, None, gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("image-hover-preview").is_some());

        let past_text = gpui::point(
            left + MAX_CONTENT_WIDTH.min(row.size.width) - px(30.),
            on_link.y,
        );
        cx.simulate_mouse_move(past_text, None, gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(cx.debug_bounds("image-hover-preview").is_none());
    }

    #[gpui::test]
    fn cmd_c_copies_text_selected_in_a_message(cx: &mut TestAppContext) {
        cx.update(crate::init_for_test);
        let (_, cx) = cx.add_window_view(|_, cx| {
            let markdown = cx.new(|cx| Markdown::new("Copy this reply".into(), None, None, cx));
            MarkdownView(markdown)
        });
        // The text is parsed in the background.
        cx.run_until_parked();
        let bounds = cx.debug_bounds("markdown").expect("the message");
        let start = gpui::point(bounds.left() + px(1.), bounds.top() + px(8.));
        let end = gpui::point(bounds.right() - px(1.), bounds.top() + px(8.));
        cx.simulate_mouse_down(start, gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_move(end, gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_mouse_up(end, gpui::MouseButton::Left, gpui::Modifiers::none());
        cx.simulate_keystrokes("cmd-c");
        let copied = cx.read_from_clipboard().and_then(|item| item.text());
        assert_eq!(copied.as_deref(), Some("Copy this reply"));
    }

    struct MarkdownView(Entity<Markdown>);

    impl Render for MarkdownView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .w(px(600.))
                .debug_selector(|| "markdown".into())
                .child(MarkdownElement::new(
                    self.0.clone(),
                    MarkdownStyle::default(),
                ))
        }
    }

    #[test]
    fn token_counts() {
        assert_eq!(humanize_token_count(950), "950");
        assert_eq!(humanize_token_count(1_234), "1.2k");
        assert_eq!(humanize_token_count(45_400), "45k");
        assert_eq!(humanize_token_count(200_000), "200k");
        assert_eq!(humanize_token_count(1_500_000), "1.5M");
    }
}
