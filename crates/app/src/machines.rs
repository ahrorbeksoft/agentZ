//! The machines the app works with: this Mac, and those saved in Settings › Machines and
//! reached over SSH. Each has its own server, connection, and copies of that server's projects,
//! threads and agents. One machine failing never affects another.
//!
//! Views of a single thread hold that thread's machine. The sidebar, the project switcher and
//! New Thread show every machine's projects together, as t3code shows its environments.

use std::cmp::Ordering;
use std::time::SystemTime;

use collections::{HashMap, HashSet};
use gpui::{App, AppContext as _, Context, Entity, EventEmitter, Global, Subscription, Task};
pub use projects::project_at;
use projects::{
    Project, ProjectId, RemoteThread, Thread, ThreadId, ThreadOrder, ThreadSection, order_key,
};
use serde::{Deserialize, Serialize};
use ui::{IconName, SharedString};

use crate::app_settings::{AppSettingsStore, CloudLink, MachineProfile};
use crate::cloud_machines;
use crate::project_store::{ProjectStore, ProjectStoreEvent, ThreadStatus};
use crate::server_client::{ServerClient, ServerClientEvent, Transport};
use crate::shared_settings::{self, ListStep, SharedMachineSettings, SharedSection, list_steps};
use crate::spaces_view::PaneKey;
use agentz_client::ssh::Ssh;
use agentz_protocol::artifacts::{
    Artifact, ArtifactId, ArtifactRequest, DesignDefaults, DesignSettings, PageTheme,
};
use agentz_protocol::cloud::{CloudActivity, CloudNotice, SshAddress};
use agentz_protocol::spaces::{PaneAgentState, PaneContent};
use agentz_protocol::subthread_roles::TaskRole;
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
    /// Chats only, as if they were a project of their own.
    Chats,
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
    /// The user snoozed the thread from this app.
    Snoozing(ThreadKey),
    /// A page asks agentZ to show its thread.
    ShowThread(ThreadKey),
    /// A machine's agent published an artifact, or a new version of one.
    ArtifactPublished(MachineId, Artifact),
}

pub struct Machines {
    /// This Mac first, then the enabled saved machines in their saved order.
    clients: Vec<Entity<ServerClient>>,
    subscriptions: HashMap<MachineId, Vec<Subscription>>,
    /// Where `ssh` dials each SSH machine's target; `None` while it's looked up, or when `ssh`
    /// couldn't say.
    ssh_addresses: HashMap<String, Option<SshAddress>>,
    /// The machines a window has something of open: a thread, a terminal, or the workspace
    /// it shows.
    open_machines: HashSet<MachineId>,
    /// The last of this Mac's server's cloud notices told to the user.
    cloud_notices_told: u64,
    /// Requests giving a machine Shared's skills and MCP servers, while they're sent.
    shared_list_syncs: HashMap<MachineId, Task<()>>,
    /// The steps last tried for each machine's lists, this connection.
    shared_list_attempts: HashMap<MachineId, Vec<ListStep>>,
    _settings_subscription: Subscription,
    _wake_subscription: Subscription,
}

impl EventEmitter<MachinesEvent> for Machines {}

struct GlobalMachines(Entity<Machines>);

impl Global for GlobalMachines {}

/// How the app reaches a saved machine: over SSH, or as its provider's machine.
pub fn transport_of(profile: &MachineProfile) -> Transport {
    match &profile.cloud {
        Some(link) => Transport::Cloud {
            link: link.clone(),
            target: (!profile.target.is_empty()).then(|| profile.target.clone()),
        },
        None => Transport::Ssh(profile.target.clone()),
    }
}

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
            ssh_addresses: HashMap::default(),
            open_machines: HashSet::default(),
            cloud_notices_told: 0,
            shared_list_syncs: HashMap::default(),
            shared_list_attempts: HashMap::default(),
            _settings_subscription: cx
                .observe(&settings, |this: &mut Machines, _, cx| this.sync(cx)),
            _wake_subscription: Machines::observe_wake(cx),
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
            ssh_addresses: HashMap::default(),
            open_machines: HashSet::default(),
            cloud_notices_told: 0,
            shared_list_syncs: HashMap::default(),
            shared_list_attempts: HashMap::default(),
            _settings_subscription: cx.observe(&settings, |this: &mut Machines, _, cx| {
                this.sync_servers(cx)
            }),
            _wake_subscription: Machines::observe_wake(cx),
        };
        for client in clients {
            this.add(client, cx);
        }
        // As a connection would, so the views' tests start from what's already been sent.
        this.sync_servers(cx);
        this
    });
    cx.set_global(GlobalMachines(machines));
}

impl Machines {
    pub fn global(cx: &App) -> Entity<Self> {
        cx.global::<GlobalMachines>().0.clone()
    }

    fn observe_wake(cx: &mut Context<Self>) -> Subscription {
        let this = cx.weak_entity();
        cx.on_system_wake(move |cx| {
            this.update(cx, |this, cx| {
                for client in &this.clients {
                    client.update(cx, |client, _| client.system_woke());
                }
            })
            .ok();
        })
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

    /// The machine by the name servers know it by, which is its label ([`Peers`]).
    pub fn machine_named(&self, name: &str, cx: &App) -> Option<MachineId> {
        self.clients
            .iter()
            .map(|client| client.read(cx))
            .find(|client| client.label().as_ref() == name)
            .map(|client| client.machine())
    }

    /// A thread on another machine, as a server names it.
    pub fn remote_thread(&self, remote: &RemoteThread, cx: &App) -> Option<ThreadKey> {
        Some(ThreadKey {
            machine: self.machine_named(&remote.machine, cx)?,
            thread: remote.thread,
        })
    }

    /// The thread that works for this one: for a task delegated to another machine, its
    /// thread there, else the thread itself.
    pub fn working_thread(&self, key: ThreadKey, cx: &App) -> ThreadKey {
        self.projects(key.machine, cx)
            .and_then(|store| {
                let runs_on = store.read(cx).thread(key.thread)?.runs_on()?.clone();
                self.remote_thread(&runs_on, cx)
            })
            .unwrap_or(key)
    }

    /// The top-level thread a subthread belongs to, also when it was delegated from another
    /// machine, or the thread itself.
    pub fn root_thread(&self, key: ThreadKey, cx: &App) -> ThreadKey {
        let mut key = key;
        // Bounded, in case two machines' threads name each other as parents.
        for _ in 0..self.clients.len() {
            let Some(store) = self.projects(key.machine, cx) else {
                break;
            };
            let store = store.read(cx);
            key.thread = store.root_thread(key.thread);
            match store
                .thread(key.thread)
                .and_then(Thread::remote_parent)
                .and_then(|parent| self.remote_thread(&parent, cx))
            {
                Some(parent) => key = parent,
                None => break,
            }
        }
        key
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
                    profile.id == id && transport_of(profile) == *client.transport()
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
                        transport_of(profile),
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
        self.sync_servers(cx);
        self.link_cloud_machines(cx);
    }

    /// A new listing of the providers' machines: those waiting for theirs to run look again
    /// now, and SSH machines are linked to the ones they reach.
    fn cloud_changed(&mut self, cx: &mut Context<Self>) {
        for client in &self.clients {
            let waiting = {
                let client = client.read(cx);
                matches!(client.transport(), Transport::Cloud { .. }) && !client.is_online()
            };
            if waiting {
                client.update(cx, |client, _| client.retry());
            }
        }
        self.link_cloud_machines(cx);
        self.tell_cloud_notices(cx);
    }

    /// Shows what this Mac's server noticed of the providers' machines (design/cloud topic 12
    /// C and D) as notifications, once each, and has it drop them.
    fn tell_cloud_notices(&mut self, cx: &mut Context<Self>) {
        let local = self.clients[0].read(cx);
        let new: Vec<&CloudNotice> = local
            .cloud()
            .notices
            .iter()
            .filter(|notice| notice.id > self.cloud_notices_told)
            .collect();
        let Some(last) = new.last().map(|notice| notice.id) else {
            return;
        };
        for notice in new {
            cx.show_system_notification(cloud_machines::notification(notice));
        }
        if local.is_online() {
            local.send(Request::DismissCloudNotices(last), cx);
        }
        self.cloud_notices_told = last;
    }

    /// Which machines a window has something of open, for stopping the provider machines left
    /// idle.
    pub fn set_open_machines(&mut self, open: HashSet<MachineId>, cx: &mut Context<Self>) {
        if open != self.open_machines {
            self.open_machines = open;
            self.sync_cloud_activity(cx);
        }
    }

    /// Tells this Mac's server what's working on each provider machine and whether anything of
    /// it is open (design/cloud topic 12 B): its threads that work or wait on the user, and the
    /// agents in its workspaces.
    fn sync_cloud_activity(&mut self, cx: &mut Context<Self>) {
        let activity: Vec<CloudActivity> = self
            .clients
            .iter()
            .filter_map(|client| {
                let client = client.read(cx);
                let Transport::Cloud { link, .. } = client.transport() else {
                    return None;
                };
                let store = client.projects().read(cx);
                let threads = store
                    .threads()
                    .iter()
                    .filter(|thread| store.root_thread(thread.id) == thread.id)
                    .filter(|thread| {
                        store
                            .thread_status(thread.id)
                            .is_some_and(|status| status != ThreadStatus::Completed)
                    })
                    .map(|thread| thread.title.clone());
                let agents = client
                    .spaces()
                    .spaces
                    .iter()
                    .flat_map(|space| &space.tabs)
                    .flat_map(|tab| &tab.panes)
                    .filter(|pane| !matches!(pane.content, PaneContent::Thread(_)))
                    .filter_map(|pane| pane.agent.as_ref())
                    .filter(|agent| {
                        matches!(
                            agent.state,
                            PaneAgentState::Working | PaneAgentState::Blocked
                        )
                    })
                    .map(|agent| agent.name.clone());
                Some(CloudActivity {
                    provider: link.provider,
                    machine: link.machine.clone(),
                    working: threads.chain(agents).collect(),
                    open: self.open_machines.contains(&client.machine()),
                })
            })
            .collect();
        self.clients[0].update(cx, |local, cx| local.report_cloud_activity(activity, cx));
    }

    /// Links each SSH machine to the provider's machine it reaches, by the host and port `ssh`
    /// dials, so it moves into the provider's group (design/cloud topic 10 B).
    fn link_cloud_machines(&mut self, cx: &mut Context<Self>) {
        let reachable: Vec<(SshAddress, CloudLink)> = self.clients[0]
            .read(cx)
            .cloud()
            .providers
            .iter()
            .flat_map(|state| {
                state.machines.iter().filter_map(|machine| {
                    let link = CloudLink {
                        provider: state.provider,
                        machine: machine.id.clone(),
                    };
                    Some((machine.ssh_address.clone()?, link))
                })
            })
            .collect();
        if reachable.is_empty() {
            return;
        }
        let settings = AppSettingsStore::global(cx).read(cx).settings();
        let mut links: Vec<(u64, CloudLink)> = Vec::new();
        let mut lookups = Vec::new();
        for profile in settings
            .machines
            .iter()
            .filter(|profile| profile.cloud.is_none() && !profile.target.is_empty())
        {
            match self.ssh_addresses.get(&profile.target) {
                None => lookups.push(profile.target.clone()),
                // Being looked up, or `ssh` couldn't say.
                Some(None) => {}
                Some(Some(address)) => {
                    let reached = reachable.iter().find(|(reached, _)| {
                        reached.port == address.port
                            && reached.host.eq_ignore_ascii_case(&address.host)
                    });
                    if let Some((_, link)) = reached
                        && settings
                            .cloud_machine(link.provider, &link.machine)
                            .is_none()
                        && !links.iter().any(|(_, linked)| linked == link)
                    {
                        links.push((profile.id, link.clone()));
                    }
                }
            }
        }
        for target in lookups {
            self.look_up_address(target, cx);
        }
        if links.is_empty() {
            return;
        }
        AppSettingsStore::global(cx).update(cx, |store, cx| {
            store.update(
                |settings| {
                    for (id, link) in links {
                        if let Some(profile) = settings
                            .machines
                            .iter_mut()
                            .find(|profile| profile.id == id)
                        {
                            profile.cloud = Some(link);
                        }
                    }
                },
                cx,
            )
        });
    }

    fn look_up_address(&mut self, target: String, cx: &mut Context<Self>) {
        self.ssh_addresses.insert(target.clone(), None);
        let looked_up = reqwest_client::runtime().handle().spawn({
            let target = target.clone();
            async move { Ssh::new(&target)?.address().await }
        });
        cx.spawn(async move |this, cx| {
            let address = match looked_up.await {
                Ok(Ok(address)) => Some(address),
                Ok(Err(error)) => {
                    log::warn!("couldn't find where `ssh {target}` dials: {error:#}");
                    None
                }
                Err(error) => {
                    log::warn!("looking up where `ssh {target}` dials ended: {error}");
                    None
                }
            };
            this.update(cx, |this, cx| {
                this.ssh_addresses.insert(target, address);
                this.link_cloud_machines(cx);
            })
            .ok();
        })
        .detach();
    }

    /// What each server is told of the app's state, as it changes and as it connects.
    fn sync_servers(&mut self, cx: &mut Context<Self>) {
        self.sync_peers(cx);
        self.sync_design_defaults(cx);
        self.sync_auto_archive(cx);
        self.sync_subthread_limit(cx);
        self.sync_subthread_roles(cx);
        self.sync_acp_v2(cx);
        self.sync_cloud_activity(cx);
        self.sync_shared_settings(cx);
    }

    /// Gives each machine that follows Shared its thread titles, cleanup rules, skills and MCP
    /// servers, once its session has arrived. A machine saved before Shared is decided the
    /// first time it's seen: values of its own are kept as its own, and the first machine's
    /// start Shared.
    fn sync_shared_settings(&mut self, cx: &mut Context<Self>) {
        let store = AppSettingsStore::global(cx);
        let before = store.read(cx).settings().shared.clone();
        let mut shared = before.clone();
        let mut loaded = Vec::new();
        for client in self.clients.clone() {
            let machine = client.read(cx).machine();
            if !client.read(cx).is_session_loaded() {
                self.shared_list_attempts.remove(&machine);
                continue;
            }
            loaded.push(client.clone());

            let titles = client.read(cx).title_generation().settings.clone();
            match shared.decision(machine, SharedSection::ThreadTitles) {
                None => {
                    let own = match &shared.title_generation {
                        None => {
                            shared.title_generation = Some(titles);
                            false
                        }
                        Some(start) => *start != titles,
                    };
                    shared
                        .sharing_mut(machine)
                        .own
                        .insert(SharedSection::ThreadTitles, own);
                }
                Some(false) => {
                    if let Some(titles) = &shared.title_generation {
                        client.update(cx, |client, cx| client.follow_shared_titles(titles, cx));
                    }
                }
                Some(true) => {}
            }

            let rules = client.read(cx).storage().cleanup.rules.clone();
            match shared.decision(machine, SharedSection::CleanupRules) {
                None => {
                    let own = match &shared.cleanup_rules {
                        None => {
                            shared.cleanup_rules = Some(rules);
                            false
                        }
                        Some(start) => *start != rules,
                    };
                    shared
                        .sharing_mut(machine)
                        .own
                        .insert(SharedSection::CleanupRules, own);
                }
                Some(false) => {
                    if let Some(rules) = &shared.cleanup_rules {
                        client.update(cx, |client, cx| client.follow_shared_cleanup(rules, cx));
                    }
                }
                Some(true) => {}
            }
        }
        if shared != before {
            // Storing it runs this again, which gives the machines just decided theirs.
            store.update(cx, |store, cx| {
                store.update(|settings| settings.shared = shared, cx)
            });
            return;
        }
        for client in loaded {
            self.sync_shared_lists(&client, &shared, cx);
        }
    }

    /// Sends a machine the requests that give it Shared's skills and MCP servers, one at a
    /// time, noting each that's done. Steps that failed aren't tried again until something
    /// changes, or the machine connects again.
    fn sync_shared_lists(
        &mut self,
        client: &Entity<ServerClient>,
        shared: &SharedMachineSettings,
        cx: &mut Context<Self>,
    ) {
        let machine = client.read(cx).machine();
        if self.shared_list_syncs.contains_key(&machine) {
            return;
        }
        let steps = list_steps(
            shared,
            machine,
            client.read(cx).skills(),
            client.read(cx).mcp_servers(),
        );
        if steps.is_empty() {
            self.shared_list_attempts.remove(&machine);
            return;
        }
        // Until the machine's lists arrive, what was just done reads as still to do.
        if self.shared_list_attempts.get(&machine) == Some(&steps) {
            return;
        }
        self.shared_list_attempts.insert(machine, steps.clone());
        let store = AppSettingsStore::global(cx);
        let folder = store.read(cx).shared_skills_folder().clone();
        let client = client.clone();
        let task = cx.spawn(async move |this, cx| {
            for step in steps {
                let request = match &step {
                    ListStep::AddSkill(skill) => {
                        let skill_folder = shared_settings::skill_folder(&folder, &skill.name);
                        let files = cx
                            .background_spawn(async move {
                                shared_settings::read_skill_folder(&skill_folder)
                            })
                            .await;
                        match files {
                            Ok(files) => Some(Request::AddSkill(files)),
                            Err(error) => {
                                log::error!("reading the shared skill {}: {error:#}", skill.name);
                                continue;
                            }
                        }
                    }
                    ListStep::DeleteSkill(name) => Some(Request::DeleteSkill(name.clone())),
                    ListStep::SaveMcpServer { replacing, server } => Some(Request::SaveMcpServer {
                        replacing: replacing.clone(),
                        server: server.clone(),
                    }),
                    ListStep::DeleteMcpServer(name) => Some(Request::DeleteMcpServer(name.clone())),
                    ListStep::ForgetSkill(_)
                    | ListStep::KeepMcpServer(_)
                    | ListStep::ForgetMcpServer(_) => None,
                };
                if let Some(request) = request {
                    let response = client.read_with(cx, |client, _| client.request(request));
                    if let Err(error) = response.await {
                        log::error!("giving a machine Shared's {step:?} failed: {error:#}");
                        continue;
                    }
                }
                store.update(cx, |store, cx| {
                    store.update(
                        |settings| settings.shared.sharing_mut(machine).record(&step),
                        cx,
                    )
                });
            }
            this.update(cx, |this, cx| {
                this.shared_list_syncs.remove(&machine);
                this.sync_servers(cx);
            })
            .ok();
        });
        self.shared_list_syncs.insert(machine, task);
    }

    /// Gives each server the ACP v2 setting, which it keeps.
    fn sync_acp_v2(&mut self, cx: &mut Context<Self>) {
        let acp_v2 = AppSettingsStore::global(cx).read(cx).settings().acp_v2;
        for client in self.clients.clone() {
            client.update(cx, |client, cx| client.set_acp_v2(acp_v2, cx));
        }
    }

    /// Gives each server the number of subthreads a thread runs at once, which it keeps.
    fn sync_subthread_limit(&mut self, cx: &mut Context<Self>) {
        let limit = AppSettingsStore::global(cx)
            .read(cx)
            .settings()
            .subthread_limit;
        for client in self.clients.clone() {
            client.update(cx, |client, cx| client.set_subthread_limit(limit, cx));
        }
    }

    /// Gives each server what each role of delegated task runs on, which it keeps. The
    /// accounts chosen for a pick are This Mac's, so other machines use all of theirs.
    fn sync_subthread_roles(&mut self, cx: &mut Context<Self>) {
        let roles = AppSettingsStore::global(cx)
            .read(cx)
            .settings()
            .subthread_roles
            .clone();
        let mut every_account = roles.clone();
        for role in TaskRole::ALL {
            for pick in every_account.picks_mut(role) {
                pick.accounts = None;
            }
        }
        for client in self.clients.clone() {
            let roles = if client.read(cx).machine() == MachineId::Local {
                &roles
            } else {
                &every_account
            };
            client.update(cx, |client, cx| client.set_subthread_roles(roles, cx));
        }
    }

    /// Gives each server the setting for archiving threads left alone, which it keeps.
    fn sync_auto_archive(&mut self, cx: &mut Context<Self>) {
        let days = AppSettingsStore::global(cx)
            .read(cx)
            .settings()
            .auto_archive
            .days();
        for client in self.clients.clone() {
            client.update(cx, |client, cx| client.set_auto_archive(days, cx));
        }
    }

    /// Gives each server Settings › Artifacts' design defaults, and those of its projects whose combined
    /// project has its own (`design/artifacts-2`, topic 21 B).
    fn sync_design_defaults(&mut self, cx: &mut Context<Self>) {
        let settings = AppSettingsStore::global(cx).read(cx).settings();
        let general = settings.design_defaults;
        let own: Vec<(ProjectGroup, DesignDefaults)> =
            if settings.project_design_defaults.is_empty() {
                Vec::new()
            } else {
                let own = settings.project_design_defaults.clone();
                self.project_groups(cx)
                    .into_iter()
                    .filter_map(|group| {
                        let defaults = *own.get(&group.key)?;
                        Some((group, defaults))
                    })
                    .collect()
            };
        for client in self.clients.clone() {
            let machine = client.read(cx).machine();
            let projects = own
                .iter()
                .flat_map(|(group, defaults)| {
                    group
                        .members
                        .iter()
                        .filter(|(member_machine, _)| *member_machine == machine)
                        .map(|(_, project)| (project.id, *defaults))
                })
                .collect();
            client.update(cx, |client, cx| {
                client.set_design_settings(DesignSettings { general, projects }, cx)
            });
        }
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
            .machine_named(&call.machine, cx)
            .and_then(|machine| self.client(machine, cx));
        let response = match target {
            // A chat's calls only read, which the target's server holds them to.
            Some(target) if call.from_chat => target.read(cx).request(Request::CallTool {
                caller: ToolCaller::RelayedChat,
                name: call.name,
                arguments: call.arguments,
            }),
            // Only to checkouts the server was told of, whatever it asks.
            Some(target)
                if source
                    .read(cx)
                    .may_relay_to(&call.machine, call.path.as_deref()) =>
            {
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

    /// A pages server on one machine needs another's artifact answered: it asks here, and the
    /// answer goes back as `ArtifactRelayed`, as tool-call relays do (`design/artifacts`).
    fn relay_artifact(
        &mut self,
        source: Entity<ServerClient>,
        relay_id: u64,
        machine: String,
        request: ArtifactRequest,
        cx: &mut Context<Self>,
    ) {
        let reply = self
            .clients
            .iter()
            .find(|client| client.read(cx).label().as_ref() == machine)
            .map(|client| client.read(cx).request(Request::Artifact(request)));
        let response = match reply {
            Some(response) => response,
            None => {
                source.read(cx).send(
                    Request::ArtifactRelayed {
                        relay_id,
                        result: Err(format!("agentZ knows no machine called {machine}")),
                    },
                    cx,
                );
                return;
            }
        };
        cx.spawn(async move |_, cx| {
            let result = match response.await {
                Ok(Response::Artifact(reply)) => Ok(reply),
                Ok(response) => Err(format!("unexpected response: {response:?}")),
                Err(error) => Err(format!("{error:#}")),
            };
            cx.update(|cx| {
                source
                    .read(cx)
                    .send(Request::ArtifactRelayed { relay_id, result }, cx)
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
                    this.sync_servers(cx);
                    cx.notify()
                }),
                cx.observe(&projects, |this, _, cx| {
                    this.sync_servers(cx);
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
                    ServerClientEvent::RelayArtifact {
                        relay_id,
                        machine,
                        request,
                    } => {
                        this.relay_artifact(client, *relay_id, machine.clone(), request.clone(), cx)
                    }
                    ServerClientEvent::ShowThread {
                        machine: named,
                        thread_id,
                    } => {
                        let machine = match named {
                            Some(name) => this.machine_named(name, cx),
                            None => Some(machine),
                        };
                        if let Some(machine) = machine {
                            cx.emit(MachinesEvent::ShowThread(ThreadKey {
                                machine,
                                thread: *thread_id,
                            }))
                        }
                    }
                    ServerClientEvent::ArtifactPublished(artifact) => {
                        cx.emit(MachinesEvent::ArtifactPublished(machine, artifact.clone()))
                    }
                    ServerClientEvent::CloudChanged => this.cloud_changed(cx),
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
                    ProjectStoreEvent::Snoozing(thread) => {
                        cx.emit(MachinesEvent::Snoozing(ThreadKey {
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
            settings.show_repository_owner,
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

    /// The name the project shows under everywhere: its combined project's when it has copies
    /// on other machines, as the project picker names it, else its own.
    pub fn project_label(&self, machine: MachineId, project: &Project, cx: &App) -> SharedString {
        if project.repository.is_none() {
            return project.name();
        }
        let settings = AppSettingsStore::global(cx).read(cx).settings();
        let projects = self
            .clients
            .iter()
            .flat_map(|client| {
                let client = client.read(cx);
                let machine = client.machine();
                client
                    .projects()
                    .read(cx)
                    .projects()
                    .iter()
                    .filter(|project| project.repository.is_some())
                    .map(move |project| (machine, project.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        build_project_groups(
            projects,
            settings.project_grouping,
            &settings.project_grouping_overrides,
            settings.show_repository_owner,
        )
        .into_iter()
        .find(|group| group.contains(machine, project.id))
        .map_or_else(|| project.name(), |group| group.name())
    }

    /// The copy whose icon stands for the project, when that isn't the project itself: the
    /// first copy of its combined project (This Mac's), as the projects list and Settings show
    /// it. An image picked on this Mac isn't in the other machines' copies, so a thread there
    /// would show a letter tile. `None` for a project that's alone or is that first copy.
    pub fn shared_icon_project(
        &self,
        machine: MachineId,
        project: &Project,
        cx: &App,
    ) -> Option<(MachineId, Project)> {
        let repository = project.repository.as_ref()?;
        let settings = AppSettingsStore::global(cx).read(cx).settings();
        let logical = |machine: MachineId, project: &Project| {
            let physical = GroupKey::of_project(ProjectKey {
                machine,
                project: project.id,
            });
            let mode = settings
                .project_grouping_overrides
                .get(&physical)
                .copied()
                .unwrap_or(settings.project_grouping);
            logical_key(project, mode)
        };
        let key = logical(machine, project)?;
        for client in &self.clients {
            let client = client.read(cx);
            let other_machine = client.machine();
            for other in client.projects().read(cx).projects() {
                let is_copy = other
                    .repository
                    .as_ref()
                    .is_some_and(|other| other.canonical_key == repository.canonical_key)
                    && logical(other_machine, other).as_ref() == Some(&key);
                if !is_copy {
                    continue;
                }
                if other_machine == machine && other.id == project.id {
                    return None;
                }
                return Some((other_machine, other.clone()));
            }
        }
        None
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
        self.groups_in(&self.scope(cx), cx)
    }

    fn groups_in(&self, scope: &Scope, cx: &App) -> Vec<ProjectGroup> {
        let groups = self.project_groups(cx);
        match scope {
            Scope::All => groups,
            Scope::Chats => Vec::new(),
            Scope::Group(key) => groups
                .into_iter()
                .filter(|group| &group.key == key)
                .collect(),
        }
    }

    /// The scope, or all projects when its project is gone, or chats are while they're off. A
    /// project combined or split off since keeps its scope, by the group it's in now.
    pub fn scope(&self, cx: &App) -> Scope {
        let key = match &AppSettingsStore::global(cx).read(cx).settings().scope {
            Scope::All => return Scope::All,
            Scope::Chats if Self::chats_enabled(cx) => return Scope::Chats,
            Scope::Chats => return Scope::All,
            Scope::Group(key) => key,
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

    /// The app's theme for every machine's pages server.
    pub fn push_page_theme(&self, theme: PageTheme, cx: &App) {
        for client in &self.clients {
            client.read(cx).set_page_theme(theme.clone(), cx);
        }
    }

    /// Every machine's artifacts, newest publish first. Machines that aren't connected
    /// contribute nothing.
    pub fn artifacts(&self, cx: &App) -> Vec<(MachineId, Artifact)> {
        let mut artifacts: Vec<(MachineId, Artifact)> = self
            .clients
            .iter()
            .filter(|client| client.read(cx).is_online())
            .flat_map(|client| {
                let machine = client.read(cx).machine();
                client
                    .read(cx)
                    .artifacts()
                    .artifacts
                    .iter()
                    .map(move |artifact| (machine, artifact.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        artifacts.sort_by_key(|(_, artifact)| std::cmp::Reverse(artifact.published_at()));
        artifacts
    }

    /// The artifacts the Artifacts button counts and lists for the picked scope: the picked
    /// project's threads', chats' under Chats, or every one, chats' too, under All projects
    /// (`design/artifacts-2`, topic 1).
    pub fn scoped_artifacts(&self, cx: &App) -> Vec<(MachineId, Artifact)> {
        let artifacts = self.artifacts(cx);
        let key = match self.scope(cx) {
            Scope::All => return artifacts,
            Scope::Chats => {
                return artifacts
                    .into_iter()
                    .filter(|(_, artifact)| artifact.is_from_chat())
                    .collect();
            }
            Scope::Group(key) => key,
        };
        let Some(group) = self.group(&key, cx) else {
            return Vec::new();
        };
        artifacts
            .into_iter()
            .filter(|(machine, artifact)| {
                artifact
                    .project_id
                    .is_some_and(|project| group.contains(*machine, project))
            })
            .collect()
    }

    /// What one thread published, for its header's pill.
    pub fn artifacts_for_thread(&self, key: &ThreadKey, cx: &App) -> Vec<Artifact> {
        self.client(key.machine, cx)
            .map(|client| {
                client
                    .read(cx)
                    .artifacts()
                    .artifacts
                    .iter()
                    .filter(|artifact| artifact.thread_id == Some(key.thread))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The artifact's page URL, served by this machine's pages server: remote machines'
    /// artifacts are relayed through it (`design/artifacts`).
    pub fn artifact_link(
        &self,
        machine: MachineId,
        artifact: &ArtifactId,
        // An earlier version, or the latest with `None`, whose page follows new publishes.
        version: Option<u32>,
        cx: &App,
    ) -> Option<String> {
        let local = self.clients.first()?.read(cx);
        if !local.is_online() {
            return None;
        }
        let pages = local.artifacts().pages.as_ref()?;
        let machine_name = match machine {
            MachineId::Local => None,
            _ => Some(self.label(machine, cx)),
        };
        Some(pages.link(machine_name.as_deref(), artifact, version))
    }

    /// The artifacts page, served by this machine's pages server, open at a project's place
    /// when given one.
    pub fn artifacts_home_link(&self, project: Option<&str>, cx: &App) -> Option<String> {
        let local = self.clients.first()?.read(cx);
        if !local.is_online() {
            return None;
        }
        Some(local.artifacts().pages.as_ref()?.home_link(project))
    }

    /// Settings › Threads' Chats: while it's off, chats are hidden and kept.
    pub fn chats_enabled(cx: &App) -> bool {
        AppSettingsStore::global(cx).read(cx).settings().chats
    }

    /// Every machine's chats the Chats entry counts: those among the threads.
    pub fn chat_count(&self, cx: &App) -> usize {
        self.threads_where(cx, |_, thread| {
            thread.is_chat()
                && thread.archived_at.is_none()
                && thread.task.is_none()
                && !thread.is_draft
        })
        .len()
    }

    /// The most pressing status among every machine's chats.
    pub fn chats_status(&self, cx: &App) -> Option<ThreadStatus> {
        self.clients
            .iter()
            .filter_map(|client| {
                client
                    .read(cx)
                    .projects()
                    .read(cx)
                    .project_status(ProjectId::CHATS)
            })
            .max()
    }

    /// Unarchived top-level threads of the visible projects: the pinned ones in the order the
    /// user gave them, then the rest, as the user arranged them below the new ones or latest
    /// active first. Shells aren't threads until an agent CLI runs in them, nor drafts until
    /// their first message.
    pub fn active_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let scope = self.scope(cx);
        let groups = self.groups_in(&scope, cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.archived_at.is_none()
                && thread.task.is_none()
                && !thread.is_draft
                && !thread.in_workspaces()
                && !self.is_shell(machine, thread, cx)
                && self.is_thread_visible(&scope, &groups, machine, thread, cx)
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

    /// The active threads that are snoozed, the soonest to wake first, for the sidebar's
    /// Snoozed shelf (t3code's).
    pub fn snoozed_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let now = SystemTime::now();
        let mut threads: Vec<(MachineId, Thread)> = self
            .active_threads(cx)
            .into_iter()
            .filter(|(machine, thread)| self.is_snoozed(*machine, thread.id, now, cx))
            .collect();
        threads.sort_by_key(|(_, thread)| thread.snoozed_until);
        threads
    }

    /// [`projects::ProjectStore::is_thread_snoozed`] on the thread's machine.
    pub fn is_snoozed(
        &self,
        machine: MachineId,
        thread: ThreadId,
        now: SystemTime,
        cx: &App,
    ) -> bool {
        self.projects(machine, cx)
            .is_some_and(|store| store.read(cx).is_thread_snoozed(thread, now))
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
        let scope = self.scope(cx);
        let groups = self.groups_in(&scope, cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.is_draft
                && thread.unsent_text.is_some()
                && thread.archived_at.is_none()
                && thread.task.is_none()
                && !thread.in_workspaces()
                && self.is_thread_visible(&scope, &groups, machine, thread, cx)
        });
        threads.sort_by_key(|(_, thread)| std::cmp::Reverse((thread.created_at, thread.id)));
        threads
    }

    /// Threads started in a workspace pane, latest active first, for the sidebar's Workspaces
    /// section: under a project, those whose folder is in it.
    pub fn workspaces_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let scope = self.scope(cx);
        let groups = self.groups_in(&scope, cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.in_workspaces()
                && thread.archived_at.is_none()
                && thread.task.is_none()
                && !thread.is_draft
                && self.is_thread_visible(&scope, &groups, machine, thread, cx)
        });
        threads.sort_by_key(|(_, thread)| {
            std::cmp::Reverse(thread.last_activity_at.or(thread.created_at))
        });
        threads
    }

    /// Unarchived terminal threads of the visible projects running no agent CLI, latest active
    /// first, for the sidebar's Shells shelf.
    pub fn shell_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        let scope = self.scope(cx);
        let groups = self.groups_in(&scope, cx);
        let mut threads = self.threads_where(cx, |machine, thread| {
            thread.archived_at.is_none()
                && thread.task.is_none()
                && !thread.in_workspaces()
                && self.is_shell(machine, thread, cx)
                && self.is_thread_visible(&scope, &groups, machine, thread, cx)
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
    /// projects, and a chat with all projects or under Chats.
    fn is_thread_visible(
        &self,
        scope: &Scope,
        groups: &[ProjectGroup],
        machine: MachineId,
        thread: &Thread,
        cx: &App,
    ) -> bool {
        if thread.is_chat() {
            return Self::shows_chats(scope, cx);
        }
        match self.thread_project(machine, thread, cx) {
            Some(project_id) => groups
                .iter()
                .any(|group| group.contains(machine, project_id)),
            None => *scope == Scope::All,
        }
    }

    fn shows_chats(scope: &Scope, cx: &App) -> bool {
        Self::chats_enabled(cx) && matches!(scope, Scope::All | Scope::Chats)
    }

    /// A terminal thread with no agent CLI running in it.
    pub fn is_shell(&self, machine: MachineId, thread: &Thread, cx: &App) -> bool {
        thread.terminal.is_some()
            && self
                .projects(machine, cx)
                .is_none_or(|store| store.read(cx).terminal_agent(thread.id).is_none())
    }

    /// Archived top-level threads of the visible projects, and chats while the scope shows them,
    /// most recently archived first.
    pub fn archived_threads(&self, cx: &App) -> Vec<(MachineId, Thread)> {
        self.archived_page(usize::MAX, cx).1
    }

    /// How many threads are archived, and the `limit` most recently archived of them. The list
    /// draws a page of them, and only while it's open, so the rest aren't cloned on every
    /// frame.
    pub fn archived_page(&self, limit: usize, cx: &App) -> (usize, Vec<(MachineId, Thread)>) {
        let scope = self.scope(cx);
        let groups = self.groups_in(&scope, cx);
        let shows_chats = Self::shows_chats(&scope, cx);
        let mut threads: Vec<(MachineId, &Thread)> = Vec::new();
        for client in &self.clients {
            let client = client.read(cx);
            let machine = client.machine();
            threads.extend(
                client
                    .projects()
                    .read(cx)
                    .threads()
                    .iter()
                    .filter(|thread| {
                        thread.archived_at.is_some()
                            && thread.task.is_none()
                            && if thread.is_chat() {
                                shows_chats
                            } else {
                                groups
                                    .iter()
                                    .any(|group| group.contains(machine, thread.project_id))
                            }
                    })
                    .map(|thread| (machine, thread)),
            );
        }
        threads.sort_by_key(|(_, thread)| std::cmp::Reverse(thread.archived_at));
        let count = threads.len();
        let page = threads
            .into_iter()
            .take(limit)
            .map(|(machine, thread)| (machine, thread.clone()))
            .collect();
        (count, page)
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
    show_repository_owner: bool,
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
            group.label = group_label(&group.members, show_repository_owner);
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
/// repository's, else `owner/repo` (unless the owner is hidden), else the repository's name,
/// else the first member's. Names are compared ignoring case, so an `agentZ` folder of the
/// `agentz` repository is still just the repository's name.
fn group_label(members: &[(MachineId, Project)], show_repository_owner: bool) -> SharedString {
    fn same(a: &str, b: &str) -> bool {
        a.to_lowercase() == b.to_lowercase()
    }
    fn unique(values: impl Iterator<Item = Option<String>>) -> Vec<String> {
        let mut unique: Vec<String> = Vec::new();
        for value in values.flatten() {
            let value = value.trim().to_string();
            if !value.is_empty() && !unique.iter().any(|other| same(other, &value)) {
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
    let is_repository_name = |name: &str| {
        display_names
            .iter()
            .chain(&repository_names)
            .any(|other| same(other, name))
    };
    match (
        names.as_slice(),
        display_names.as_slice(),
        repository_names.as_slice(),
    ) {
        ([name], _, _) if !is_repository_name(name) => name.clone().into(),
        (_, [display_name], _) if show_repository_owner => display_name.clone().into(),
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

    /// A machine that uses shared settings is given Shared's thread titles, cleanup rules,
    /// skills and MCP servers once its session arrives. The first machine seen starts Shared,
    /// and one saved before Shared whose values differ keeps them as its own.
    #[gpui::test]
    fn machines_that_use_shared_settings_are_given_them(cx: &mut gpui::TestAppContext) {
        use std::cell::RefCell;
        use std::rc::Rc;

        use agentz_protocol::mcp_servers::{McpServer, McpTransport};
        use agentz_protocol::spaces::SpacesSnapshot;
        use agentz_protocol::storage::{CleanupRules, Storage};
        use agentz_protocol::title_generation::{TitleGeneration, TitleGenerationState};
        use agentz_protocol::{Request, Response};

        use crate::app_settings::AppSettingsStore;
        use crate::server_client::ServerClient;
        use crate::shared_settings::{self, MachineSharing, SharedSection};

        let titles = TitleGeneration {
            enabled: true,
            ..TitleGeneration::default()
        };
        let own_rules = CleanupRules {
            merged: true,
            ..CleanupRules::default()
        };
        let github = McpServer {
            name: "github".into(),
            enabled: true,
            transport: McpTransport::Local {
                command: "npx".into(),
                args: Vec::new(),
                env: Vec::new(),
            },
            kept_off: Vec::new(),
        };
        let requests: Rc<RefCell<Vec<(MachineId, Request)>>> = Rc::default();
        let devbox_2 = MachineId::Remote(2);
        let clients = cx.update(|cx| {
            crate::init_for_test(cx);
            let clients: Vec<_> = [
                (MachineId::Local, "This Mac"),
                (MachineId::Remote(1), "Devbox 1"),
                (devbox_2, "Devbox 2"),
            ]
            .into_iter()
            .map(|(machine, label)| {
                let client = ServerClient::new_for_test(
                    machine,
                    label.into(),
                    SpacesSnapshot::default(),
                    cx,
                );
                let requests = requests.clone();
                client.update(cx, |client, _| {
                    client.answer_for_test(move |request| {
                        requests.borrow_mut().push((machine, request.clone()));
                        matches!(
                            request,
                            Request::AddSkill(_) | Request::SaveMcpServer { .. }
                        )
                        .then_some(Response::Ok)
                    })
                });
                client
            })
            .collect();
            clients[0].update(cx, |client, cx| {
                client.set_title_generation_for_test(
                    TitleGenerationState {
                        settings: titles.clone(),
                        providers: Vec::new(),
                    },
                    cx,
                )
            });
            // Devbox 1 was saved before Shared, with rules of its own.
            clients[1].update(cx, |client, cx| {
                let mut storage = Storage::default();
                storage.cleanup.rules = own_rules.clone();
                client.set_storage_for_test(storage, cx)
            });
            let store = AppSettingsStore::global(cx);
            let folder = store.read(cx).shared_skills_folder().clone();
            let review =
                shared_settings::create_skill(&folder, "review", "Reviews a change.", "Read it.")
                    .expect("the shared skill is written");
            store.update(cx, |store, cx| {
                store.update(
                    |settings| {
                        settings.shared.put_skill(review);
                        settings.shared.mcp_servers.push(github.clone());
                        // Devbox 2 was added since.
                        *settings.shared.sharing_mut(devbox_2) = MachineSharing::following();
                    },
                    cx,
                )
            });
            super::init_for_test(clients.clone(), cx);
            clients
        });
        let shared = |cx: &mut gpui::TestAppContext| {
            cx.update(|cx| {
                AppSettingsStore::global(cx)
                    .read(cx)
                    .settings()
                    .shared
                    .clone()
            })
        };

        // Nothing is decided before a machine's session arrives.
        cx.run_until_parked();
        assert_eq!(shared(cx).title_generation, None);
        assert!(requests.borrow().is_empty());
        for client in &clients {
            client.update(cx, |client, cx| client.set_session_loaded_for_test(cx));
            cx.run_until_parked();
        }

        let shared = shared(cx);
        assert_eq!(shared.title_generation, Some(titles.clone()));
        assert_eq!(shared.cleanup_rules, Some(CleanupRules::default()));
        assert!(!shared.sets_own(MachineId::Local, SharedSection::ThreadTitles));
        for section in [SharedSection::ThreadTitles, SharedSection::CleanupRules] {
            assert!(shared.sets_own(MachineId::Remote(1), section));
            assert!(!shared.sets_own(devbox_2, section));
        }
        let sent = |client: &gpui::Entity<ServerClient>, cx: &mut gpui::TestAppContext| {
            client.read_with(cx, |client, _| {
                client
                    .sent_for_test()
                    .into_iter()
                    .filter(|request| {
                        matches!(
                            request,
                            Request::SetTitleGeneration(_) | Request::SetCleanupRules(_)
                        )
                    })
                    .collect::<Vec<_>>()
            })
        };
        assert_eq!(sent(&clients[2], cx), [Request::SetTitleGeneration(titles)]);
        assert!(sent(&clients[1], cx).is_empty());
        assert_eq!(
            clients[1].read_with(cx, |client, _| client.storage().cleanup.rules.clone()),
            own_rules
        );

        // Every machine follows Shared's lists unless it says otherwise, so each is given
        // the skill and the server once.
        for client in &clients {
            let machine = client.read_with(cx, |client, _| client.machine());
            let given: Vec<String> = requests
                .borrow()
                .iter()
                .filter(|(to, _)| *to == machine)
                .filter_map(|(_, request)| match request {
                    Request::AddSkill(files) => Some(format!("skill {}", files.len())),
                    Request::SaveMcpServer { replacing, server } => {
                        Some(format!("server {} {replacing:?}", server.name))
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(given, ["skill 1", "server github None"], "{machine:?}");
            let sharing = shared.sharing(machine).cloned().unwrap_or_default();
            assert!(sharing.skills_given.contains_key("review"));
            assert!(sharing.mcp_servers_given.contains("github"));
        }
        let folder = cx.update(|cx| {
            AppSettingsStore::global(cx)
                .read(cx)
                .shared_skills_folder()
                .clone()
        });
        std::fs::remove_dir_all(folder).ok();
    }
    /// This Mac's server hears what's working on each provider machine and whether anything of
    /// it is open, each time that changes. Each of its cloud notices is shown once and then
    /// dismissed, and a notification's Add Credit opens the provider's billing page.
    #[gpui::test]
    fn tells_this_macs_server_of_cloud_machines_and_shows_its_notices(
        cx: &mut gpui::TestAppContext,
    ) {
        use agentz_protocol::Request;
        use agentz_protocol::cloud::{
            CloudActivity, CloudNotice, CloudSnapshot, CloudState, NoticeKind, Provider,
        };
        use agentz_protocol::layout::{Node, PaneId};
        use agentz_protocol::spaces::{
            Pane, PaneAgent, PaneAgentState, PaneContent, PaneTerminal, Space, SpaceId,
            SpacesSnapshot, Tab, TabId,
        };
        use collections::HashSet;

        use crate::app_settings::CloudLink;
        use crate::cloud_machines::{NoticeAction, act_on_notification};
        use crate::server_client::{ServerClient, Transport};

        let spaces = |state: PaneAgentState| {
            let mut pane = Pane::new(
                PaneId(1),
                PaneContent::Terminal(PaneTerminal {
                    folder: "/root".into(),
                    command: None,
                }),
            );
            pane.agent = Some(PaneAgent {
                name: "Claude Code".into(),
                state,
                registry_agent: None,
            });
            SpacesSnapshot {
                spaces: vec![Space {
                    id: SpaceId(1),
                    name: None,
                    folder: "/root".into(),
                    project_id: None,
                    tabs: vec![Tab {
                        id: TabId(1),
                        name: None,
                        root: Node::Pane(PaneId(1)),
                        panes: vec![pane],
                    }],
                    git: None,
                    current: None,
                }],
            }
        };
        let (mac, devbox) = cx.update(|cx| {
            crate::init_for_test(cx);
            cx.set_app_identity("dev.agentz.test", "agentZ");
            let mac = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            mac.update(cx, |mac, cx| mac.set_online_for_test(cx));
            let devbox = ServerClient::new_for_test(
                MachineId::Remote(1),
                "devbox5".into(),
                spaces(PaneAgentState::Working),
                cx,
            );
            devbox.update(cx, |devbox, _| {
                devbox.set_transport_for_test(Transport::Cloud {
                    link: CloudLink {
                        provider: Provider::Proton,
                        machine: "c-1".into(),
                    },
                    target: None,
                })
            });
            super::init_for_test(vec![mac.clone(), devbox.clone()], cx);
            (mac, devbox)
        });
        let sent = |cx: &mut gpui::TestAppContext| mac.read_with(cx, |mac, _| mac.sent_for_test());
        let reports = |cx: &mut gpui::TestAppContext| {
            sent(cx)
                .into_iter()
                .filter_map(|request| match request {
                    Request::ReportCloudActivity(activity) => Some(activity),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        let activity = |working: &[&str], open: bool| {
            vec![CloudActivity {
                provider: Provider::Proton,
                machine: "c-1".into(),
                working: working.iter().map(|name| name.to_string()).collect(),
                open,
            }]
        };
        assert_eq!(reports(cx), [activity(&["Claude Code"], false)]);

        let open = |cx: &mut gpui::TestAppContext| {
            cx.update(|cx| {
                super::Machines::global(cx).update(cx, |machines, cx| {
                    machines.set_open_machines(HashSet::from_iter([MachineId::Remote(1)]), cx)
                })
            })
        };
        open(cx);
        open(cx);
        devbox.update(cx, |devbox, cx| {
            devbox.set_spaces_for_test(spaces(PaneAgentState::Idle), cx)
        });
        cx.run_until_parked();
        assert_eq!(
            reports(cx),
            [
                activity(&["Claude Code"], false),
                activity(&["Claude Code"], true),
                activity(&[], true),
            ]
        );

        let notice = |id: u64| CloudNotice {
            id,
            provider: Provider::Proton,
            kind: NoticeKind::Stopped {
                machine: "c-1".into(),
                name: "devbox5".into(),
                state: CloudState::Stopped,
                working: vec![format!("Thread {id}")],
            },
            at: std::time::SystemTime::UNIX_EPOCH,
        };
        let notices = |ids: &[u64], cx: &mut gpui::TestAppContext| {
            let cloud = CloudSnapshot {
                notices: ids.iter().map(|id| notice(*id)).collect(),
                last_notice: ids.last().copied().unwrap_or_default(),
                ..Default::default()
            };
            mac.update(cx, |mac, cx| mac.set_cloud_for_test(cloud, cx));
            cx.run_until_parked();
        };
        let shown = |cx: &mut gpui::TestAppContext| {
            cx.shown_system_notifications()
                .into_iter()
                .map(|shown| shown.body.to_string())
                .collect::<Vec<_>>()
        };
        let dismissed = |cx: &mut gpui::TestAppContext| {
            sent(cx)
                .into_iter()
                .filter_map(|request| match request {
                    Request::DismissCloudNotices(up_to) => Some(up_to),
                    _ => None,
                })
                .collect::<Vec<_>>()
        };
        notices(&[1], cx);
        assert_eq!(shown(cx), ["Working on it: Thread 1"]);
        assert_eq!(dismissed(cx), [1]);
        // Sent again before the server drops it, it isn't shown twice.
        notices(&[1, 2], cx);
        assert_eq!(
            shown(cx),
            ["Working on it: Thread 1", "Working on it: Thread 2"]
        );
        assert_eq!(dismissed(cx), [1, 2]);

        cx.update(|cx| act_on_notification(Provider::Proton, NoticeAction::AddCredit, cx));
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://app.cube.computer/billing")
        );
    }

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
            let groups = build_project_groups(clones(), mode, &BTreeMap::new(), true);
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
        let groups = build_project_groups(
            named,
            ProjectGroupingMode::Repository,
            &BTreeMap::new(),
            true,
        );
        assert_eq!(groups[0].label.as_ref(), "Custom project");

        let repository_named = vec![
            (MachineId::Local, project(1, "/work/a/t3code", None)),
            (MachineId::Remote(1), project(1, "/srv/t3code", None)),
        ];
        let groups = build_project_groups(
            repository_named,
            ProjectGroupingMode::Repository,
            &BTreeMap::new(),
            true,
        );
        assert_eq!(groups[0].label.as_ref(), "T3 Code");

        let alone = build_project_groups(
            vec![(MachineId::Local, project(1, "/work/t3code", None))],
            ProjectGroupingMode::Repository,
            &BTreeMap::new(),
            true,
        );
        assert_eq!(alone[0].label.as_ref(), "t3code");
    }

    #[test]
    fn labels_ignore_case_and_can_hide_the_owner() {
        // Folders named `agentZ` and `AGENTZ`, of a remote written `Owner/agentz`.
        let checkouts = || {
            [
                (MachineId::Local, "/work/agentZ"),
                (MachineId::Remote(1), "/srv/AGENTZ"),
            ]
            .into_iter()
            .map(|(machine, path)| {
                let mut project = project(1, path, None);
                if let Some(repository) = &mut project.repository {
                    repository.display_name = Some("Owner/agentz".into());
                    repository.name = Some("agentz".into());
                }
                (machine, project)
            })
            .collect::<Vec<_>>()
        };
        let groups = build_project_groups(
            checkouts(),
            ProjectGroupingMode::Repository,
            &BTreeMap::new(),
            true,
        );
        assert_eq!(groups[0].label.as_ref(), "Owner/agentz");

        let groups = build_project_groups(
            checkouts(),
            ProjectGroupingMode::Repository,
            &BTreeMap::new(),
            false,
        );
        assert_eq!(groups[0].label.as_ref(), "agentz");
    }

    #[test]
    fn keeps_clones_apart_when_asked() {
        let groups = build_project_groups(
            clones(),
            ProjectGroupingMode::Separate,
            &BTreeMap::new(),
            true,
        );
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
        let groups =
            build_project_groups(clones(), ProjectGroupingMode::Repository, &overrides, true);
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
            true,
        );
        assert_eq!(
            member_ids(&by_path),
            [
                vec![(MachineId::Local, 1), (MachineId::Remote(1), 1)],
                vec![(MachineId::Local, 2)]
            ]
        );
        let by_repository = build_project_groups(
            projects,
            ProjectGroupingMode::Repository,
            &BTreeMap::new(),
            true,
        );
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
            true,
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
