//! Settings › Storage (design/storage): what agentZ keeps on a machine and the room it takes, as
//! the machine's server keeps it measured. The total and a bar of each kind lead; a group for
//! each kind follows, biggest first: chats, threads by project, worktrees and pastures, agents,
//! and Node.js and logs. A row that can go shows a trash button on hover, which asks first and
//! deletes for good; a working thread's data and a checkout with changes are listed with why
//! they stay.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::SystemTime;

use agentz_protocol::storage::{CheckoutStorage, Storage, StorageCache};
use agentz_protocol::workspace::WorkspaceRemoval;
use gpui::{AnyElement, App, Context, Entity, Hsla, Window};
use projects::{ProjectId, Thread, ThreadId, WorkspaceKind};
use ui::{Tooltip, prelude::*};
use util::ResultExt as _;

use super::skills::render_error;
use super::{SettingsPage, SettingsPageEvent, render_rows};
use crate::agent_icons::agent_icon;
use crate::confirm_dialog::ConfirmRequest;
use crate::controls::ACCOUNT_COLORS;
use crate::machines::MachineId;
use crate::project_info::{render_project_icon, workspace_icon};
use crate::project_store::ProjectStore;
use crate::server_client::ServerClient;
use crate::sidebar::format_relative_time;

/// An open project's biggest threads shown at first, and how many more Show more adds, as the
/// Threads tab pages an agent's sessions.
const THREADS_INITIAL_COUNT: usize = 5;
const THREADS_PAGE_COUNT: usize = 25;
const ROW_GROUP: &str = "storage-row";
/// The columns at a row's end, so sizes line up down a group.
const SIZE_WIDTH: Pixels = px(64.);
const WHEN_WIDTH: Pixels = px(34.);
const SLOT_WIDTH: Pixels = px(22.);
/// How far a project's threads are set in, past its chevron and icon.
const THREAD_INDENT: Pixels = px(34.);

/// What Settings › Storage remembers while it's open.
#[derive(Default)]
pub(super) struct StoragePage {
    /// The projects whose threads are listed.
    open_projects: HashSet<(MachineId, ProjectId)>,
    /// How many of an open project's threads show.
    threads_shown: HashMap<(MachineId, ProjectId), usize>,
    /// Why the last Delete or Clear failed.
    pub(super) error: Option<SharedString>,
}

/// The kinds the bar and the groups show, in their order.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Chats,
    Threads,
    Checkouts,
    Agents,
    Other,
}

impl Kind {
    const ALL: [Kind; 5] = [
        Kind::Chats,
        Kind::Threads,
        Kind::Checkouts,
        Kind::Agents,
        Kind::Other,
    ];

    fn name(self) -> &'static str {
        match self {
            Kind::Chats => "Chats",
            Kind::Threads => "Threads",
            Kind::Checkouts => "Worktrees and pastures",
            Kind::Agents => "Agents",
            Kind::Other => "Node.js and logs",
        }
    }

    /// The design's blue, green, yellow and purple, in the theme's shade, and gray.
    fn color(self, cx: &App) -> Hsla {
        let index = match self {
            Kind::Chats => 5,
            Kind::Threads => 3,
            Kind::Checkouts => 2,
            Kind::Agents => 6,
            Kind::Other => return cx.theme().colors().text_muted,
        };
        let (_, light, dark) = ACCOUNT_COLORS[index];
        gpui::rgb(if cx.theme().appearance().is_light() {
            light
        } else {
            dark
        })
        .into()
    }
}

/// A thread or chat's row.
struct ThreadRow {
    id: ThreadId,
    title: SharedString,
    bytes: u64,
    last_used: Option<SystemTime>,
    is_archived: bool,
    is_working: bool,
}

/// A project's row in Threads, with its threads biggest first.
struct ProjectThreads {
    project_id: ProjectId,
    bytes: u64,
    threads: Vec<ThreadRow>,
}

/// What a row shows, from its icon to the button at its end.
struct StorageRow {
    selector: String,
    lead: AnyElement,
    name: SharedString,
    detail: Option<SharedString>,
    tag: Option<(&'static str, Color)>,
    bytes: u64,
    /// When it was last used, for the rows with that column.
    when: Option<String>,
    end: Option<AnyElement>,
    is_indented: bool,
}

impl SettingsPage {
    /// The page's title, and the machine it shows, as on the Agents page.
    pub(super) fn render_storage_header(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        h_flex()
            .h(px(28.))
            .gap_4()
            .justify_between()
            .child(Headline::new("Storage").size(HeadlineSize::Small))
            .when(self.machines.read(cx).has_remotes(), |header| {
                header.child(self.render_agents_machine_picker(window, cx))
            })
            .into_any_element()
    }

    pub(super) fn render_storage(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let machine = self.agents_machine;
        let client = self.agents_client(cx);
        let label = self.machines.read(cx).label(machine, cx);
        let storage = client.read(cx).storage().clone();
        let store = self.machines.read(cx).projects(machine, cx);
        let (Some(store), true) = (store, storage.measured) else {
            let text = if client.read(cx).is_online() {
                format!("Measuring what agentZ uses on {label}…")
            } else {
                format!("{label} isn't connected.")
            };
            return vec![render_unmeasured(text, cx)];
        };

        let (chats, project_threads) = listed_threads(&storage, store.read(cx));
        let mut checkouts = storage.checkouts.clone();
        checkouts.sort_by_key(|checkout| std::cmp::Reverse(checkout.bytes));
        let mut agents = storage.agents.clone();
        agents.sort_by_key(|agent| std::cmp::Reverse(agent.bytes));
        let totals: Vec<(Kind, u64)> = Kind::ALL
            .into_iter()
            .map(|kind| {
                let bytes = match kind {
                    Kind::Chats => chats.iter().map(|chat| chat.bytes).sum(),
                    Kind::Threads => project_threads.iter().map(|project| project.bytes).sum(),
                    Kind::Checkouts => checkouts.iter().map(|checkout| checkout.bytes).sum(),
                    Kind::Agents => {
                        agents.iter().map(|agent| agent.bytes).sum::<u64>() + storage.registry_cache
                    }
                    Kind::Other => {
                        storage.node.as_ref().map_or(0, |node| node.bytes) + storage.server_log
                    }
                };
                (kind, bytes)
            })
            .collect();
        let total = |kind: Kind| {
            totals
                .iter()
                .find(|(listed, _)| *listed == kind)
                .map_or(0, |(_, bytes)| *bytes)
        };

        let mut sections = vec![render_summary(&storage, &label, &totals, cx)];
        sections.extend(self.storage_page.error.clone().map(|error| {
            div()
                .child(render_error(error, "storage-error"))
                .into_any_element()
        }));
        if !chats.is_empty() {
            let rows = chats
                .iter()
                .map(|chat| self.render_thread_row(machine, chat, false, cx))
                .collect();
            sections.push(render_group(Kind::Chats, total(Kind::Chats), rows, cx));
        }
        if !project_threads.is_empty() {
            let mut rows = Vec::new();
            for project in &project_threads {
                rows.extend(self.render_project_threads(machine, project, store.read(cx), cx));
            }
            sections.push(render_group(Kind::Threads, total(Kind::Threads), rows, cx));
        }
        if !checkouts.is_empty() {
            let rows = checkouts
                .iter()
                .map(|checkout| self.render_checkout_row(&client, checkout, store.read(cx), cx))
                .collect();
            sections.push(render_group(
                Kind::Checkouts,
                total(Kind::Checkouts),
                rows,
                cx,
            ));
        }
        let mut agent_rows: Vec<AnyElement> = agents
            .iter()
            .map(|agent| {
                let lead = match agent_icon(&agent.agent_id, cx) {
                    Some(markup) => Icon::from_svg_markup(markup),
                    None => Icon::new(IconName::Sparkle),
                }
                .size(IconSize::Small)
                .color(Color::Muted)
                .into_any_element();
                let name = self
                    .registry(cx)
                    .read(cx)
                    .agent(&agent.agent_id)
                    .map(|listing| listing.name().clone())
                    .unwrap_or_else(|| agent.agent_id.0.clone());
                render_storage_row(
                    StorageRow {
                        selector: format!("storage-agent-{}", agent.agent_id.0),
                        lead,
                        name,
                        detail: Some("Installed agent · uninstall it on the Agents page".into()),
                        tag: None,
                        bytes: agent.bytes,
                        when: None,
                        end: None,
                        is_indented: false,
                    },
                    cx,
                )
            })
            .collect();
        agent_rows.push(self.render_cache_row(
            &client,
            StorageCache::RegistryCache,
            storage.registry_cache,
            cx,
        ));
        sections.push(render_group(
            Kind::Agents,
            total(Kind::Agents),
            agent_rows,
            cx,
        ));
        let mut other_rows = Vec::new();
        if let Some(node) = &storage.node {
            other_rows.push(self.render_node_row(
                &client,
                &node.version,
                node.bytes,
                node.in_use,
                cx,
            ));
        }
        other_rows.push(self.render_cache_row(
            &client,
            StorageCache::ServerLog,
            storage.server_log,
            cx,
        ));
        sections.push(render_group(
            Kind::Other,
            total(Kind::Other),
            other_rows,
            cx,
        ));
        sections
    }

    fn render_thread_row(
        &self,
        machine: MachineId,
        thread: &ThreadRow,
        is_indented: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let id = thread.id;
        let icon = if thread.is_archived {
            IconName::Archive
        } else {
            IconName::Chat
        };
        let end = (!thread.is_working).then(|| {
            trash_button(
                format!("storage-delete-thread-{}", id.0),
                cx.listener(move |this, _, _, cx| this.confirm_delete_thread(machine, id, cx)),
            )
        });
        render_storage_row(
            StorageRow {
                selector: format!("storage-thread-{}", id.0),
                lead: Icon::new(icon)
                    .size(IconSize::Small)
                    .color(Color::Muted)
                    .into_any_element(),
                name: thread.title.clone(),
                detail: None,
                tag: thread.is_working.then_some(("Working", Color::Warning)),
                bytes: thread.bytes,
                when: Some(
                    thread
                        .last_used
                        .map(|time| format_relative_time(time, SystemTime::now()))
                        .unwrap_or_default(),
                ),
                end,
                is_indented,
            },
            cx,
        )
    }

    /// A project's row, and when it's open, its biggest threads and Show more.
    fn render_project_threads(
        &self,
        machine: MachineId,
        project: &ProjectThreads,
        store: &ProjectStore,
        cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        let key = (machine, project.project_id);
        let is_open = self.storage_page.open_projects.contains(&key);
        let (name, icon) = match store.project(project.project_id) {
            Some(listed) => (
                listed.name(),
                render_project_icon(machine, listed, px(14.), cx),
            ),
            // Workspaces threads, which belong to no project.
            None => (
                SharedString::from("Workspaces"),
                Icon::new(IconName::Folder)
                    .size(IconSize::Small)
                    .color(Color::Muted)
                    .into_any_element(),
            ),
        };
        let lead = h_flex()
            .gap_1p5()
            .child(
                Icon::new(if is_open {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                })
                .size(IconSize::XSmall)
                .color(Color::Muted),
            )
            .child(icon)
            .into_any_element();
        let count = project.threads.len();
        let row = render_storage_row(
            StorageRow {
                selector: format!("storage-project-{}", project.project_id.0),
                lead,
                name,
                detail: Some(
                    if count == 1 {
                        "1 thread".to_string()
                    } else {
                        format!("{count} threads")
                    }
                    .into(),
                ),
                tag: None,
                bytes: project.bytes,
                when: None,
                end: None,
                is_indented: false,
            },
            cx,
        );
        let mut rows = vec![
            div()
                .id(SharedString::from(format!(
                    "storage-project-toggle-{}",
                    project.project_id.0
                )))
                .cursor_pointer()
                .child(row)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let page = &mut this.storage_page;
                    if !page.open_projects.remove(&key) {
                        page.open_projects.insert(key);
                    }
                    cx.notify();
                }))
                .into_any_element(),
        ];
        if !is_open {
            return rows;
        }
        let shown = self
            .storage_page
            .threads_shown
            .get(&key)
            .copied()
            .unwrap_or(THREADS_INITIAL_COUNT);
        rows.extend(
            project
                .threads
                .iter()
                .take(shown)
                .map(|thread| self.render_thread_row(machine, thread, true, cx)),
        );
        let hidden = count.saturating_sub(shown);
        if hidden > 0 {
            rows.push(
                h_flex()
                    .id(SharedString::from(format!(
                        "storage-show-more-{}",
                        project.project_id.0
                    )))
                    .debug_selector(move || format!("storage-show-more-{}", key.1.0))
                    .min_h(px(34.))
                    .pl(THREAD_INDENT)
                    .pr_3()
                    .cursor_pointer()
                    .hover(|row| row.bg(cx.theme().colors().ghost_element_hover))
                    .child(
                        Label::new(format!("Show {} more", hidden.min(THREADS_PAGE_COUNT)))
                            .color(Color::Accent),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        *this
                            .storage_page
                            .threads_shown
                            .entry(key)
                            .or_insert(THREADS_INITIAL_COUNT) += THREADS_PAGE_COUNT;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        rows
    }

    /// A worktree or pasture, with its project, kind and what its thread is doing.
    fn render_checkout_row(
        &self,
        client: &Entity<ServerClient>,
        checkout: &CheckoutStorage,
        store: &ProjectStore,
        cx: &Context<Self>,
    ) -> AnyElement {
        let project = checkout
            .project_id
            .and_then(|project_id| store.project(project_id))
            .map_or_else(
                || SharedString::from(checkout.repository.clone()),
                |project| project.name(),
            );
        let threads: Vec<&Thread> = store
            .threads_in_folder(&checkout.path)
            .into_iter()
            .filter_map(|thread_id| store.thread(thread_id))
            .collect();
        let is_working = threads
            .iter()
            .any(|thread| store.is_thread_working(thread.id));
        let state = if is_working {
            "Its thread is working".to_string()
        } else if checkout.changed_files > 0 {
            match checkout.changed_files {
                1 => "Uncommitted changes in 1 file".to_string(),
                files => format!("Uncommitted changes in {files} files"),
            }
        } else if checkout.has_own_commits {
            "Commits the project doesn't have".to_string()
        } else {
            thread_state(&threads, SystemTime::now())
        };
        let tag = if is_working {
            Some(("Working", Color::Warning))
        } else if checkout.has_changes() {
            Some(("Has changes", Color::Success))
        } else {
            None
        };
        let name: SharedString = checkout
            .branch
            .clone()
            .or_else(|| {
                checkout
                    .path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_default()
            .into();
        let end = tag.is_none().then(|| {
            let client = client.clone();
            let checkout = checkout.clone();
            let name = name.clone();
            trash_button(
                format!("storage-delete-checkout-{name}"),
                cx.listener(move |this, _, _, cx| {
                    this.confirm_delete_checkout(&client, &checkout, &name, cx)
                }),
            )
        });
        render_storage_row(
            StorageRow {
                selector: format!("storage-checkout-{name}"),
                lead: Icon::new(workspace_icon(checkout.kind))
                    .size(IconSize::Small)
                    .color(Color::Muted)
                    .into_any_element(),
                name,
                detail: Some(format!("{project} · {} · {state}", checkout.kind.label()).into()),
                tag,
                bytes: checkout.bytes,
                when: None,
                end,
                is_indented: false,
            },
            cx,
        )
    }

    fn render_node_row(
        &self,
        client: &Entity<ServerClient>,
        version: &str,
        bytes: u64,
        in_use: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let major = version_major(version).to_string();
        let name = format!("Node.js {major}");
        let client = client.clone();
        let button = Button::new("storage-delete-node", "Delete")
            .style(ButtonStyle::Outlined)
            .size(ButtonSize::Compact)
            .label_size(LabelSize::Small)
            .disabled(in_use)
            .when(in_use, |button| {
                button.tooltip(Tooltip::text(
                    "An agent is running on it. Stop its threads to delete it.",
                ))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                let request = this.clear_request(
                    &client,
                    StorageCache::Node,
                    ConfirmRequest::delete_storage(
                        format!("Delete Node.js {major}?"),
                        format!(
                            "It takes {}. It's downloaded again when an agent needs it.",
                            format_size(bytes)
                        ),
                        |_, _| {},
                    ),
                    cx,
                );
                cx.emit(SettingsPageEvent::Confirm(request));
            }));
        render_storage_row(
            StorageRow {
                selector: "storage-node".into(),
                lead: Icon::new(IconName::Box)
                    .size(IconSize::Small)
                    .color(Color::Muted)
                    .into_any_element(),
                name: name.into(),
                detail: Some(
                    "Downloaded for agents that run on Node; downloaded again when one needs it"
                        .into(),
                ),
                tag: None,
                bytes,
                when: None,
                end: Some(
                    div()
                        .debug_selector(|| "storage-delete-node".into())
                        .child(button)
                        .into_any_element(),
                ),
                is_indented: false,
            },
            cx,
        )
    }

    /// The registry's cache or the server's log, which Clear empties.
    fn render_cache_row(
        &self,
        client: &Entity<ServerClient>,
        cache: StorageCache,
        bytes: u64,
        cx: &Context<Self>,
    ) -> AnyElement {
        let (selector, icon, name, detail) = match cache {
            StorageCache::RegistryCache => (
                "storage-registry-cache",
                IconName::AcpRegistry,
                "Registry cache",
                "The ACP Registry’s list and icons, fetched again when needed",
            ),
            StorageCache::ServerLog => (
                "storage-server-log",
                IconName::FileTextOutlined,
                "Server log",
                "Started over when it passes its limit",
            ),
            StorageCache::Node => unreachable!("Node.js has a row of its own"),
        };
        let client = client.clone();
        let button = Button::new(format!("{selector}-clear"), "Clear")
            .style(ButtonStyle::Outlined)
            .size(ButtonSize::Compact)
            .label_size(LabelSize::Small)
            .disabled(bytes == 0)
            .on_click(cx.listener(move |this, _, _, cx| {
                let size = format_size(bytes);
                let confirm = match cache {
                    StorageCache::RegistryCache => ConfirmRequest::clear_storage(
                        "Clear the registry cache?",
                        format!(
                            "It takes {size}: the ACP Registry’s list and icons, fetched again \
                             at once."
                        ),
                        |_, _| {},
                    ),
                    _ => ConfirmRequest::clear_storage(
                        "Clear the server’s log?",
                        format!("It takes {size}. What the server logged so far is gone."),
                        |_, _| {},
                    ),
                };
                let request = this.clear_request(&client, cache, confirm, cx);
                cx.emit(SettingsPageEvent::Confirm(request));
            }));
        render_storage_row(
            StorageRow {
                selector: selector.into(),
                lead: Icon::new(icon)
                    .size(IconSize::Small)
                    .color(Color::Muted)
                    .into_any_element(),
                name: name.into(),
                detail: Some(detail.into()),
                tag: None,
                bytes,
                when: None,
                end: Some(
                    div()
                        .debug_selector(move || format!("{selector}-clear"))
                        .child(button)
                        .into_any_element(),
                ),
                is_indented: false,
            },
            cx,
        )
    }

    /// `confirm`, sending the Delete or Clear once it's agreed to.
    fn clear_request(
        &self,
        client: &Entity<ServerClient>,
        cache: StorageCache,
        confirm: ConfirmRequest,
        cx: &mut Context<Self>,
    ) -> ConfirmRequest {
        let page = cx.weak_entity();
        let client = client.clone();
        ConfirmRequest {
            on_confirm: std::rc::Rc::new(move |_, cx| {
                let client = client.clone();
                page.update(cx, |page, cx| page.clear_storage(&client, cache, cx))
                    .log_err();
            }),
            ..confirm
        }
    }

    fn clear_storage(
        &mut self,
        client: &Entity<ServerClient>,
        cache: StorageCache,
        cx: &mut Context<Self>,
    ) {
        self.storage_page.error = None;
        let cleared = client.read(cx).clear_storage(cache, cx);
        cx.spawn(async move |this, cx| {
            let Err(error) = cleared.await else {
                return;
            };
            let what = match cache {
                StorageCache::Node => "Couldn't delete Node.js",
                StorageCache::ServerLog => "Couldn't clear the server’s log",
                StorageCache::RegistryCache => "Couldn't clear the registry cache",
            };
            this.update(cx, |this, cx| {
                this.storage_page.error = Some(format!("{what}: {error:#}").into());
                cx.notify();
            })
            .log_err();
        })
        .detach();
        cx.notify();
    }

    fn confirm_delete_thread(
        &mut self,
        machine: MachineId,
        thread_id: ThreadId,
        cx: &mut Context<Self>,
    ) {
        let Some(store) = self.machines.read(cx).projects(machine, cx) else {
            return;
        };
        let Some(thread) = store.read(cx).thread(thread_id).cloned() else {
            return;
        };
        let client = self.machines.read(cx).client(machine, cx);
        let bytes = client
            .and_then(|client| {
                client
                    .read(cx)
                    .storage()
                    .thread(thread_id)
                    .map(|thread| thread.bytes)
            })
            .unwrap_or_default();
        let size = format_size(bytes);
        let message = if thread.is_chat() {
            format!("It takes {size}: its conversation, images and folder. This can’t be undone.")
        } else {
            format!(
                "It takes {size}: its conversation, images and subthreads. Its worktree or \
                 pasture stays. This can’t be undone."
            )
        };
        let request = ConfirmRequest::delete_storage(
            format!("Delete “{}”?", thread.title),
            message,
            move |_, cx| {
                store.update(cx, |store, cx| store.delete_thread(thread_id, cx));
            },
        );
        cx.emit(SettingsPageEvent::Confirm(request));
    }

    fn confirm_delete_checkout(
        &mut self,
        client: &Entity<ServerClient>,
        checkout: &CheckoutStorage,
        name: &SharedString,
        cx: &mut Context<Self>,
    ) {
        let size = format_size(checkout.bytes);
        let (title, message) = match checkout.kind {
            WorkspaceKind::Worktree => (
                format!("Delete the {name} worktree?"),
                format!(
                    "It takes {size}. Its folder is deleted from disk, and its branch stays. \
                     This can’t be undone."
                ),
            ),
            WorkspaceKind::Pasture => (
                format!("Delete the {name} pasture?"),
                format!(
                    "It takes {size}. Its folder, a copy of the project, is deleted from disk. \
                     This can’t be undone."
                ),
            ),
        };
        let page = cx.weak_entity();
        let client = client.clone();
        let path = checkout.path.clone();
        let name = name.clone();
        let request = ConfirmRequest::delete_storage(title, message, move |_, cx| {
            let store = client.read(cx).projects().clone();
            let removal = store.read(cx).remove_workspace(path.clone(), false, cx);
            let name = name.clone();
            page.update(cx, |page, cx| {
                page.storage_page.error = None;
                cx.spawn(async move |this, cx| {
                    let failure = match removal.await {
                        Ok(WorkspaceRemoval::NeedsConfirmation(reason)) => reason,
                        Ok(_) => return,
                        Err(error) => format!("{error:#}"),
                    };
                    this.update(cx, |this, cx| {
                        this.storage_page.error =
                            Some(format!("Couldn't delete {name}: {failure}").into());
                        cx.notify();
                    })
                    .log_err();
                })
                .detach();
            })
            .log_err();
        });
        cx.emit(SettingsPageEvent::Confirm(request));
    }
}

/// The chats, and the other threads by project, with their sizes, biggest first. Only those
/// the thread lists show: subthreads count in their thread, and drafts keep nothing.
fn listed_threads(
    storage: &Storage,
    store: &ProjectStore,
) -> (Vec<ThreadRow>, Vec<ProjectThreads>) {
    let mut chats = Vec::new();
    let mut projects: BTreeMap<ProjectId, Vec<ThreadRow>> = BTreeMap::new();
    for measured in &storage.threads {
        let Some(thread) = store.thread(measured.thread_id) else {
            continue;
        };
        let row = ThreadRow {
            id: thread.id,
            title: thread.title.clone().into(),
            bytes: measured.bytes,
            last_used: thread.last_activity_at,
            is_archived: thread.archived_at.is_some(),
            is_working: store
                .thread_and_subthreads(thread.id)
                .into_iter()
                .any(|thread_id| store.is_thread_working(thread_id)),
        };
        if thread.is_chat() {
            chats.push(row);
        } else {
            projects.entry(thread.project_id).or_default().push(row);
        }
    }
    chats.sort_by_key(|chat| std::cmp::Reverse(chat.bytes));
    let mut projects: Vec<ProjectThreads> = projects
        .into_iter()
        .map(|(project_id, mut threads)| {
            threads.sort_by_key(|thread| std::cmp::Reverse(thread.bytes));
            ProjectThreads {
                project_id,
                bytes: threads.iter().map(|thread| thread.bytes).sum(),
                threads,
            }
        })
        .collect();
    projects.sort_by_key(|project| std::cmp::Reverse(project.bytes));
    (chats, projects)
}

/// What a checkout's threads did last, when none is working: the newest one's.
fn thread_state(threads: &[&Thread], now: SystemTime) -> String {
    let Some(thread) = threads.iter().max_by_key(|thread| thread.last_activity_at) else {
        return "Its thread was deleted".to_string();
    };
    match (thread.archived_at, thread.last_activity_at) {
        (Some(archived_at), _) => format!("Its thread was archived {}", long_ago(archived_at, now)),
        (None, Some(last_activity_at)) => {
            format!("Its thread finished {}", long_ago(last_activity_at, now))
        }
        (None, None) => "Its thread finished".to_string(),
    }
}

/// "just now", "3 hours ago", "9 days ago".
fn long_ago(time: SystemTime, now: SystemTime) -> String {
    let seconds = now.duration_since(time).unwrap_or_default().as_secs();
    let (count, unit) = match seconds {
        0..60 => return "just now".to_string(),
        60..3_600 => (seconds / 60, "minute"),
        3_600..86_400 => (seconds / 3_600, "hour"),
        86_400..604_800 => (seconds / 86_400, "day"),
        _ => (seconds / 604_800, "week"),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

fn version_major(version: &str) -> &str {
    let version = version.trim_start_matches('v');
    version.split('.').next().unwrap_or(version)
}

/// As `du -h` rounds: whole numbers from 10 up, a decimal under.
pub(crate) fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.;
    const MB: f64 = KB * 1024.;
    const GB: f64 = MB * 1024.;
    let bytes_f = bytes as f64;
    let (value, unit) = if bytes_f < KB {
        return format!("{bytes} B");
    } else if bytes_f < MB {
        (bytes_f / KB, "KB")
    } else if bytes_f < GB {
        (bytes_f / MB, "MB")
    } else {
        (bytes_f / GB, "GB")
    };
    if value < 10. {
        format!("{value:.1} {unit}")
    } else {
        format!("{value:.0} {unit}")
    }
}

/// Before the server's first measurement, or while its machine is away.
fn render_unmeasured(text: String, cx: &App) -> AnyElement {
    v_flex()
        .debug_selector(|| "storage-measuring".into())
        .gap_2()
        .child(Label::new(text).color(Color::Muted))
        .child(
            div()
                .h(px(10.))
                .w_full()
                .rounded(px(5.))
                .bg(cx.theme().colors().ghost_element_selected),
        )
        .into_any_element()
}

/// macOS's Storage settings: the total and the data folder, over a bar with a color for each
/// kind, and the kinds with their sizes under it.
fn render_summary(
    storage: &Storage,
    machine: &str,
    totals: &[(Kind, u64)],
    cx: &App,
) -> AnyElement {
    let colors = cx.theme().colors();
    let total: u64 = totals.iter().map(|(_, bytes)| bytes).sum();
    let segments = totals
        .iter()
        .filter(|(_, bytes)| *bytes > 0)
        .map(|(kind, bytes)| {
            let mut segment = div()
                .h_full()
                .min_w(px(2.))
                .flex_basis(px(0.))
                .bg(kind.color(cx));
            segment.style().flex_grow = Some(*bytes as f32);
            segment
        });
    let legend = totals.iter().map(|(kind, bytes)| {
        h_flex()
            .gap_1p5()
            .child(div().size(px(8.)).rounded(px(2.)).bg(kind.color(cx)))
            .child(
                Label::new(format!("{} {}", kind.name(), format_size(*bytes)))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
    });
    v_flex()
        .debug_selector(|| "storage-summary".into())
        .gap_2()
        .child(
            h_flex()
                .gap_4()
                .justify_between()
                .child(
                    h_flex()
                        .gap_1()
                        .child(Label::new("agentZ uses").color(Color::Muted))
                        .child(Label::new(format_size(total)).weight(gpui::FontWeight::SEMIBOLD))
                        .child(Label::new(format!("on {machine}")).color(Color::Muted)),
                )
                .child(
                    Label::new(storage.data_folder.clone())
                        .size(LabelSize::Small)
                        .color(Color::Placeholder),
                ),
        )
        .child(
            h_flex()
                .h(px(10.))
                .w_full()
                .gap(px(2.))
                .rounded(px(5.))
                .overflow_hidden()
                .bg(colors.ghost_element_selected)
                .children(segments),
        )
        .child(h_flex().flex_wrap().gap_x_3p5().gap_y_1().children(legend))
        .into_any_element()
}

/// A group's label with its total beside it, over its rows.
fn render_group(kind: Kind, total: u64, rows: Vec<AnyElement>, cx: &App) -> AnyElement {
    v_flex()
        .debug_selector(move || format!("storage-group-{}", kind.name()))
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .child(
                    Label::new(kind.name())
                        .size(LabelSize::Small)
                        .color(Color::Muted),
                )
                .child(
                    Label::new(format_size(total))
                        .size(LabelSize::Small)
                        .color(Color::Placeholder),
                ),
        )
        .child(render_rows(rows, cx))
        .into_any_element()
}

/// A row: its icon, its name over a line about it, why it stays, its size, when it was last
/// used, and its button.
fn render_storage_row(row: StorageRow, cx: &App) -> AnyElement {
    let colors = cx.theme().colors();
    let StorageRow {
        selector,
        lead,
        name,
        detail,
        tag,
        bytes,
        when,
        end,
        is_indented,
    } = row;
    h_flex()
        .debug_selector(move || selector)
        .group(ROW_GROUP)
        .min_h(px(34.))
        .py_1()
        .pr_3()
        .map(|row| {
            if is_indented {
                row.pl(THREAD_INDENT)
            } else {
                row.pl_3()
            }
        })
        .gap_2p5()
        .hover(|row| row.bg(colors.ghost_element_hover))
        .child(div().flex_none().child(lead))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(Label::new(name).truncate())
                .children(detail.map(|detail| {
                    Label::new(detail)
                        .size(LabelSize::XSmall)
                        .color(Color::Placeholder)
                        .truncate()
                })),
        )
        .children(tag.map(|(text, color)| {
            div()
                .flex_none()
                .px(px(6.))
                .py(px(1.))
                .rounded(px(4.))
                .bg(colors.ghost_element_selected)
                .child(Label::new(text).size(LabelSize::XSmall).color(color))
        }))
        .child(
            div().w(SIZE_WIDTH).flex_none().flex().justify_end().child(
                Label::new(format_size(bytes))
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            ),
        )
        .children(when.map(|when| {
            div().w(WHEN_WIDTH).flex_none().flex().justify_end().child(
                Label::new(when)
                    .size(LabelSize::XSmall)
                    .color(Color::Placeholder),
            )
        }))
        .child(
            div()
                .min_w(SLOT_WIDTH)
                .flex_none()
                .flex()
                .justify_center()
                .children(end),
        )
        .into_any_element()
}

/// The trash button a row shows on hover.
fn trash_button(
    selector: String,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    div()
        .debug_selector({
            let selector = selector.clone();
            move || selector
        })
        .visible_on_hover(ROW_GROUP)
        .child(
            IconButton::new(SharedString::from(selector), IconName::Trash)
                .icon_size(IconSize::Small)
                .tooltip(Tooltip::text("Delete…"))
                .on_click(on_click),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::Duration;

    use agentz_protocol::agents::AgentId;
    use agentz_protocol::spaces::SpacesSnapshot;
    use agentz_protocol::storage::{AgentStorage, NodeStorage, ThreadStorage};
    use agentz_protocol::{Request, Response};
    use gpui::{TestAppContext, VisualTestContext};
    use projects::ProjectsSnapshot;

    use super::super::Section;
    use super::*;

    fn thread(id: u64, project_id: ProjectId, title: &str, workspace: Option<&str>) -> Thread {
        let time = serde_json::json!({ "secs_since_epoch": 1_000, "nanos_since_epoch": 0 });
        serde_json::from_value(serde_json::json!({
            "id": id,
            "project_id": project_id,
            "title": title,
            "agent_id": "mock",
            "created_at": time,
            "last_activity_at": time,
            "workspace": workspace,
        }))
        .expect("a thread")
    }

    fn checkout(
        path: &str,
        kind: WorkspaceKind,
        branch: &str,
        changed_files: u32,
    ) -> CheckoutStorage {
        CheckoutStorage {
            path: path.into(),
            kind,
            project_id: Some(ProjectId(1)),
            repository: "agentz".into(),
            branch: Some(branch.into()),
            bytes: 300 * 1024 * 1024,
            changed_files,
            has_own_commits: false,
        }
    }

    /// agentz with three threads, one of them working in its worktree, and a chat; a clean
    /// pasture and one with changes; Node.js and the registry's cache, and an empty log.
    fn storage() -> Storage {
        let thread = |id, bytes| ThreadStorage {
            thread_id: ThreadId(id),
            bytes,
        };
        Storage {
            data_folder: "~/.agentz".into(),
            measured: true,
            threads: vec![
                thread(1, 2 * 1024 * 1024),
                thread(2, 90 * 1024 * 1024),
                thread(3, 5 * 1024 * 1024),
                thread(4, 1024 * 1024),
            ],
            checkouts: vec![
                checkout(
                    "/data/worktrees/agentz/fix",
                    WorkspaceKind::Worktree,
                    "fix",
                    0,
                ),
                checkout(
                    "/data/pastures/agentz/old",
                    WorkspaceKind::Pasture,
                    "old",
                    0,
                ),
                checkout(
                    "/data/pastures/agentz/wip",
                    WorkspaceKind::Pasture,
                    "wip",
                    3,
                ),
            ],
            agents: vec![AgentStorage {
                agent_id: AgentId::new("mock"),
                bytes: 200 * 1024 * 1024,
            }],
            registry_cache: 600 * 1024,
            node: Some(NodeStorage {
                version: "v24.11.0".into(),
                bytes: 120 * 1024 * 1024,
                in_use: false,
            }),
            server_log: 0,
        }
    }

    #[gpui::test]
    fn storage_lists_the_biggest_first_and_deletes_after_asking(cx: &mut TestAppContext) {
        let requests: Rc<RefCell<Vec<Request>>> = Rc::default();
        let client = cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            let project = serde_json::from_value(serde_json::json!({
                "id": 1,
                "path": "/work/agentz",
            }))
            .expect("a project");
            client.read(cx).projects().clone().update(cx, |store, cx| {
                store.set_snapshot(
                    ProjectsSnapshot {
                        projects: vec![project],
                        threads: vec![
                            thread(1, ProjectId(1), "Small thread", None),
                            thread(2, ProjectId(1), "Big thread", None),
                            thread(3, ProjectId::CHATS, "A chat", None),
                            thread(
                                4,
                                ProjectId(1),
                                "Working thread",
                                Some("/data/worktrees/agentz/fix"),
                            ),
                        ],
                        working_threads: vec![ThreadId(4)],
                        ..Default::default()
                    },
                    cx,
                )
            });
            let requests = requests.clone();
            client.update(cx, |client, _| {
                client.answer_for_test(move |request| {
                    requests.borrow_mut().push(request.clone());
                    match request {
                        Request::ClearStorage(_) => Some(Response::Ok),
                        Request::RemoveWorkspace { .. } => {
                            Some(Response::WorkspaceRemoval(WorkspaceRemoval::Removed))
                        }
                        _ => None,
                    }
                })
            });
            crate::machines::init_for_test(vec![client.clone()], cx);
            client
        });
        let (page, cx) = cx.add_window_view(|_, cx| SettingsPage::new(cx));
        let confirms: Rc<RefCell<Vec<ConfirmRequest>>> = Rc::default();
        cx.update(|_, cx| {
            let confirms = confirms.clone();
            cx.subscribe(&page, move |_, event: &SettingsPageEvent, _| {
                if let SettingsPageEvent::Confirm(request) = event {
                    confirms.borrow_mut().push(request.clone());
                }
            })
            .detach();
        });
        page.update_in(cx, |page, window, cx| {
            page.select(Section::Storage, window, cx)
        });
        cx.run_until_parked();
        // Until the server has measured, the page says so.
        assert!(cx.debug_bounds("storage-measuring").is_some());
        assert!(cx.debug_bounds("storage-summary").is_none());

        client.update(cx, |client, cx| client.set_storage_for_test(storage(), cx));
        cx.run_until_parked();
        assert!(cx.debug_bounds("storage-measuring").is_none());
        let top = |selector: &'static str, cx: &mut VisualTestContext| {
            cx.debug_bounds(selector)
                .unwrap_or_else(|| panic!("{selector} is shown"))
                .top()
        };
        let groups = [
            "storage-summary",
            "storage-group-Chats",
            "storage-group-Threads",
            "storage-group-Worktrees and pastures",
            "storage-group-Agents",
            "storage-group-Node.js and logs",
        ];
        for pair in groups.windows(2) {
            assert!(top(pair[0], cx) < top(pair[1], cx), "{pair:?}");
        }
        assert!(cx.debug_bounds("storage-thread-3").is_some());

        // A project opens to its threads, biggest first; a working one can't be deleted.
        assert!(cx.debug_bounds("storage-thread-2").is_none());
        let project = cx
            .debug_bounds("storage-project-1")
            .expect("the project is listed");
        cx.simulate_click(project.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        assert!(top("storage-thread-2", cx) < top("storage-thread-1", cx));
        assert!(cx.debug_bounds("storage-delete-thread-2").is_some());
        assert!(cx.debug_bounds("storage-delete-thread-4").is_none());

        // So can't a checkout whose thread works or that has changes.
        assert!(cx.debug_bounds("storage-delete-checkout-old").is_some());
        assert!(cx.debug_bounds("storage-delete-checkout-fix").is_none());
        assert!(cx.debug_bounds("storage-delete-checkout-wip").is_none());

        // Deleting asks first.
        let click_on_hover =
            |row: &'static str, button: &'static str, cx: &mut VisualTestContext| {
                let row = cx.debug_bounds(row).expect("the row is shown");
                cx.simulate_mouse_move(row.center(), None, gpui::Modifiers::none());
                let button = cx.debug_bounds(button).expect("the button is shown");
                cx.simulate_click(button.center(), gpui::Modifiers::none());
                cx.run_until_parked();
            };
        let confirm = |confirms: &Rc<RefCell<Vec<ConfirmRequest>>>, cx: &mut VisualTestContext| {
            let request = confirms.borrow_mut().pop().expect("it asks first");
            cx.update(|window, cx| (request.on_confirm)(window, cx));
            cx.run_until_parked();
            request.title
        };
        click_on_hover("storage-thread-2", "storage-delete-thread-2", cx);
        assert_eq!(confirm(&confirms, cx), "Delete “Big thread”?");
        let sent = client.read_with(cx, |client, _| client.sent_for_test());
        assert!(sent.contains(&Request::DeleteThread(ThreadId(2))));
        assert!(cx.debug_bounds("storage-thread-2").is_none());

        click_on_hover("storage-checkout-old", "storage-delete-checkout-old", cx);
        assert_eq!(confirm(&confirms, cx), "Delete the old pasture?");
        assert!(requests.borrow().contains(&Request::RemoveWorkspace {
            path: "/data/pastures/agentz/old".into(),
            force: false,
        }));

        // Node.js and the registry's cache go too; an empty log has nothing to clear.
        let click = |selector: &'static str, cx: &mut VisualTestContext| {
            let bounds = cx.debug_bounds(selector).expect("the button is shown");
            cx.simulate_click(bounds.center(), gpui::Modifiers::none());
            cx.run_until_parked();
        };
        click("storage-delete-node", cx);
        assert_eq!(confirm(&confirms, cx), "Delete Node.js 24?");
        click("storage-registry-cache-clear", cx);
        assert_eq!(confirm(&confirms, cx), "Clear the registry cache?");
        click("storage-server-log-clear", cx);
        assert!(confirms.borrow().is_empty());
        let cleared: Vec<StorageCache> = requests
            .borrow()
            .iter()
            .filter_map(|request| match request {
                Request::ClearStorage(cache) => Some(*cache),
                _ => None,
            })
            .collect();
        assert_eq!(cleared, [StorageCache::Node, StorageCache::RegistryCache]);
        assert!(page.read_with(cx, |page, _| page.storage_page.error.is_none()));
    }

    #[test]
    fn sizes_and_ages_read_as_people_say_them() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(900), "900 B");
        assert_eq!(format_size(12 * 1024), "12 KB");
        assert_eq!(format_size(600 * 1024), "600 KB");
        assert_eq!(format_size(8_600_000), "8.2 MB");
        assert_eq!(format_size(402 * 1024 * 1024), "402 MB");
        assert_eq!(format_size(4_939_212_390), "4.6 GB");
        assert_eq!(format_size(12 * 1024 * 1024 * 1024), "12 GB");

        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000_000);
        let ago = |seconds| now - Duration::from_secs(seconds);
        assert_eq!(long_ago(ago(5), now), "just now");
        assert_eq!(long_ago(ago(60), now), "1 minute ago");
        assert_eq!(long_ago(ago(3 * 3_600), now), "3 hours ago");
        assert_eq!(long_ago(ago(9 * 86_400), now), "1 week ago");
        assert_eq!(long_ago(ago(2 * 86_400), now), "2 days ago");
        assert_eq!(version_major("v24.11.0"), "24");
    }
}
