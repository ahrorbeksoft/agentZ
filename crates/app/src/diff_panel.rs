//! A thread's changes next to its conversation, as t3code's diff panel shows them: the latest
//! turn or all of them, file by file, and files ticked off as viewed collapse. The server
//! computes the diff from its checkpoints; the panel reloads it whenever a turn ends.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::rc::Rc;
use std::time::SystemTime;

use agentz_protocol::diff::{
    DiffFile, DiffScope, DiffStatus, FileChange, RestoreAvailability, ThreadDiff,
};
use agentz_protocol::thread::DiffLineKind;
use agentz_protocol::{CAPABILITY_THREAD_DIFF, Request, Response};
use collections::{HashMap, HashSet};
use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, ListAlignment, ListState, PromptLevel,
    Subscription, Task, WeakEntity, Window, list,
};
use projects::ThreadId;
use ui::{
    Checkbox, Disclosure, ToggleButtonGroup, ToggleButtonGroupSize, ToggleButtonSimple,
    ToggleState, Tooltip, prelude::*,
};

use crate::ToggleDiff;
use crate::agent_view::TOOLBAR_HEIGHT;
use crate::server_client::ServerClient;

pub const DIFF_PANEL_WIDTH: Pixels = px(520.);

/// What the panel asks of the shell showing it.
pub enum DiffPanelEvent {
    Close,
    ToggleFullScreen,
}
const TAB: &str = "    ";

/// One line of the panel's list.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Row {
    File(usize),
    Hunk {
        file: usize,
        hunk: usize,
    },
    Line {
        file: usize,
        hunk: usize,
        line: usize,
        old: Option<u32>,
        new: Option<u32>,
    },
    Binary(usize),
    Truncated,
}

pub struct DiffPanel {
    client: Entity<ServerClient>,
    thread_id: ThreadId,
    scope: DiffScope,
    diff: Option<Rc<ThreadDiff>>,
    error: Option<SharedString>,
    loading: bool,
    /// When the thread last finished a turn, to reload after the next one.
    completed_at: Option<SystemTime>,
    collapsed: HashSet<String>,
    /// Files marked as viewed, with their contents' hash then: a file that changes since comes
    /// back unviewed (t3code).
    viewed: HashMap<String, u64>,
    rows: Rc<Vec<Row>>,
    list_state: ListState,
    is_full_screen: bool,
    _load: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DiffPanelEvent> for DiffPanel {}

impl DiffPanel {
    /// Whether the panel fills the thread's area, for its header's button.
    pub fn set_full_screen(&mut self, is_full_screen: bool, cx: &mut Context<Self>) {
        if self.is_full_screen != is_full_screen {
            self.is_full_screen = is_full_screen;
            cx.notify();
        }
    }

    pub fn new(client: Entity<ServerClient>, thread_id: ThreadId, cx: &mut Context<Self>) -> Self {
        let store = client.read(cx).projects().clone();
        let completed_at = store
            .read(cx)
            .thread(thread_id)
            .and_then(|thread| thread.completed_at);
        let subscriptions = vec![cx.observe(&store, |this, store, cx| {
            let completed_at = store
                .read(cx)
                .thread(this.thread_id)
                .and_then(|thread| thread.completed_at);
            if completed_at != this.completed_at {
                this.completed_at = completed_at;
                this.reload(cx);
            }
        })];
        let mut this = Self {
            client,
            thread_id,
            scope: DiffScope::LatestTurn,
            diff: None,
            error: None,
            loading: false,
            completed_at,
            collapsed: HashSet::default(),
            viewed: HashMap::default(),
            rows: Rc::new(Vec::new()),
            list_state: ListState::new(0, ListAlignment::Top, px(400.)),
            is_full_screen: false,
            _load: Task::ready(()),
            _subscriptions: subscriptions,
        };
        this.reload(cx);
        this
    }

    pub fn thread_id(&self) -> ThreadId {
        self.thread_id
    }

    pub fn client(&self) -> &Entity<ServerClient> {
        &self.client
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let client = self.client.clone();
        let client_state = client.read(cx);
        if client_state.connection().is_some()
            && !client_state.has_capability(CAPABILITY_THREAD_DIFF)
        {
            self.error = Some("This agentz-server can't show changes; update it.".into());
            cx.notify();
            return;
        }
        let request = client.read(cx).request(Request::ThreadDiff {
            thread_id: self.thread_id,
            scope: self.scope,
        });
        self.loading = true;
        // Replacing the task drops a load still on its way, which would be stale.
        self._load = cx.spawn(async move |this, cx| {
            let response = request.await;
            this.update(cx, |this, cx| {
                this.loading = false;
                match response {
                    Ok(Response::ThreadDiff(diff)) => {
                        this.error = None;
                        this.set_diff(diff);
                    }
                    Ok(response) => {
                        log::error!("expected a diff, got {response:?}");
                        this.error = Some("The server sent something unexpected.".into());
                    }
                    Err(error) => this.error = Some(format!("{error:#}").into()),
                }
                cx.notify();
            })
            .ok();
        });
        cx.notify();
    }

    fn set_scope(&mut self, scope: DiffScope, cx: &mut Context<Self>) {
        if self.scope == scope {
            return;
        }
        self.scope = scope;
        self.diff = None;
        self.update_rows(true);
        self.reload(cx);
    }

    fn set_diff(&mut self, diff: ThreadDiff) {
        let unchanged: HashMap<String, u64> = diff
            .files
            .iter()
            .filter_map(|file| {
                let hash = content_hash(file);
                (self.viewed.get(&file.path) == Some(&hash)).then(|| (file.path.clone(), hash))
            })
            .collect();
        // A viewed file that changed opens again.
        for (path, _) in &self.viewed {
            if !unchanged.contains_key(path) {
                self.collapsed.remove(path);
            }
        }
        self.viewed = unchanged;
        let is_new_scope = self.diff.is_none();
        self.diff = Some(Rc::new(diff));
        self.update_rows(is_new_scope);
    }

    fn update_rows(&mut self, scroll_to_top: bool) {
        let rows = match &self.diff {
            Some(diff) => build_rows(diff, &self.collapsed),
            None => Vec::new(),
        };
        if scroll_to_top {
            self.list_state.reset(rows.len());
        } else {
            // Only what changed is measured again, so the scroll position stays put.
            let old = &self.rows;
            let prefix = old
                .iter()
                .zip(&rows)
                .take_while(|(old, new)| old == new)
                .count();
            let suffix = old[prefix..]
                .iter()
                .rev()
                .zip(rows[prefix..].iter().rev())
                .take_while(|(old, new)| old == new)
                .count();
            self.list_state
                .splice(prefix..old.len() - suffix, rows.len() - prefix - suffix);
        }
        self.rows = Rc::new(rows);
    }

    fn toggle_collapsed(&mut self, path: &str, cx: &mut Context<Self>) {
        if !self.collapsed.remove(path) {
            self.collapsed.insert(path.to_string());
        }
        self.update_rows(false);
        cx.notify();
    }

    fn set_viewed(&mut self, file: usize, viewed: bool, cx: &mut Context<Self>) {
        let Some(file) = self.diff.as_ref().and_then(|diff| diff.files.get(file)) else {
            return;
        };
        let path = file.path.clone();
        if viewed {
            self.viewed.insert(path.clone(), content_hash(file));
            self.collapsed.insert(path);
        } else {
            self.viewed.remove(&path);
            self.collapsed.remove(&path);
        }
        self.update_rows(false);
        cx.notify();
    }

    fn toggle_all_collapsed(&mut self, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            return;
        };
        let all_collapsed = diff
            .files
            .iter()
            .all(|file| self.collapsed.contains(&file.path));
        if all_collapsed {
            self.collapsed.clear();
        } else {
            self.collapsed
                .extend(diff.files.iter().map(|file| file.path.clone()));
        }
        self.update_rows(false);
        cx.notify();
    }

    /// Puts the files back as they were before the shown changes, after asking.
    fn confirm_restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let scope = self.scope;
        let (message, detail) = match scope {
            DiffScope::LatestTurn => (
                "Revert the latest turn?",
                "Its changes to files are undone, and it no longer counts. The conversation \
                 stays as it is.",
            ),
            DiffScope::All => (
                "Revert all changes?",
                "Files go back to how they were before the thread's first turn. The \
                 conversation stays as it is.",
            ),
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            message,
            Some(detail),
            &["Revert", "Cancel"],
            cx,
        );
        let thread_id = self.thread_id;
        let client = self.client.clone();
        self._load = cx.spawn_in(window, async move |this, cx| {
            if answer.await != Ok(0) {
                return;
            }
            let request = client.read_with(cx, |client, _| {
                client.request(Request::RestoreCheckpoint { thread_id, scope })
            });
            this.update(cx, |this, cx| {
                this.loading = true;
                cx.notify();
            })
            .ok();
            let response = request.await;
            let failure = this
                .update(cx, |this, cx| {
                    this.loading = false;
                    cx.notify();
                    match response {
                        Ok(Response::ThreadDiff(diff)) => {
                            this.set_diff(diff);
                            None
                        }
                        Ok(response) => Some(format!("Unexpected response: {response:?}")),
                        Err(error) => Some(format!("{error:#}")),
                    }
                })
                .ok()
                .flatten();
            if let Some(failure) = failure {
                let answer = cx.update(|window, cx| {
                    window.prompt(
                        PromptLevel::Critical,
                        "Couldn't revert",
                        Some(&failure),
                        &["OK"],
                        cx,
                    )
                });
                if let Ok(answer) = answer {
                    answer.await.ok();
                }
            }
        });
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let latest = cx.entity().downgrade();
        let all = latest.clone();
        let files = self.diff.as_ref().map(|diff| diff.files.as_slice());
        let all_collapsed = files.is_some_and(|files| {
            !files.is_empty() && files.iter().all(|file| self.collapsed.contains(&file.path))
        });
        h_flex()
            .h(TOOLBAR_HEIGHT)
            .flex_none()
            .px_2()
            .gap_1()
            .border_b_1()
            .border_color(cx.theme().colors().border)
            .child(
                ToggleButtonGroup::single_row(
                    "diff-scope",
                    [
                        ToggleButtonSimple::new("Latest Turn", move |_, _, cx| {
                            latest
                                .update(cx, |this, cx| this.set_scope(DiffScope::LatestTurn, cx))
                                .ok();
                        }),
                        ToggleButtonSimple::new("All Changes", move |_, _, cx| {
                            all.update(cx, |this, cx| this.set_scope(DiffScope::All, cx))
                                .ok();
                        }),
                    ],
                )
                .size(ToggleButtonGroupSize::Default)
                .auto_width()
                .selected_index(match self.scope {
                    DiffScope::LatestTurn => 0,
                    DiffScope::All => 1,
                }),
            )
            .child(div().flex_1())
            .when_some(files.filter(|files| !files.is_empty()), |this, files| {
                let viewed = files
                    .iter()
                    .filter(|file| self.viewed.contains_key(&file.path))
                    .count();
                this.child(
                    Label::new(format!("{viewed}/{} viewed", files.len()))
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    IconButton::new(
                        "diff-collapse-all",
                        if all_collapsed {
                            IconName::ExpandVertical
                        } else {
                            IconName::FoldVertical
                        },
                    )
                    .icon_size(IconSize::Small)
                    .tooltip(Tooltip::text(if all_collapsed {
                        "Expand All Files"
                    } else {
                        "Collapse All Files"
                    }))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_all_collapsed(cx))),
                )
            })
            .when_some(
                self.diff
                    .as_ref()
                    .filter(|diff| !diff.files.is_empty())
                    .map(|diff| diff.restore.clone()),
                |this, restore| {
                    let label = match self.scope {
                        DiffScope::LatestTurn => "Revert Latest Turn",
                        DiffScope::All => "Revert All Changes",
                    };
                    let (available, tooltip) = match restore {
                        RestoreAvailability::Available => (true, SharedString::from(label)),
                        RestoreAvailability::Unavailable(reason) => (false, reason.into()),
                        RestoreAvailability::Unknown(_) => {
                            (false, "This server can't restore files.".into())
                        }
                    };
                    this.child(
                        IconButton::new("diff-restore", IconName::Undo)
                            .icon_size(IconSize::Small)
                            .disabled(!available || self.loading)
                            .tooltip(Tooltip::text(tooltip))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.confirm_restore(window, cx)),
                            ),
                    )
                },
            )
            .child(
                IconButton::new("diff-refresh", IconName::ArrowCircle)
                    .icon_size(IconSize::Small)
                    .disabled(self.loading)
                    .tooltip(Tooltip::text("Refresh"))
                    .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
            )
            .map(|this| {
                let is_full_screen = self.is_full_screen;
                this.child(
                    IconButton::new(
                        "diff-full-screen",
                        if is_full_screen {
                            IconName::Minimize
                        } else {
                            IconName::Maximize
                        },
                    )
                    .icon_size(IconSize::Small)
                    .tooltip(Tooltip::text(if is_full_screen {
                        "Exit Full Screen"
                    } else {
                        "Full Screen"
                    }))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DiffPanelEvent::ToggleFullScreen))),
                )
            })
            .child(
                IconButton::new("diff-close", IconName::Close)
                    .icon_size(IconSize::Small)
                    .tooltip(|_, cx| Tooltip::for_action("Hide Changes", &ToggleDiff, cx))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(DiffPanelEvent::Close))),
            )
    }

    fn render_summary(&self, diff: &ThreadDiff) -> Option<AnyElement> {
        if diff.files.is_empty() {
            return None;
        }
        let additions: u32 = diff.files.iter().map(|file| file.additions).sum();
        let deletions: u32 = diff.files.iter().map(|file| file.deletions).sum();
        let count = diff.files.len();
        let turns = match self.scope {
            DiffScope::LatestTurn => format!("Turn {}", diff.turns),
            DiffScope::All if diff.turns == 1 => "1 turn".to_string(),
            DiffScope::All => format!("{} turns", diff.turns),
        };
        Some(
            h_flex()
                .px_2()
                .py_1()
                .gap_2()
                .flex_none()
                .child(
                    Label::new(if count == 1 {
                        "1 file".to_string()
                    } else {
                        format!("{count} files")
                    })
                    .size(LabelSize::Small),
                )
                .child(line_stat(additions, deletions))
                .child(div().flex_1())
                .child(Label::new(turns).size(LabelSize::Small).color(Color::Muted))
                .into_any_element(),
        )
    }
}

impl Render for DiffPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let notice = match (&self.diff, &self.error) {
            (_, Some(error)) => Some(error.clone()),
            (None, None) => Some(if self.loading { "Loading…" } else { "" }.into()),
            (Some(diff), None) => match &diff.status {
                DiffStatus::NotRepository => Some(
                    "This project isn't a git repository, so agentZ can't track its changes."
                        .into(),
                ),
                DiffStatus::NoTurns => Some("No finished turns yet.".into()),
                DiffStatus::Unknown(_) => Some("This server sent a diff agentZ can't read.".into()),
                DiffStatus::Ready if diff.files.is_empty() => Some(match self.scope {
                    DiffScope::LatestTurn => "The latest turn changed no files.".into(),
                    DiffScope::All => "No changes yet.".into(),
                }),
                DiffStatus::Ready => None,
            },
        };
        let summary = self
            .diff
            .as_ref()
            .filter(|_| notice.is_none())
            .and_then(|diff| self.render_summary(diff));

        let body = match (notice, self.diff.clone()) {
            (None, Some(diff)) => {
                let rows = self.rows.clone();
                let viewed = Rc::new(self.viewed.clone());
                let collapsed = Rc::new(self.collapsed.clone());
                let panel = cx.entity().downgrade();
                list(self.list_state.clone(), move |index, _, cx| {
                    let Some(row) = rows.get(index) else {
                        return div().into_any_element();
                    };
                    render_row(*row, &diff, &viewed, &collapsed, &panel, cx)
                })
                .flex_1()
                .min_h_0()
                .into_any_element()
            }
            (notice, _) => v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .p_4()
                .child(Label::new(notice.unwrap_or_default()).color(Color::Muted))
                .into_any_element(),
        };

        v_flex()
            .size_full()
            .bg(colors.editor_background)
            .child(self.render_header(cx))
            .children(summary)
            .child(body)
    }
}

fn render_row(
    row: Row,
    diff: &ThreadDiff,
    viewed: &HashMap<String, u64>,
    collapsed: &HashSet<String>,
    panel: &WeakEntity<DiffPanel>,
    cx: &App,
) -> AnyElement {
    let colors = cx.theme().colors();
    match row {
        Row::File(index) => {
            let file = &diff.files[index];
            let is_collapsed = collapsed.contains(&file.path);
            let is_viewed = viewed.contains_key(&file.path);
            let (directory, name) = match file.path.rsplit_once('/') {
                Some((directory, name)) => (format!("{directory}/"), name.to_string()),
                None => (String::new(), file.path.clone()),
            };
            let toggle_panel = panel.clone();
            let toggle_path = file.path.clone();
            let viewed_panel = panel.clone();
            let (change, change_color) = match file.change {
                FileChange::Added => ("Added", Color::Created),
                FileChange::Deleted => ("Deleted", Color::Deleted),
                FileChange::Renamed => ("Renamed", Color::Modified),
                FileChange::Modified => ("", Color::Muted),
            };
            h_flex()
                .id(("diff-file", index))
                .w_full()
                .px_2()
                .py_1()
                .gap_1p5()
                .mt(if index == 0 { px(0.) } else { px(6.) })
                .border_y_1()
                .border_color(colors.border)
                .bg(colors.panel_background)
                .cursor_pointer()
                .on_click(move |_, _, cx| {
                    toggle_panel
                        .update(cx, |panel, cx| panel.toggle_collapsed(&toggle_path, cx))
                        .ok();
                })
                .child(Disclosure::new(
                    ("diff-file-disclosure", index),
                    !is_collapsed,
                ))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .when(!directory.is_empty(), |this| {
                            this.child(
                                Label::new(directory)
                                    .size(LabelSize::Small)
                                    .color(Color::Muted)
                                    .truncate(),
                            )
                        })
                        .child(Label::new(name).size(LabelSize::Small).truncate()),
                )
                .when_some(file.old_path.clone(), |this, old_path| {
                    this.tooltip(Tooltip::text(format!("Renamed from {old_path}")))
                })
                .when(!change.is_empty(), |this| {
                    this.child(
                        Label::new(change)
                            .size(LabelSize::XSmall)
                            .color(change_color),
                    )
                })
                .child(line_stat(file.additions, file.deletions))
                .child(
                    // Ticking the box must not also toggle the file.
                    div()
                        .id(("diff-file-viewed-wrapper", index))
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            Checkbox::new(
                                ("diff-file-viewed", index),
                                if is_viewed {
                                    ToggleState::Selected
                                } else {
                                    ToggleState::Unselected
                                },
                            )
                            .label("Viewed")
                            .label_size(LabelSize::XSmall)
                            .on_click(move |state, _, cx| {
                                let viewed = *state == ToggleState::Selected;
                                viewed_panel
                                    .update(cx, |panel, cx| panel.set_viewed(index, viewed, cx))
                                    .ok();
                            }),
                        ),
                )
                .into_any_element()
        }
        Row::Hunk { file, hunk } => {
            let hunk = &diff.files[file].hunks[hunk];
            div()
                .w_full()
                .px_2()
                .py_0p5()
                .bg(colors.element_background)
                .font_buffer(cx)
                .text_size(rems_from_px(11_f32))
                .text_color(colors.text_muted)
                .child(hunk.header.clone())
                .into_any_element()
        }
        Row::Line {
            file,
            hunk,
            line,
            old,
            new,
        } => {
            let line = &diff.files[file].hunks[hunk].lines[line];
            let (marker, background) = match line.kind {
                DiffLineKind::Context => (" ", None),
                DiffLineKind::Removed => ("-", Some(colors.editor_diff_hunk_deleted_background)),
                DiffLineKind::Added => ("+", Some(colors.editor_diff_hunk_added_background)),
            };
            let number = |number: Option<u32>| {
                div()
                    .w(px(34.))
                    .flex_none()
                    .text_right()
                    .text_color(colors.text_muted.opacity(0.7))
                    .child(number.map(|number| number.to_string()).unwrap_or_default())
            };
            h_flex()
                .w_full()
                .items_start()
                .font_buffer(cx)
                .text_size(rems_from_px(12_f32))
                .line_height(rems_from_px(18_f32))
                .when_some(background, |this, background| this.bg(background))
                .child(number(old))
                .child(number(new).mr_1())
                .child(
                    div()
                        .w(px(12.))
                        .flex_none()
                        .text_color(colors.text_muted)
                        .child(marker),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .pr_2()
                        .when(line.kind == DiffLineKind::Context, |this| {
                            this.text_color(colors.text_muted)
                        })
                        .child(line.text.replace('\t', TAB)),
                )
                .into_any_element()
        }
        Row::Binary(_) => div()
            .px_4()
            .py_1()
            .child(
                Label::new("Binary file not shown")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
            .into_any_element(),
        Row::Truncated => div()
            .px_2()
            .py_2()
            .child(
                Label::new("This diff is too large to show in full.")
                    .size(LabelSize::Small)
                    .color(Color::Warning),
            )
            .into_any_element(),
    }
}

fn build_rows(diff: &ThreadDiff, collapsed: &HashSet<String>) -> Vec<Row> {
    let mut rows = Vec::new();
    for (file_index, file) in diff.files.iter().enumerate() {
        rows.push(Row::File(file_index));
        if collapsed.contains(&file.path) {
            continue;
        }
        if file.binary {
            rows.push(Row::Binary(file_index));
            continue;
        }
        for (hunk_index, hunk) in file.hunks.iter().enumerate() {
            rows.push(Row::Hunk {
                file: file_index,
                hunk: hunk_index,
            });
            let (mut old, mut new) = (hunk.old_start, hunk.new_start);
            for (line_index, line) in hunk.lines.iter().enumerate() {
                let (old_number, new_number) = match line.kind {
                    DiffLineKind::Context => {
                        old += 1;
                        new += 1;
                        (Some(old - 1), Some(new - 1))
                    }
                    DiffLineKind::Removed => {
                        old += 1;
                        (Some(old - 1), None)
                    }
                    DiffLineKind::Added => {
                        new += 1;
                        (None, Some(new - 1))
                    }
                };
                rows.push(Row::Line {
                    file: file_index,
                    hunk: hunk_index,
                    line: line_index,
                    old: old_number,
                    new: new_number,
                });
            }
        }
    }
    if diff.truncated {
        rows.push(Row::Truncated);
    }
    rows
}

fn content_hash(file: &DiffFile) -> u64 {
    let mut hasher = DefaultHasher::new();
    for hunk in &file.hunks {
        hunk.header.hash(&mut hasher);
        for line in &hunk.lines {
            (line.kind as u8).hash(&mut hasher);
            line.text.hash(&mut hasher);
        }
    }
    file.binary.hash(&mut hasher);
    (file.change as u8).hash(&mut hasher);
    hasher.finish()
}

fn line_stat(additions: u32, deletions: u32) -> impl IntoElement {
    h_flex()
        .gap_1()
        .child(
            Label::new(format!("+{additions}"))
                .size(LabelSize::Small)
                .color(Color::Created),
        )
        .child(
            Label::new(format!("−{deletions}"))
                .size(LabelSize::Small)
                .color(Color::Deleted),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_number_lines_and_skip_collapsed_files() {
        let patch = "\
diff --git a/a.txt b/a.txt
--- a/a.txt
+++ b/a.txt
@@ -10,3 +10,3 @@
 keep
-old
+new
 keep
diff --git a/b.txt b/b.txt
--- a/b.txt
+++ b/b.txt
@@ -1 +1 @@
-x
+y
";
        let diff = ThreadDiff {
            status: DiffStatus::Ready,
            turns: 1,
            files: agentz_protocol::diff::parse_patch(patch),
            truncated: false,
            restore: RestoreAvailability::Available,
        };
        let collapsed = HashSet::from_iter(["b.txt".to_string()]);
        let numbers: Vec<(Option<u32>, Option<u32>)> = build_rows(&diff, &collapsed)
            .into_iter()
            .filter_map(|row| match row {
                Row::Line { old, new, .. } => Some((old, new)),
                _ => None,
            })
            .collect();
        assert_eq!(
            numbers,
            vec![
                (Some(10), Some(10)),
                (Some(11), None),
                (None, Some(11)),
                (Some(12), Some(12)),
            ]
        );
        assert_eq!(build_rows(&diff, &collapsed).last(), Some(&Row::File(1)));
    }
}
