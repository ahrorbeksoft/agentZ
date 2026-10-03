//! The machines the app works with: this Mac, and those saved in Settings › Machines and
//! reached over SSH. Each has its own server, connection, and copies of that server's projects,
//! threads and agents. One machine failing never affects another.
//!
//! Views of a single thread hold that thread's machine. The sidebar, the project switcher and
//! New Thread show every machine's projects together, as t3code shows its environments.

use std::time::SystemTime;

use collections::HashMap;
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, Subscription};
use projects::{Project, ProjectId, Thread, ThreadId, ThreadOrder};
use serde::{Deserialize, Serialize};
use ui::SharedString;

use crate::app_settings::{AppSettingsStore, MachineProfile};
use crate::project_store::{ProjectStore, ProjectStoreEvent, ThreadStatus};
use crate::server_client::{ServerClient, Transport};

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
}

/// Which projects the sidebar shows threads for.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    All,
    Group(GroupKey),
}

/// A project as the projects list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectGroup {
    pub key: GroupKey,
    pub members: Vec<(MachineId, Project)>,
}

impl ProjectGroup {
    pub fn name(&self) -> SharedString {
        self.members
            .first()
            .map(|(_, project)| project.name())
            .unwrap_or_default()
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
pub fn init(cx: &mut App) {
    let settings = AppSettingsStore::global(cx);
    let machines = cx.new(|cx| {
        let mut this = Machines {
            clients: Vec::new(),
            subscriptions: HashMap::default(),
            _settings_subscription: cx
                .observe(&settings, |this: &mut Machines, _, cx| this.sync(cx)),
        };
        let local = ServerClient::new(MachineId::Local, "This Mac".into(), Transport::Local, cx);
        this.add(local, cx);
        this.sync(cx);
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
    }

    fn add(&mut self, client: Entity<ServerClient>, cx: &mut Context<Self>) {
        let machine = client.read(cx).machine();
        let projects = client.read(cx).projects().clone();
        let registry = client.read(cx).registry().clone();
        self.subscriptions.insert(
            machine,
            vec![
                cx.observe(&client, |_, _, cx| cx.notify()),
                cx.observe(&projects, |_, _, cx| cx.notify()),
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
                }),
            ],
        );
        self.clients.push(client);
    }

    /// Every machine's projects, each its own group, in each machine's order.
    pub fn project_groups(&self, cx: &App) -> Vec<ProjectGroup> {
        let order = self.thread_order(cx);
        let mut groups = Vec::new();
        for client in &self.clients {
            let client = client.read(cx);
            let machine = client.machine();
            let store = client.projects().read(cx);
            for project in store.projects() {
                groups.push((
                    latest_activity(store, project.id),
                    ProjectGroup {
                        key: GroupKey::of_project(ProjectKey {
                            machine,
                            project: project.id,
                        }),
                        members: vec![(machine, project.clone())],
                    },
                ));
            }
        }
        if order == ThreadOrder::LastActivity {
            // Stable, so projects without threads keep their added order at the end.
            groups.sort_by_key(|(activity, _)| std::cmp::Reverse(*activity));
        }
        groups.into_iter().map(|(_, group)| group).collect()
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

    /// The scope, or all projects when its project is gone.
    pub fn scope(&self, cx: &App) -> Scope {
        match &AppSettingsStore::global(cx).read(cx).settings().scope {
            Scope::Group(key) if self.group(key, cx).is_some() => Scope::Group(key.clone()),
            _ => Scope::All,
        }
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

    /// Unarchived top-level threads of the visible projects, newest or latest active first.
    pub fn active_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let groups = self.visible_groups(cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.archived_at.is_none()
                && thread.task.is_none()
                && groups
                    .iter()
                    .any(|group| group.contains(machine, thread.project_id))
        });
        let order = self.thread_order(cx);
        threads.sort_by(|(a_machine, a), (b_machine, b)| {
            let key = |thread: &Thread| match order {
                ThreadOrder::LastActivity => thread.last_activity_at,
                ThreadOrder::Created => thread.created_at.or(thread.last_activity_at),
            };
            key(b)
                .cmp(&key(a))
                .then(b_machine.cmp(a_machine))
                .then(b.id.cmp(&a.id))
        });
        threads
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

fn latest_activity(store: &ProjectStore, project: ProjectId) -> Option<SystemTime> {
    store
        .threads()
        .iter()
        .filter(|thread| thread.project_id == project)
        .filter_map(|thread| thread.last_activity_at)
        .max()
}
