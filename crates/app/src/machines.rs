//! The machines the app works with: this Mac, and those saved in Settings › Machines and
//! reached over SSH. Each has its own server, connection, and copies of that server's projects,
//! threads and agents. One machine failing never affects another.
//!
//! Views of a single thread hold that thread's machine. The sidebar, the project switcher and
//! New Thread show every machine's projects together, as t3code shows its environments.

use std::cmp::Ordering;
use std::time::SystemTime;

use collections::HashMap;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, Subscription};
pub use projects::project_at;
use projects::{Project, ProjectId, Thread, ThreadId, ThreadOrder, ThreadSection, order_key};
use serde::{Deserialize, Serialize};
use ui::{IconName, SharedString};

use crate::app_settings::{AppSettingsStore, MachineProfile};
use crate::project_store::{ProjectStore, ProjectStoreEvent, ThreadStatus};
use crate::server_client::{ServerClient, ServerClientEvent, Transport};
use crate::spaces_view::PaneKey;
use agentz_protocol::{
    MachineKind, PeerCheckout, PeerCheckouts, PeerMachine, Peers, RelayToolCall, Request, Response,
    ToolCaller, ToolResult,
};
use futures::FutureExt as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MachineId {
    /// This Mac.
    Local,
    /// A saved machine, by its profile's id.
    Remote(u64),
}

impl MachineId {
    /// Stable, for element ids and file names.
    pub fn slug(self) -> String {
        match self {
            MachineId::Local => "local".to_string(),
            MachineId::Remote(id) => format!("remote-{id}"),
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug {
            "local" => Some(MachineId::Local),
            _ => slug
                .strip_prefix("remote-")?
                .parse()
                .ok()
                .map(MachineId::Remote),
        }
    }
}

/// A thread on a machine. Thread ids are only unique per server.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ThreadKey {
    pub machine: MachineId,
    pub thread: ThreadId,
}

/// A project on a machine: one physical folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProjectKey {
    pub machine: MachineId,
    pub project: ProjectId,
}

/// One entry in the projects list, standing for one or more projects.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GroupKey(pub String);

impl GroupKey {
    pub fn of_project(key: ProjectKey) -> Self {
        Self(format!("{}/{}", key.machine.slug(), key.project.0))
    }

    /// The project, for a key made by [`Self::of_project`].
    pub fn project(&self) -> Option<ProjectKey> {
        let (machine, project) = self.0.split_once('/')?;
        Some(ProjectKey {
            machine: MachineId::from_slug(machine)?,
            project: ProjectId(project.parse().ok()?),
        })
    }
}

/// How checkouts of one repository combine in the projects list (t3code's
/// `SidebarProjectGroupingMode`). Projects without a remote never combine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectGroupingMode {
    /// Every checkout of the repository, whichever folder of it was added.
    #[default]
    Repository,
    /// Checkouts of the repository added at the same folder inside it, so a monorepo's
    /// packages stay apart.
    RepositoryPath,
    Separate,
}

impl ProjectGroupingMode {
    pub const ALL: [Self; 3] = [Self::Repository, Self::RepositoryPath, Self::Separate];

    pub fn label(self) -> &'static str {
        match self {
            Self::Repository => "Group by repository",
            Self::RepositoryPath => "Group by repository path",
            Self::Separate => "Keep separate",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Repository => "Projects from the same repository share one row.",
            Self::RepositoryPath => {
                "Projects combine only when the repository and the folder inside it match."
            }
            Self::Separate => "Every project folder gets its own row.",
        }
    }

    pub fn combines(self) -> bool {
        self != Self::Separate
    }
}

/// Which projects the sidebar shows threads for.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    All,
    Group(GroupKey),
}

/// A project as the projects list shows it: one folder, or checkouts of one repository on
/// any machines, combined (t3code's `ProjectGroup`).
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectGroup {
    pub key: GroupKey,
    pub label: SharedString,
    /// This Mac's first, then each machine's in order.
    pub members: Vec<(MachineId, Project)>,
}

impl ProjectGroup {
    pub fn name(&self) -> SharedString {
        self.label.clone()
    }

    /// The machines the group's checkouts are on, once each, in order.
    pub fn machines(&self) -> Vec<MachineId> {
        let mut machines: Vec<MachineId> = Vec::new();
        for (machine, _) in &self.members {
            if !machines.contains(machine) {
                machines.push(*machine);
            }
        }
        machines
    }

    /// The member shown for the whole group: its icon and settings.
    pub fn primary(&self) -> Option<(MachineId, &Project)> {
        self.members
            .first()
            .map(|(machine, project)| (*machine, project))
    }

    pub fn contains(&self, machine: MachineId, project: ProjectId) -> bool {
        self.members
            .iter()
            .any(|(member_machine, member)| *member_machine == machine && member.id == project)
    }
}

pub enum MachinesEvent {
    /// A thread finished a turn or started waiting for a permission answer.
    NeedsAttention(ThreadKey, ThreadStatus),
    /// A Workspaces pane's agent CLI finished working or got blocked.
    PaneNeedsAttention(PaneKey, ThreadStatus),
    /// The user archived the thread from this app.
    Archiving(ThreadKey),
}

pub struct Machines {
    /// This Mac first, then the enabled saved machines in their saved order.
    clients: Vec<Entity<ServerClient>>,
    subscriptions: HashMap<MachineId, Vec<Subscription>>,
    _settings_subscription: Subscription,
}

impl EventEmitter<MachinesEvent> for Machines {}

struct GlobalMachines(Entity<Machines>);

impl Global for GlobalMachines {}

/// Call after the app settings are loaded.
/// What the machine the app runs on is called.
pub const LOCAL_MACHINE_NAME: &str = if cfg!(target_os = "macos") {
    "This Mac"
} else {
    "This Computer"
};

pub fn init(cx: &mut App) {
    let settings = AppSettingsStore::global(cx);
    let machines = cx.new(|cx| {
        let mut this = Machines {
            clients: Vec::new(),
            subscriptions: HashMap::default(),
            _settings_subscription: cx
                .observe(&settings, |this: &mut Machines, _, cx| this.sync(cx)),
        };
        let local = ServerClient::new(
            MachineId::Local,
            LOCAL_MACHINE_NAME.into(),
            Transport::Local,
            cx,
        );
        this.add(local, cx);
        this.sync(cx);
        this
    });
    cx.set_global(GlobalMachines(machines));
}

/// Machines of the given clients only, for the views' tests.
#[cfg(test)]
pub fn init_for_test(clients: Vec<Entity<ServerClient>>, cx: &mut App) {
    let settings = AppSettingsStore::global(cx);
    let machines = cx.new(|cx| {
        let mut this = Machines {
            clients: Vec::new(),
            subscriptions: HashMap::default(),
            _settings_subscription: cx.observe(&settings, |_: &mut Machines, _, _| {}),
        };
        for client in clients {
            this.add(client, cx);
        }
        this
    });
    cx.set_global(GlobalMachines(machines));
}

impl Machines {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalMachines>().0.clone()
    }

    /// This Mac's server.
    pub fn local(cx: &App) -> Entity<ServerClient> {
        Self::global(cx).read(cx).clients[0].clone()
    }

    pub fn clients(&self) -> &[Entity<ServerClient>] {
        &self.clients
    }

    pub fn client(&self, machine: MachineId, cx: &App) -> Option<Entity<ServerClient>> {
        self.clients
            .iter()
            .find(|client| client.read(cx).machine() == machine)
            .cloned()
    }

    pub fn projects(&self, machine: MachineId, cx: &App) -> Option<Entity<ProjectStore>> {
        Some(self.client(machine, cx)?.read(cx).projects().clone())
    }

    /// Whether any saved machine is enabled. With only this Mac, nothing mentions machines.
    /// The machine's icon: its kind as its server detected it, or as chosen (t3code).
    pub fn machine_icon(&self, machine: MachineId, cx: &App) -> IconName {
        let kind = self
            .client(machine, cx)
            .map_or(MachineKind::Server, |client| {
                client.read(cx).machine_icon().kind()
            });
        machine_kind_icon(&kind)
    }

    pub fn has_remotes(&self) -> bool {
        self.clients.len() > 1
    }

    pub fn label(&self, machine: MachineId, cx: &App) -> SharedString {
        self.client(machine, cx)
            .map(|client| client.read(cx).label().clone())
            .unwrap_or_else(|| "Unknown machine".into())
    }

    pub fn is_online(&self, machine: MachineId, cx: &App) -> bool {
        self.client(machine, cx)
            .is_some_and(|client| client.read(cx).is_online())
    }

    /// Follows the saved machines: connects to newly enabled ones, and drops removed or
    /// disabled ones, whose servers keep running.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let profiles: Vec<MachineProfile> = AppSettingsStore::global(cx)
            .read(cx)
            .settings()
            .machines
            .iter()
            .filter(|profile| profile.enabled)
            .cloned()
            .collect();
        let mut changed = false;
        self.clients.retain(|client| {
            let client = client.read(cx);
            let keep = match client.machine() {
                MachineId::Local => true,
                MachineId::Remote(id) => profiles.iter().any(|profile| {
                    profile.id == id
                        && Transport::Ssh(profile.target.clone()) == *client.transport()
                }),
            };
            changed |= !keep;
            keep
        });
        let kept: Vec<MachineId> = self
            .clients
            .iter()
            .map(|client| client.read(cx).machine())
            .collect();
        self.subscriptions
            .retain(|machine, _| kept.contains(machine));
        for profile in &profiles {
            let machine = MachineId::Remote(profile.id);
            match self.client(machine, cx) {
                Some(client) => client.update(cx, |client, cx| {
                    client.set_label(profile.display_label().into(), cx)
                }),
                None => {
                    let client = ServerClient::new(
                        machine,
                        profile.display_label().into(),
                        Transport::Ssh(profile.target.clone()),
                        cx,
                    );
                    self.add(client, cx);
                    changed = true;
                }
            }
        }
        // Saved order, after this Mac.
        let order = |machine: MachineId| match machine {
            MachineId::Local => 0,
            MachineId::Remote(id) => {
                1 + profiles
                    .iter()
                    .position(|profile| profile.id == id)
                    .unwrap_or(usize::MAX - 1)
            }
        };
        self.clients
            .sort_by_cached_key(|client| order(client.read(cx).machine()));
        if changed {
            cx.notify();
        }
        self.sync_peers(cx);
    }

    /// Tells each server about the other machines and the projects combined with its own, so
    /// its agents can work there.
    fn sync_peers(&mut self, cx: &mut Context<Self>) {
        let groups = self.project_groups(cx);
        let machines: Vec<(MachineId, SharedString, bool)> = self
            .clients
            .iter()
            .map(|client| {
                let client = client.read(cx);
                (client.machine(), client.label().clone(), client.is_online())
            })
            .collect();
        for client in self.clients.clone() {
            let machine = client.read(cx).machine();
            let peers = Peers {
                this_machine: self.label(machine, cx).to_string(),
                machines: machines
                    .iter()
                    .filter(|(other, _, _)| *other != machine)
                    .map(|(_, name, online)| PeerMachine {
                        name: name.to_string(),
                        online: *online,
                    })
                    .collect(),
                checkouts: peer_checkouts(&groups, machine, |machine| {
                    self.label(machine, cx).to_string()
                }),
            };
            client.update(cx, |client, cx| client.set_peers(peers, cx));
        }
    }

    /// Runs an agent's call on the machine it names, and answers the agent's server.
    fn relay_tool_call(
        &mut self,
        source: Entity<ServerClient>,
        call: RelayToolCall,
        cx: &mut Context<Self>,
    ) {
        let target = self
            .clients
            .iter()
            .find(|client| client.read(cx).label().as_ref() == call.machine)
            .cloned();
        let response = match target {
            // Only to checkouts the server was told of, whatever it asks.
            Some(target) if source.read(cx).may_relay_to(&call.machine, &call.path) => {
                target.read(cx).request(Request::CallTool {
                    caller: ToolCaller::Relayed(call.path),
                    name: call.name,
                    arguments: call.arguments,
                })
            }
            _ => futures::future::ready(Err(anyhow::anyhow!(
                "{} has no such project in agentZ",
                call.machine
            )))
            .boxed(),
        };
        let relay_id = call.relay_id;
        cx.spawn(async move |_, cx| {
            let result = match response.await {
                Ok(Response::ToolResult(result)) => result,
                Ok(response) => relay_failure(format!("unexpected response: {response:?}")),
                Err(error) => relay_failure(format!("{error:#}")),
            };
            cx.update(|cx| {
                source
                    .read(cx)
                    .send(Request::RelayToolResult { relay_id, result }, cx)
            });
        })
        .detach();
    }

    fn add(&mut self, client: Entity<ServerClient>, cx: &mut Context<Self>) {
        let machine = client.read(cx).machine();
        let projects = client.read(cx).projects().clone();
        let registry = client.read(cx).registry().clone();
        self.subscriptions.insert(
            machine,
            vec![
                cx.observe(&client, |this, _, cx| {
                    this.sync_peers(cx);
                    cx.notify()
                }),
                cx.observe(&projects, |this, _, cx| {
                    this.sync_peers(cx);
                    cx.notify()
                }),
                cx.subscribe(&client, move |this, client, event, cx| match event {
                    ServerClientEvent::RelayToolCall(call) => {
                        this.relay_tool_call(client, call.clone(), cx)
                    }
                    ServerClientEvent::PaneNeedsAttention(pane, status) => {
                        cx.emit(MachinesEvent::PaneNeedsAttention(
                            PaneKey {
                                machine,
                                pane: *pane,
                            },
                            *status,
                        ))
                    }
                }),
                cx.observe(&registry, |_, _, cx| cx.notify()),
                cx.subscribe(&projects, move |_, _, event, cx| match event {
                    ProjectStoreEvent::NeedsAttention(thread, status) => {
                        cx.emit(MachinesEvent::NeedsAttention(
                            ThreadKey {
                                machine,
                                thread: *thread,
                            },
                            *status,
                        ))
                    }
                    ProjectStoreEvent::Archiving(thread) => {
                        cx.emit(MachinesEvent::Archiving(ThreadKey {
                            machine,
                            thread: *thread,
                        }))
                    }
                }),
            ],
        );
        self.clients.push(client);
    }

    /// Every machine's projects, combined as the grouping settings say, in the machines'
    /// order or by latest activity.
    pub fn project_groups(&self, cx: &App) -> Vec<ProjectGroup> {
        let settings = AppSettingsStore::global(cx).read(cx).settings();
        let mut projects = Vec::new();
        let mut activity = HashMap::default();
        for client in &self.clients {
            let client = client.read(cx);
            let machine = client.machine();
            let store = client.projects().read(cx);
            for project in store.projects() {
                activity.insert(
                    ProjectKey {
                        machine,
                        project: project.id,
                    },
                    latest_activity(store, project.id),
                );
                projects.push((machine, project.clone()));
            }
        }
        let mut groups = build_project_groups(
            projects,
            settings.project_grouping,
            &settings.project_grouping_overrides,
        );
        if self.thread_order(cx) == ThreadOrder::LastActivity {
            let group_activity = |group: &ProjectGroup| {
                group
                    .members
                    .iter()
                    .filter_map(|(machine, project)| {
                        *activity.get(&ProjectKey {
                            machine: *machine,
                            project: project.id,
                        })?
                    })
                    .max()
            };
            // Stable, so projects without threads keep their added order at the end.
            groups.sort_by_cached_key(|group| std::cmp::Reverse(group_activity(group)));
        }
        groups
    }

    /// The machines a group is on, for its badge (t3code's `ProjectEnvironmentBadge`):
    /// nothing when it's only on this Mac.
    pub fn group_machines_label(&self, group: &ProjectGroup, cx: &App) -> Option<SharedString> {
        let machines = group.machines();
        if machines == [MachineId::Local] {
            return None;
        }
        let labels: Vec<SharedString> = machines
            .into_iter()
            .map(|machine| self.label(machine, cx))
            .collect();
        Some(labels.join(", ").into())
    }

    /// The checkout of the group whose newest thread is newest, which New Thread offers first.
    pub fn last_used_member(&self, group: &ProjectGroup, cx: &App) -> Option<ProjectKey> {
        self.threads_where(cx, |machine, thread| {
            group.contains(machine, thread.project_id)
        })
        .into_iter()
        .max_by_key(|(_, thread)| thread.created_at.or(thread.last_activity_at))
        .map(|(machine, thread)| ProjectKey {
            machine,
            project: thread.project_id,
        })
    }

    /// The checkout New Thread starts in: the one used last while its machine is online, or
    /// else the first online one. The draft's machine picker moves it to another.
    pub fn new_thread_member(&self, group: &ProjectGroup, cx: &App) -> Option<ProjectKey> {
        self.last_used_member(group, cx)
            .filter(|member| self.is_online(member.machine, cx))
            .or_else(|| {
                group
                    .members
                    .iter()
                    .find(|(machine, _)| self.is_online(*machine, cx))
                    .map(|(machine, project)| ProjectKey {
                        machine: *machine,
                        project: project.id,
                    })
            })
    }

    /// Whether every machine the group is on is unreachable.
    pub fn is_group_offline(&self, group: &ProjectGroup, cx: &App) -> bool {
        group
            .machines()
            .into_iter()
            .all(|machine| !self.is_online(machine, cx))
    }

    pub fn group(&self, key: &GroupKey, cx: &App) -> Option<ProjectGroup> {
        self.project_groups(cx)
            .into_iter()
            .find(|group| &group.key == key)
    }

    /// The group a project belongs to.
    pub fn group_of(
        &self,
        machine: MachineId,
        project: ProjectId,
        cx: &App,
    ) -> Option<ProjectGroup> {
        self.project_groups(cx)
            .into_iter()
            .find(|group| group.contains(machine, project))
    }

    /// The groups the scope shows.
    pub fn visible_groups(&self, cx: &App) -> Vec<ProjectGroup> {
        let groups = self.project_groups(cx);
        match self.scope(cx) {
            Scope::All => groups,
            Scope::Group(key) => groups
                .into_iter()
                .filter(|group| group.key == key)
                .collect(),
        }
    }

    /// The scope, or all projects when its project is gone. A project combined or split off
    /// since keeps its scope, by the group it's in now.
    pub fn scope(&self, cx: &App) -> Scope {
        let Scope::Group(key) = &AppSettingsStore::global(cx).read(cx).settings().scope else {
            return Scope::All;
        };
        let groups = self.project_groups(cx);
        groups
            .iter()
            .find(|group| &group.key == key)
            .or_else(|| {
                let project = key.project()?;
                groups
                    .iter()
                    .find(|group| group.contains(project.machine, project.project))
            })
            .map_or(Scope::All, |group| Scope::Group(group.key.clone()))
    }

    pub fn set_scope(scope: Scope, cx: &mut App) {
        AppSettingsStore::global(cx).update(cx, |settings, cx| {
            settings.update(|settings| settings.scope = scope, cx)
        });
    }

    /// The sidebar's order, kept by this Mac's server.
    pub fn thread_order(&self, cx: &App) -> ThreadOrder {
        self.clients[0].read(cx).projects().read(cx).thread_order()
    }

    pub fn archived_expanded(&self, cx: &App) -> bool {
        self.clients[0]
            .read(cx)
            .projects()
            .read(cx)
            .archived_expanded()
    }

    pub fn workspaces_expanded(&self, cx: &App) -> bool {
        self.clients[0]
            .read(cx)
            .projects()
            .read(cx)
            .workspaces_expanded()
    }

    /// Unarchived top-level threads of the visible projects: the pinned ones in the order the
    /// user gave them, then the rest, as the user arranged them below the new ones or latest
    /// active first. Shells aren't threads until an agent CLI runs in them, nor drafts until
    /// their first message.
    pub fn active_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let groups = self.visible_groups(cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.archived_at.is_none()
                && thread.task.is_none()
                && !thread.is_draft
                && !thread.in_workspaces()
                && !self.is_shell(machine, thread, cx)
                && self.is_thread_visible(&groups, machine, thread, cx)
        });
        let order = self.thread_order(cx);
        threads.sort_by(|(a_machine, a), (b_machine, b)| {
            let created = |thread: &Thread| thread.created_at.or(thread.last_activity_at);
            let placement = match (a.is_pinned(), b.is_pinned()) {
                (true, false) => Ordering::Less,
                (false, true) => Ordering::Greater,
                (true, true) => order_key::compare(
                    (a.pin_order_key.as_deref(), a.pinned_at),
                    (b.pin_order_key.as_deref(), b.pinned_at),
                    false,
                ),
                (false, false) => match order {
                    ThreadOrder::LastActivity => {
                        return by_latest_activity((*a_machine, a), (*b_machine, b));
                    }
                    ThreadOrder::Created => order_key::compare(
                        (a.active_order_key.as_deref(), created(a)),
                        (b.active_order_key.as_deref(), created(b)),
                        true,
                    ),
                },
            };
            placement
                .then(b_machine.cmp(a_machine))
                .then(b.id.cmp(&a.id))
        });
        threads
    }

    /// The order keys of every machine's threads in `section`, shown or not: those a reorder
    /// leaves free.
    pub fn order_keys(&self, section: ThreadSection, cx: &App) -> HashMap<ThreadKey, String> {
        let mut keys = HashMap::default();
        for (machine, thread) in self.threads_where(cx, |_, thread| thread.archived_at.is_none()) {
            let key = match section {
                ThreadSection::Pinned if thread.is_pinned() => thread.pin_order_key,
                ThreadSection::Active if !thread.is_pinned() => thread.active_order_key,
                _ => None,
            };
            if let Some(key) = key {
                keys.insert(
                    ThreadKey {
                        machine,
                        thread: thread.id,
                    },
                    key,
                );
            }
        }
        keys
    }

    /// Pins the thread above every pinned thread on any machine, as a menu does (t3code's
    /// `topOfPinnedRunOrderKey`).
    pub fn pin_thread(machines: &Entity<Self>, key: ThreadKey, cx: &mut App) {
        let machines = machines.read(cx);
        let keys = machines.order_keys(ThreadSection::Pinned, cx);
        let order_key = order_key::before_all(keys.values().map(String::as_str));
        let Some(store) = machines.projects(key.machine, cx) else {
            return;
        };
        store.update(cx, |store, cx| store.pin_thread(key.thread, order_key, cx));
    }

    /// Drafts of the visible projects with something typed in them, newest first: the
    /// sidebar lists them above the threads, as t3code does.
    pub fn typed_drafts(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let groups = self.visible_groups(cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.is_draft
                && thread.unsent_text.is_some()
                && thread.archived_at.is_none()
                && thread.task.is_none()
                && !thread.in_workspaces()
                && self.is_thread_visible(&groups, machine, thread, cx)
        });
        threads.sort_by_key(|(_, thread)| std::cmp::Reverse((thread.created_at, thread.id)));
        threads
    }

    /// Threads started in a workspace pane, latest active first, for the sidebar's Workspaces
    /// section: under a project, those whose folder is in it.
    pub fn workspaces_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let groups = self.visible_groups(cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.in_workspaces()
                && thread.archived_at.is_none()
                && thread.task.is_none()
                && !thread.is_draft
                && self.is_thread_visible(&groups, machine, thread, cx)
        });
        threads.sort_by_key(|(_, thread)| {
            std::cmp::Reverse(thread.last_activity_at.or(thread.created_at))
        });
        threads
    }

    /// Unarchived terminal threads of the visible projects running no agent CLI, latest active
    /// first, for the sidebar's Shells shelf.
    pub fn shell_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let groups = self.visible_groups(cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.archived_at.is_none()
                && thread.task.is_none()
                && !thread.in_workspaces()
                && self.is_shell(machine, thread, cx)
                && self.is_thread_visible(&groups, machine, thread, cx)
        });
        threads.sort_by_key(|(_, thread)| {
            std::cmp::Reverse(thread.last_activity_at.or(thread.created_at))
        });
        threads
    }

    /// The project a thread is listed under. A terminal thread belongs to where it is now,
    /// and a Workspaces thread to where it works: the project its folder is in, or none
    /// outside every project.
    pub fn thread_project(
        &self,
        machine: MachineId,
        thread: &Thread,
        cx: &App,
    ) -> Option<ProjectId> {
        let Some(store) = self.projects(machine, cx) else {
            return Some(thread.project_id);
        };
        let store = store.read(cx);
        if thread.in_workspaces() {
            return store.thread_project(thread.id);
        }
        match store.terminal_folder(thread.id) {
            Some(folder) => project_at(store.projects(), &folder.path).map(|project| project.id),
            None => Some(thread.project_id),
        }
    }

    /// Whether the scope's groups show the thread. One in no project shows with all
    /// projects.
    fn is_thread_visible(
        &self,
        groups: &[ProjectGroup],
        machine: MachineId,
        thread: &Thread,
        cx: &App,
    ) -> bool {
        match self.thread_project(machine, thread, cx) {
            Some(project_id) => groups
                .iter()
                .any(|group| group.contains(machine, project_id)),
            None => self.scope(cx) == Scope::All,
        }
    }

    /// A terminal thread with no agent CLI running in it.
    pub fn is_shell(&self, machine: MachineId, thread: &Thread, cx: &App) -> bool {
        thread.terminal.is_some()
            && self
                .projects(machine, cx)
                .is_none_or(|store| store.read(cx).terminal_agent(thread.id).is_none())
    }

    /// Archived top-level threads of every project, most recently archived first.
    pub fn archived_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let groups = self.visible_groups(cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.archived_at.is_some()
                && thread.task.is_none()
                && groups
                    .iter()
                    .any(|group| group.contains(machine, thread.project_id))
        });
        threads.sort_by_key(|(_, thread)| std::cmp::Reverse(thread.archived_at));
        threads
    }

    fn threads_where(
        &self,
        cx: &App,
        filter: impl Fn(MachineId, &Thread) -> bool,
    ) -> Vec<(MachineId, Thread)> {
        let mut threads = Vec::new();
        for client in &self.clients {
            let client = client.read(cx);
            let machine = client.machine();
            for thread in client.projects().read(cx).threads() {
                if filter(machine, thread) {
                    threads.push((machine, thread.clone()));
                }
            }
        }
        threads
    }

    /// The most pressing status among the group's threads.
    pub fn group_status(&self, group: &ProjectGroup, cx: &App) -> Option<ThreadStatus> {
        group
            .members
            .iter()
            .filter_map(|(machine, project)| {
                self.projects(*machine, cx)?
                    .read(cx)
                    .project_status(project.id)
            })
            .max()
    }
}

/// How the unpinned threads follow [`ThreadOrder::LastActivity`]: latest active first.
pub fn by_latest_activity(
    (a_machine, a): (MachineId, &Thread),
    (b_machine, b): (MachineId, &Thread),
) -> Ordering {
    b.last_activity_at
        .cmp(&a.last_activity_at)
        .then(b_machine.cmp(&a_machine))
        .then(b.id.cmp(&a.id))
}

/// t3code's `buildProjectGroups`: a group per logical key, members in the order given, and
/// t3code's label rule for groups of several.
fn build_project_groups(
    projects: Vec<(MachineId, Project)>,
    mode: ProjectGroupingMode,
    overrides: &std::collections::BTreeMap<GroupKey, ProjectGroupingMode>,
) -> Vec<ProjectGroup> {
    let mut groups: Vec<ProjectGroup> = Vec::new();
    for (machine, project) in projects {
        let physical = GroupKey::of_project(ProjectKey {
            machine,
            project: project.id,
        });
        let mode = overrides.get(&physical).copied().unwrap_or(mode);
        let key = logical_key(&project, mode).unwrap_or(physical);
        match groups.iter_mut().find(|group| group.key == key) {
            Some(group) => group.members.push((machine, project)),
            None => groups.push(ProjectGroup {
                key,
                label: project.name(),
                members: vec![(machine, project)],
            }),
        }
    }
    for group in &mut groups {
        if group.members.len() > 1 {
            group.label = group_label(&group.members);
        }
    }
    groups
}

/// The key projects combine by, or `None` to stand alone.
fn logical_key(project: &Project, mode: ProjectGroupingMode) -> Option<GroupKey> {
    let repository = project.repository.as_ref()?;
    let key = match mode {
        ProjectGroupingMode::Separate => return None,
        ProjectGroupingMode::Repository => repository.canonical_key.clone(),
        ProjectGroupingMode::RepositoryPath => {
            match path_in_repository(&project.path, &repository.root_path) {
                Some(relative) if !relative.is_empty() => {
                    format!("{}::{relative}", repository.canonical_key)
                }
                _ => repository.canonical_key.clone(),
            }
        }
    };
    Some(GroupKey(format!("repository:{key}")))
}

/// The project's folder inside its repository, with `/` separators; empty at the top.
fn path_in_repository(path: &std::path::Path, root: &std::path::Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    Some(
        relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// t3code's `deriveProjectGroupLabel`: the members' shared name unless it's just the
/// repository's, else `owner/repo`, else the repository's name, else the first member's.
fn group_label(members: &[(MachineId, Project)]) -> SharedString {
    fn unique(values: impl Iterator<Item = Option<String>>) -> Vec<String> {
        let mut unique: Vec<String> = Vec::new();
        for value in values.flatten() {
            let value = value.trim().to_string();
            if !value.is_empty() && !unique.contains(&value) {
                unique.push(value);
            }
        }
        unique
    }
    let names = unique(
        members
            .iter()
            .map(|(_, project)| Some(project.name().to_string())),
    );
    let display_names = unique(members.iter().map(|(_, project)| {
        project
            .repository
            .as_ref()
            .and_then(|repository| repository.display_name.clone())
    }));
    let repository_names = unique(members.iter().map(|(_, project)| {
        project
            .repository
            .as_ref()
            .and_then(|repository| repository.name.clone())
    }));
    match (
        names.as_slice(),
        display_names.as_slice(),
        repository_names.as_slice(),
    ) {
        ([name], _, _) if !display_names.contains(name) && !repository_names.contains(name) => {
            name.clone().into()
        }
        (_, [display_name], _) => display_name.clone().into(),
        (_, _, [repository_name]) => repository_name.clone().into(),
        _ => members
            .first()
            .map(|(_, project)| project.name())
            .unwrap_or_default(),
    }
}

/// For each of the machine's projects combined with others, those on other machines.
fn peer_checkouts(
    groups: &[ProjectGroup],
    machine: MachineId,
    label: impl Fn(MachineId) -> String,
) -> Vec<PeerCheckouts> {
    let mut checkouts = Vec::new();
    for group in groups {
        let elsewhere: Vec<PeerCheckout> = group
            .members
            .iter()
            .filter(|(member_machine, _)| *member_machine != machine)
            .map(|(member_machine, project)| PeerCheckout {
                machine: label(*member_machine),
                path: project.path.clone(),
            })
            .collect();
        if elsewhere.is_empty() {
            continue;
        }
        for (member_machine, project) in &group.members {
            if *member_machine == machine {
                checkouts.push(PeerCheckouts {
                    project_id: project.id,
                    checkouts: elsewhere.clone(),
                });
            }
        }
    }
    checkouts
}

fn relay_failure(message: String) -> ToolResult {
    ToolResult {
        value: serde_json::json!({"code": "machine_unavailable", "message": message}),
        is_error: true,
    }
}

fn latest_activity(store: &ProjectStore, project: ProjectId) -> Option<SystemTime> {
    store
        .threads()
        .iter()
        .filter(|thread| thread.project_id == project)
        .filter_map(|thread| thread.last_activity_at)
        .max()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use projects::{Project, ProjectId, RepositoryIdentity};

    use super::{GroupKey, MachineId, ProjectGroupingMode, ProjectKey, build_project_groups};

    fn identity() -> RepositoryIdentity {
        RepositoryIdentity {
            canonical_key: "github.com/t3tools/t3code".into(),
            root_path: PathBuf::from("/work/t3code"),
            remote_name: "upstream".into(),
            remote_url: "https://github.com/t3tools/t3code.git".into(),
            display_name: Some("T3 Code".into()),
            owner: Some("t3tools".into()),
            name: Some("t3code".into()),
        }
    }

    fn project(id: u64, path: &str, custom_name: Option<&str>) -> Project {
        let mut repository = identity();
        repository.root_path = PathBuf::from(path);
        Project {
            id: ProjectId(id),
            path: PathBuf::from(path),
            custom_name: custom_name.map(str::to_string),
            icon: None,
            workspaces: Vec::new(),
            repository: Some(repository),
        }
    }

    fn clones() -> Vec<(MachineId, Project)> {
        vec![
            (MachineId::Local, project(1, "/work/t3code", None)),
            (MachineId::Local, project(2, "/work/t3code-2", None)),
            (MachineId::Remote(1), project(1, "/home/me/t3code-3", None)),
        ]
    }

    fn member_ids(groups: &[super::ProjectGroup]) -> Vec<Vec<(MachineId, u64)>> {
        groups
            .iter()
            .map(|group| {
                group
                    .members
                    .iter()
                    .map(|(machine, project)| (*machine, project.id.0))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn combines_every_clone_in_repository_modes() {
        for mode in [
            ProjectGroupingMode::Repository,
            ProjectGroupingMode::RepositoryPath,
        ] {
            let groups = build_project_groups(clones(), mode, &BTreeMap::new());
            assert_eq!(
                member_ids(&groups),
                [vec![
                    (MachineId::Local, 1),
                    (MachineId::Local, 2),
                    (MachineId::Remote(1), 1)
                ]]
            );
            assert_eq!(groups[0].label.as_ref(), "T3 Code");
        }
    }

    #[test]
    fn labels_like_t3code() {
        let named = vec![
            (
                MachineId::Local,
                project(1, "/work/a", Some("Custom project")),
            ),
            (
                MachineId::Local,
                project(2, "/work/b", Some("Custom project")),
            ),
        ];
        let groups = build_project_groups(named, ProjectGroupingMode::Repository, &BTreeMap::new());
        assert_eq!(groups[0].label.as_ref(), "Custom project");

        let repository_named = vec![
            (MachineId::Local, project(1, "/work/a/t3code", None)),
            (MachineId::Remote(1), project(1, "/srv/t3code", None)),
        ];
        let groups = build_project_groups(
            repository_named,
            ProjectGroupingMode::Repository,
            &BTreeMap::new(),
        );
        assert_eq!(groups[0].label.as_ref(), "T3 Code");

        let alone = build_project_groups(
            vec![(MachineId::Local, project(1, "/work/t3code", None))],
            ProjectGroupingMode::Repository,
            &BTreeMap::new(),
        );
        assert_eq!(alone[0].label.as_ref(), "t3code");
    }

    #[test]
    fn keeps_clones_apart_when_asked() {
        let groups =
            build_project_groups(clones(), ProjectGroupingMode::Separate, &BTreeMap::new());
        assert_eq!(groups.len(), 3);
        assert_eq!(
            groups
                .iter()
                .map(|group| group.label.to_string())
                .collect::<Vec<_>>(),
            ["t3code", "t3code-2", "t3code-3"]
        );

        let overrides = BTreeMap::from([(
            GroupKey::of_project(ProjectKey {
                machine: MachineId::Local,
                project: ProjectId(2),
            }),
            ProjectGroupingMode::Separate,
        )]);
        let groups = build_project_groups(clones(), ProjectGroupingMode::Repository, &overrides);
        assert_eq!(
            member_ids(&groups),
            [
                vec![(MachineId::Local, 1), (MachineId::Remote(1), 1)],
                vec![(MachineId::Local, 2)]
            ]
        );
    }

    #[test]
    fn repository_path_mode_keeps_monorepo_folders_apart() {
        let mut app = project(1, "/work/mono/apps/web", None);
        let mut server = project(2, "/work/mono/apps/server", None);
        let mut remote_app = project(1, "/srv/mono/apps/web", None);
        for (project, root) in [
            (&mut app, "/work/mono"),
            (&mut server, "/work/mono"),
            (&mut remote_app, "/srv/mono"),
        ] {
            if let Some(repository) = &mut project.repository {
                repository.root_path = PathBuf::from(root);
            }
        }
        let projects = vec![
            (MachineId::Local, app),
            (MachineId::Local, server),
            (MachineId::Remote(1), remote_app),
        ];
        let by_path = build_project_groups(
            projects.clone(),
            ProjectGroupingMode::RepositoryPath,
            &BTreeMap::new(),
        );
        assert_eq!(
            member_ids(&by_path),
            [
                vec![(MachineId::Local, 1), (MachineId::Remote(1), 1)],
                vec![(MachineId::Local, 2)]
            ]
        );
        let by_repository =
            build_project_groups(projects, ProjectGroupingMode::Repository, &BTreeMap::new());
        assert_eq!(by_repository.len(), 1);
    }

    #[test]
    fn a_project_is_found_from_its_own_key() {
        let key = ProjectKey {
            machine: MachineId::Remote(7),
            project: ProjectId(3),
        };
        assert_eq!(GroupKey::of_project(key).project(), Some(key));
        assert_eq!(GroupKey("repository:github.com/a/b".into()).project(), None);
    }

    #[test]
    fn folders_without_a_remote_stand_alone() {
        let mut first = project(1, "/work/a", None);
        let mut second = project(2, "/work/b", None);
        first.repository = None;
        second.repository = None;
        let groups = build_project_groups(
            vec![(MachineId::Local, first), (MachineId::Local, second)],
            ProjectGroupingMode::Repository,
            &BTreeMap::new(),
        );
        assert_eq!(groups.len(), 2);
    }
}

/// t3code's icon for each kind of machine.
pub fn machine_kind_icon(kind: &MachineKind) -> IconName {
    match kind {
        MachineKind::Server | MachineKind::Unknown(_) => IconName::Server,
        MachineKind::Cloud => IconName::Cloud,
        MachineKind::Linux => IconName::Linux,
        MachineKind::Desktop => IconName::Screen,
        MachineKind::Laptop => IconName::Laptop,
        MachineKind::MacMini => IconName::MacMini,
        MachineKind::MacStudio => IconName::MacStudio,
    }
}
