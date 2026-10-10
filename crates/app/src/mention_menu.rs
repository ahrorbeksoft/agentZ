//! The composer's @-mentions (Zed's, with t3code's menu as picked in `design/composer/`): what @
//! finds as it's typed, files and threads in groups, and what each mention sends.

use std::path::PathBuf;

use agentz_protocol::attachments::AttachmentId;
use agentz_protocol::{FileListing, PromptPart};
use gpui::{AnyElement, App, SharedString};
use projects::{Mentioned, Project, ThreadId};
use ui::{ListItem, ListItemSpacing, prelude::*};

use crate::machines::MachineId;
use crate::project_info::render_project_icon;
use crate::project_switcher::fuzzy_match;

/// The most of each group the menu lists.
const MAX_FILES: usize = 8;
const MAX_THREADS: usize = 5;
const MAX_PROJECTS: usize = 5;

/// What a chip in the composer stands for.
#[derive(Clone)]
pub(crate) enum Mention {
    /// A file or folder on the thread's machine.
    Path(PathBuf),
    Thread(ThreadId),
    /// An image kept by the thread's server.
    Image(AttachmentId),
    /// A thread on another machine, as the app read its conversation there.
    Conversation {
        uri: String,
        title: String,
        text: String,
    },
    /// A project on another machine, in words: the agent reaches it with agentZ's tools.
    Text(String),
}

impl Mention {
    pub(crate) fn prompt_part(&self) -> PromptPart {
        match self {
            Mention::Path(path) => PromptPart::Path(path.clone()),
            Mention::Thread(thread_id) => PromptPart::Thread(*thread_id),
            Mention::Image(id) => PromptPart::Image(id.clone()),
            Mention::Conversation { uri, title, text } => PromptPart::Conversation {
                uri: uri.clone(),
                title: title.clone(),
                text: text.clone(),
            },
            Mention::Text(text) => PromptPart::Text(text.clone()),
        }
    }

    /// How a composer draft keeps it ([`projects::Thread::unsent_mentions`]). Another
    /// machine's thread or project stays in the draft as its name.
    pub(crate) fn target(&self) -> Option<Mentioned> {
        Some(match self {
            Mention::Path(path) => Mentioned::Path(path.clone()),
            Mention::Thread(thread_id) => Mentioned::Thread(*thread_id),
            Mention::Image(id) => Mentioned::Image(id.as_str().to_string()),
            Mention::Conversation { .. } | Mention::Text(_) => return None,
        })
    }

    pub(crate) fn from_target(target: &Mentioned) -> Option<Self> {
        Some(match target {
            Mentioned::Path(path) => Mention::Path(path.clone()),
            Mentioned::Thread(thread_id) => Mention::Thread(*thread_id),
            Mentioned::Image(id) => Mention::Image(AttachmentId::parse(id)?),
        })
    }
}

/// What the + button's menu narrowed @ to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum MentionKind {
    #[default]
    Any,
    Files,
    Threads,
}

/// The `@query` the cursor is at the end of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MentionQuery {
    /// Where its `@` is.
    pub at: usize,
    pub query: String,
}

/// An `@` at the start or after whitespace, then no whitespace up to the cursor.
pub(crate) fn mention_query(text: &str, cursor: usize) -> Option<MentionQuery> {
    let before = text.get(..cursor)?;
    let at = before.rfind('@')?;
    let query = &before[at + 1..];
    if query.contains(char::is_whitespace) {
        return None;
    }
    let starts_word = before[..at]
        .chars()
        .next_back()
        .is_none_or(char::is_whitespace);
    starts_word.then(|| MentionQuery {
        at,
        query: query.to_string(),
    })
}

/// A thread @ can mention.
pub(crate) struct MentionableThread {
    pub machine: MachineId,
    pub id: ThreadId,
    pub title: SharedString,
    /// When it last did something, or in a chat its project, as the menu shows it.
    pub detail: SharedString,
}

/// A project a chat's @ can mention, on any machine.
pub(crate) struct MentionableProject {
    pub machine: MachineId,
    pub project: Project,
    pub name: SharedString,
    /// The machine it's on, when that's not the chat's.
    pub detail: SharedString,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MentionTarget {
    /// Relative to the file listing's root.
    Path { path: String, is_dir: bool },
    Thread {
        machine: MachineId,
        id: ThreadId,
        title: SharedString,
    },
    Project {
        machine: MachineId,
        project: Project,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MentionMatch {
    pub target: MentionTarget,
    pub label: SharedString,
    pub detail: SharedString,
}

/// What a query finds: files whose name matches before those whose path does, the most
/// compact matches first; then threads, the latest first. A chat's (with `projects`) lists
/// projects first, then threads, then the files in its own folder.
pub(crate) fn find_mentions(
    query: &str,
    kind: MentionKind,
    files: Option<&FileListing>,
    threads: &[MentionableThread],
    projects: Option<&[MentionableProject]>,
) -> Vec<MentionMatch> {
    let query = query.to_lowercase();
    let mut found = Vec::new();
    if kind == MentionKind::Any
        && let Some(projects) = projects
    {
        found.extend(
            projects
                .iter()
                .filter(|project| fuzzy_match(&query, &project.name).is_some())
                .take(MAX_PROJECTS)
                .map(|project| MentionMatch {
                    target: MentionTarget::Project {
                        machine: project.machine,
                        project: project.project.clone(),
                    },
                    label: project.name.clone(),
                    detail: project.detail.clone(),
                }),
        );
    }
    let mut files_found = Vec::new();
    if kind != MentionKind::Threads
        && let Some(files) = files
    {
        let mut scored: Vec<((u8, usize, usize), &agentz_protocol::FileEntry)> = files
            .entries
            .iter()
            .filter_map(|entry| {
                let name_start = entry.path.rfind('/').map_or(0, |slash| slash + 1);
                let name = &entry.path[name_start..];
                let span = |positions: &[usize]| match (positions.first(), positions.last()) {
                    (Some(first), Some(last)) => last - first,
                    _ => 0,
                };
                let depth = entry.path.matches('/').count();
                if let Some(positions) = fuzzy_match(&query, name) {
                    Some(((0, span(&positions), depth), entry))
                } else {
                    fuzzy_match(&query, &entry.path)
                        .map(|positions| ((1, span(&positions), depth), entry))
                }
            })
            .collect();
        scored.sort_by(|(a, a_entry), (b, b_entry)| {
            a.cmp(b)
                .then_with(|| a_entry.path.len().cmp(&b_entry.path.len()))
                .then_with(|| a_entry.path.cmp(&b_entry.path))
        });
        for (_, entry) in scored.into_iter().take(MAX_FILES) {
            let (folder, name) = match entry.path.rsplit_once('/') {
                Some((folder, name)) => (folder.to_string(), name.to_string()),
                None => (String::new(), entry.path.clone()),
            };
            files_found.push(MentionMatch {
                target: MentionTarget::Path {
                    path: entry.path.clone(),
                    is_dir: entry.is_dir,
                },
                label: name.into(),
                detail: folder.into(),
            });
        }
    }
    if projects.is_none() {
        found.append(&mut files_found);
    }
    if kind != MentionKind::Files {
        found.extend(
            threads
                .iter()
                .filter(|thread| fuzzy_match(&query, &thread.title).is_some())
                .take(MAX_THREADS)
                .map(|thread| MentionMatch {
                    target: MentionTarget::Thread {
                        machine: thread.machine,
                        id: thread.id,
                        title: thread.title.clone(),
                    },
                    label: thread.title.clone(),
                    detail: thread.detail.clone(),
                }),
        );
    }
    found.append(&mut files_found);
    found
}

pub(crate) fn target_icon(target: &MentionTarget) -> IconName {
    match target {
        MentionTarget::Path { is_dir: true, .. } | MentionTarget::Project { .. } => {
            IconName::Folder
        }
        MentionTarget::Path { .. } => IconName::File,
        MentionTarget::Thread { .. } => IconName::Thread,
    }
}

/// t3code's grouped menu, in the slash-command menu's place above the composer.
pub(crate) fn render_mention_menu(
    matches: &[MentionMatch],
    selected: usize,
    loading_files: bool,
    on_click: impl Fn(usize, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> AnyElement {
    let header = |label: &'static str| {
        div().px_2().pt_1p5().pb_0p5().child(
            Label::new(label)
                .size(LabelSize::XSmall)
                .color(Color::Muted),
        )
    };
    let mut items: Vec<AnyElement> = Vec::new();
    let mut last_group = None;
    for (index, found) in matches.iter().enumerate() {
        let group = match found.target {
            MentionTarget::Path { .. } => "Files",
            MentionTarget::Thread { .. } => "Threads",
            MentionTarget::Project { .. } => "Projects",
        };
        if last_group != Some(group) {
            items.push(header(group).into_any_element());
            last_group = Some(group);
        }
        let on_click = on_click.clone();
        let icon = match &found.target {
            MentionTarget::Project { machine, project } => {
                render_project_icon(*machine, project, px(14.), cx)
            }
            target => Icon::new(target_icon(target))
                .size(IconSize::Small)
                .color(Color::Muted)
                .into_any_element(),
        };
        items.push(
            ListItem::new(("mention", index))
                .inset(true)
                .spacing(ListItemSpacing::Dense)
                .toggle_state(index == selected)
                .start_slot(icon)
                .child(
                    h_flex()
                        .w_full()
                        .min_w_0()
                        .gap_2()
                        .child(Label::new(found.label.clone()).truncate())
                        .child(
                            div().ml_auto().min_w_0().child(
                                Label::new(found.detail.clone())
                                    .size(LabelSize::Small)
                                    .color(Color::Placeholder)
                                    .truncate(),
                            ),
                        ),
                )
                .on_click(move |_, window, cx| on_click(index, window, cx))
                .into_any_element(),
        );
    }
    if matches.is_empty() {
        let message = if loading_files {
            "Looking for files…"
        } else {
            "Nothing matches"
        };
        items.push(
            div()
                .px_2()
                .py_1()
                .child(
                    Label::new(message)
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .into_any_element(),
        );
    }
    v_flex()
        .absolute()
        .bottom_full()
        .left_0()
        .mb_1()
        .w(rems(26.))
        .p_1()
        .elevation_2(cx)
        .children(items)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use agentz_protocol::FileEntry;

    use super::*;

    #[test]
    fn a_mention_starts_at_an_at_sign_that_starts_a_word() {
        assert_eq!(
            mention_query("look at @tot", 12),
            Some(MentionQuery {
                at: 8,
                query: "tot".into()
            })
        );
        assert_eq!(mention_query("@", 1).map(|found| found.at), Some(0));
        assert_eq!(mention_query("mail me@home", 12), None);
        assert_eq!(mention_query("@tot al", 7), None);
    }

    #[test]
    fn names_match_before_paths() {
        let files = FileListing {
            root: "/repo".into(),
            entries: [
                "src/total.ts",
                "src/cart/item.ts",
                "totals",
                "docs/totally/readme.md",
            ]
            .into_iter()
            .map(|path| FileEntry {
                path: path.into(),
                is_dir: path == "totals",
            })
            .collect(),
        };
        let threads = [MentionableThread {
            machine: MachineId::Local,
            id: ThreadId(4),
            title: "Order totals report".into(),
            detail: "2d".into(),
        }];
        let found = find_mentions("tot", MentionKind::Any, Some(&files), &threads, None);
        let labels: Vec<&str> = found.iter().map(|found| found.label.as_ref()).collect();
        assert_eq!(
            labels,
            ["totals", "total.ts", "readme.md", "Order totals report"]
        );
        let only_threads = find_mentions("tot", MentionKind::Threads, Some(&files), &threads, None);
        assert_eq!(only_threads.len(), 1);
    }

    #[test]
    fn a_chat_lists_projects_then_threads_then_its_files() {
        let files = FileListing {
            root: "/chats/a".into(),
            entries: vec![FileEntry {
                path: "store-notes.md".into(),
                is_dir: false,
            }],
        };
        let threads = [MentionableThread {
            machine: MachineId::Remote(1),
            id: ThreadId(4),
            title: "Stock levels".into(),
            detail: "api · Devbox 1".into(),
        }];
        let projects = [MentionableProject {
            machine: MachineId::Local,
            project: Project {
                id: projects::ProjectId(1),
                path: "/code/storefront".into(),
                custom_name: None,
                icon: None,
                workspaces: Vec::new(),
                repository: None,
            },
            name: "storefront".into(),
            detail: "".into(),
        }];
        let found = find_mentions(
            "st",
            MentionKind::Any,
            Some(&files),
            &threads,
            Some(&projects),
        );
        let labels: Vec<&str> = found.iter().map(|found| found.label.as_ref()).collect();
        assert_eq!(labels, ["storefront", "Stock levels", "store-notes.md"]);
    }
}
