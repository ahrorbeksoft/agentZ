//! Cloud providers (design/cloud): their CLIs, looked for and run on this Mac's server once an
//! app asks ([`Request::CheckCloud`]), then every minute while one is open. Only this Mac's
//! server is asked, since the CLIs are installed here; another machine's never runs them.
//! What they report is kept in `cloud.json`, so a machine that's off is still known after a
//! restart.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use agentz_protocol::Response;
use agentz_protocol::cloud::{
    Change, CloudActivity, CloudMachine, CloudNotice, CloudSnapshot, CloudState, CloudUsage,
    CreateMachine, Lifecycle, MachineAction, MachineChange, NoticeKind, Provider, ProviderState,
    ProviderStatus, hours_of_balance,
};
use anyhow::{Context as _, Result, anyhow};
use cloud::{Cli, InstaboxApi};
use util::ResultExt as _;

use super::{ClientId, Input, Server};
use crate::agent_settings::{read_json, write_json};

/// How often a signed-in provider's machines are listed while an app is open (topic 10).
const LIST_INTERVAL: Duration = Duration::from_secs(60);
/// How often the CLIs are looked for again, for one installed or signed in since.
const STATUS_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// A machine seen stopped this long after the listing before isn't told of (topic 12 D): it
/// stopped while nothing was watching, maybe hours ago.
const NOTICE_GAP: Duration = Duration::from_secs(5 * 60);
/// How long after agentZ's own stop, pause, restart or delete is done the stop a listing shows
/// is taken as asked for.
const ASKED_FOR: Duration = Duration::from_secs(10 * 60);
/// A balance that lasts less than this at the running machines' rate is warned of (topic 12 C).
const LOW_BALANCE_HOURS: f64 = 24.0;
/// The notices kept until an app shows them; older ones are dropped.
const MAX_NOTICES: usize = 20;

/// A provider's machine, by its id.
type MachineKey = (Provider, String);

pub(super) struct CloudStore {
    snapshot: CloudSnapshot,
    path: Option<PathBuf>,
    revision: u64,
    /// The `PATH` the CLIs are looked for on; `None` is the server's own.
    cli_path: Option<OsString>,
    /// An app asked, so the providers are refreshed every minute while one is open.
    watching: bool,
    status_checked_at: Option<Instant>,
    /// Providers being looked at now, so a refresh doesn't run beside another.
    reading: BTreeSet<Provider>,
    /// For what Instabox's CLI doesn't do, with agentZ's key.
    api: Arc<InstaboxApi>,
    /// What the app last said of each machine's use.
    uses: BTreeMap<MachineKey, MachineUse>,
    /// The client that reports it; once it's gone, nothing is open.
    reporter: Option<ClientId>,
    /// Since when each machine has been listed running, so one started after a long stop isn't
    /// idle from before it.
    running_since: BTreeMap<MachineKey, Instant>,
    /// When agentZ last stopped, paused, restarted or deleted each machine, so the stop a
    /// listing then shows isn't told as unasked.
    asked: BTreeMap<MachineKey, Instant>,
    /// Machines whose idle pause is being turned off or set back now (topic 12 A).
    holding: BTreeSet<MachineKey>,
}

/// What the app last said of a machine's use (topic 12 B).
struct MachineUse {
    working: Vec<String>,
    open: bool,
    /// Since when nothing has worked on it and nothing of it has been open; `None` while
    /// something is.
    idle_since: Option<Instant>,
}

impl CloudStore {
    pub(super) fn load(
        path: Option<PathBuf>,
        cli_path: Option<OsString>,
        api: InstaboxApi,
    ) -> Self {
        let mut snapshot: CloudSnapshot = path
            .as_deref()
            .and_then(|path| read_json(path).log_err().flatten())
            .unwrap_or_default();
        // Nothing is being changed by a server that just started.
        snapshot.checking = false;
        for provider in &mut snapshot.providers {
            provider.changing.clear();
        }
        Self {
            snapshot,
            path,
            revision: 0,
            cli_path,
            watching: false,
            status_checked_at: None,
            reading: BTreeSet::new(),
            api: Arc::new(api),
            uses: BTreeMap::new(),
            reporter: None,
            running_since: BTreeMap::new(),
            asked: BTreeMap::new(),
            holding: BTreeSet::new(),
        }
    }

    pub(super) fn snapshot(&self) -> &CloudSnapshot {
        &self.snapshot
    }

    /// Bumped by every change.
    pub(super) fn revision(&self) -> u64 {
        self.revision
    }

    fn cli(&self, provider: Provider) -> Cli {
        Cli::on_path(provider, self.cli_path.clone())
    }

    fn update<T>(&mut self, change: impl FnOnce(&mut CloudSnapshot) -> T) -> T {
        let previous = self.snapshot.clone();
        let result = change(&mut self.snapshot);
        if self.snapshot != previous {
            self.revision += 1;
            let mut saved = self.snapshot.clone();
            saved.checking = false;
            if let Some(path) = &self.path {
                write_json(path, &saved).log_err();
            }
        }
        result
    }

    /// The provider's state, made when it's first looked for.
    fn provider(snapshot: &mut CloudSnapshot, provider: Provider) -> &mut ProviderState {
        if snapshot.provider(provider).is_none() {
            snapshot.providers.push(ProviderState {
                provider,
                status: ProviderStatus::NotInstalled,
                machines: Vec::new(),
                usage: None,
                listed_at: None,
                error: None,
                changing: Vec::new(),
                stop_when_idle: BTreeMap::new(),
                balance_low: false,
                held_awake: BTreeMap::new(),
            });
            snapshot
                .providers
                .sort_by_key(|state| Provider::ALL.iter().position(|p| *p == state.provider));
        }
        snapshot
            .providers
            .iter_mut()
            .find(|state| state.provider == provider)
            .expect("just added")
    }

    /// Keeps the app's report of what's working on each machine and what's open; a machine it
    /// leaves out has neither.
    fn keep_activity(&mut self, client: ClientId, activity: Vec<CloudActivity>, now: Instant) {
        self.reporter = Some(client);
        let mut reported: BTreeMap<MachineKey, (Vec<String>, bool)> = activity
            .into_iter()
            .map(|activity| {
                (
                    (activity.provider, activity.machine),
                    (activity.working, activity.open),
                )
            })
            .collect();
        let keys: BTreeSet<MachineKey> = self.uses.keys().chain(reported.keys()).cloned().collect();
        for key in keys {
            let (working, open) = reported.remove(&key).unwrap_or_default();
            let busy = open || !working.is_empty();
            let machine_use = self.uses.entry(key).or_insert(MachineUse {
                working: Vec::new(),
                open: false,
                idle_since: None,
            });
            machine_use.idle_since = if busy {
                None
            } else {
                machine_use.idle_since.or(Some(now))
            };
            machine_use.working = working;
            machine_use.open = open;
        }
    }

    /// The reporting app is gone, so nothing is open now; what was working may still be, so
    /// it's kept until an app says otherwise.
    fn reporter_gone(&mut self, client: ClientId, now: Instant) {
        if self.reporter != Some(client) {
            return;
        }
        self.reporter = None;
        for machine_use in self.uses.values_mut() {
            machine_use.open = false;
            if machine_use.working.is_empty() {
                machine_use.idle_since.get_or_insert(now);
            }
        }
    }

    /// How long the machine has run with nothing working on it and nothing of it open.
    fn idle_for(&self, key: &MachineKey, now: Instant) -> Option<Duration> {
        let running_since = *self.running_since.get(key)?;
        let since = match self.uses.get(key) {
            Some(MachineUse {
                idle_since: None, ..
            }) => return None,
            Some(MachineUse {
                idle_since: Some(since),
                ..
            }) => (*since).max(running_since),
            None => running_since,
        };
        Some(now.saturating_duration_since(since))
    }

    /// The provider's machines that stopped since the listing before without agentZ asking
    /// (topic 12 D), as they're listed now; since when each running one runs is kept.
    fn stops_seen(
        &mut self,
        provider: Provider,
        listed: &[CloudMachine],
        now: Instant,
    ) -> Vec<NoticeKind> {
        self.asked
            .retain(|_, at| now.saturating_duration_since(*at) < ASKED_FOR);
        self.running_since.retain(|(of, id), _| {
            *of != provider || listed.iter().any(|machine| machine.id == *id)
        });
        let previous = self.snapshot.provider(provider);
        let recent = previous
            .and_then(|state| state.listed_at)
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|gap| gap <= NOTICE_GAP);
        let mut stops = Vec::new();
        for machine in listed {
            let key = (provider, machine.id.clone());
            if machine.state.is_running() {
                self.running_since.entry(key).or_insert(now);
                continue;
            }
            self.running_since.remove(&key);
            let was_running = previous
                .and_then(|state| state.machines.iter().find(|before| before.id == machine.id))
                .is_some_and(|before| before.state.is_running());
            if !was_running || !is_stopped(&machine.state) {
                continue;
            }
            if self.asked.remove(&key).is_some() || !recent {
                continue;
            }
            stops.push(NoticeKind::Stopped {
                machine: machine.id.clone(),
                name: machine.name.clone(),
                state: machine.state.clone(),
                working: self
                    .uses
                    .get(&key)
                    .map(|machine_use| machine_use.working.clone())
                    .unwrap_or_default(),
            });
        }
        stops
    }

    /// The provider's machines that have been idle as long as their Stop when idle says
    /// (topic 12 B), and aren't being changed.
    fn idle_machines(&self, provider: Provider, now: Instant) -> Vec<String> {
        let Some(state) = self.snapshot.provider(provider) else {
            return Vec::new();
        };
        state
            .stop_when_idle
            .iter()
            .filter(|(machine, after)| {
                state
                    .machines
                    .iter()
                    .any(|listed| listed.id == **machine && listed.state.is_running())
                    && !state
                        .changing
                        .iter()
                        .any(|changing| changing.machine == **machine)
                    && self
                        .idle_for(&(provider, (*machine).clone()), now)
                        .is_some_and(|idle| idle >= **after)
            })
            .map(|(machine, _)| machine.clone())
            .collect()
    }

    /// The provider has a running machine it stops when idle, or one whose idle pause it holds
    /// off, so it's listed with no app open.
    fn needs_listing(&self, provider: Provider) -> bool {
        self.snapshot.provider(provider).is_some_and(|state| {
            !state.held_awake.is_empty()
                || state.stop_when_idle.keys().any(|machine| {
                    state
                        .machines
                        .iter()
                        .any(|listed| listed.id == *machine && listed.state.is_running())
                })
        })
    }

    /// What holding off the provider's idle pause needs now (topic 12 A): the running machines
    /// agents work on that aren't held yet, unless the account fixes the pause, and the held
    /// ones nothing works on any more, with the pause each gets back.
    fn awake_changes(&self, provider: Provider) -> (Vec<String>, Vec<(String, Duration)>) {
        let Some(state) = self.snapshot.provider(provider) else {
            return Default::default();
        };
        if !provider.description().capabilities.idle_pause {
            return Default::default();
        }
        let working = |machine: &str| {
            self.uses
                .get(&(provider, machine.to_string()))
                .is_some_and(|machine_use| !machine_use.working.is_empty())
        };
        let free = |machine: &str| !self.holding.contains(&(provider, machine.to_string()));
        let can_hold = matches!(
            &state.status,
            ProviderStatus::SignedIn { rules, .. } if rules.fixed_idle_pause.is_none()
        );
        let hold = state
            .machines
            .iter()
            .filter(|machine| {
                can_hold
                    && machine.state.is_running()
                    && working(&machine.id)
                    && free(&machine.id)
                    && !state.held_awake.contains_key(&machine.id)
            })
            .map(|machine| machine.id.clone())
            .collect();
        let release = state
            .held_awake
            .iter()
            .filter(|(machine, _)| free(machine) && !working(machine))
            .map(|(machine, pause)| (machine.clone(), *pause))
            .collect();
        (hold, release)
    }
}

/// Paused, stopped or suspended, or on its way there.
fn is_stopped(state: &CloudState) -> bool {
    matches!(
        state,
        CloudState::Pausing
            | CloudState::Paused
            | CloudState::Stopping
            | CloudState::Stopped
            | CloudState::Suspended
    )
}

/// The low-balance notice (topic 12 C), while the balance runs out within a day at the rate
/// the provider's running machines cost.
fn low_balance(state: &ProviderState) -> Option<NoticeKind> {
    let hourly = state.provider.description().hourly_micro_usd?;
    let Some(CloudUsage::Balance {
        balance_micro_usd: Some(balance_micro_usd),
        ..
    }) = state.usage
    else {
        return None;
    };
    let running: Vec<String> = state
        .machines
        .iter()
        .filter(|machine| machine.state.is_running())
        .map(|machine| machine.name.clone())
        .collect();
    let hours_left = hours_of_balance(balance_micro_usd, hourly * running.len() as u64);
    (hours_left < LOW_BALANCE_HOURS).then_some(NoticeKind::LowBalance {
        balance_micro_usd,
        hours_left,
        running,
    })
}

fn push_notice(snapshot: &mut CloudSnapshot, provider: Provider, kind: NoticeKind) {
    snapshot.last_notice += 1;
    snapshot.notices.push(CloudNotice {
        id: snapshot.last_notice,
        provider,
        kind,
        at: SystemTime::now(),
    });
    let excess = snapshot.notices.len().saturating_sub(MAX_NOTICES);
    snapshot.notices.drain(..excess);
}

/// What reading a provider found.
struct Read {
    provider: Provider,
    /// `None` when only the machines were listed.
    status: Option<ProviderStatus>,
    machines: Option<Result<Vec<CloudMachine>>>,
    usage: Option<Result<CloudUsage>>,
}

impl Server {
    /// Check Again, and an app's first ask: every provider looked for now, answered once done.
    pub(super) fn check_cloud(&mut self, client: ClientId, id: u64) {
        self.watch_cloud();
        self.cloud.update(|snapshot| snapshot.checking = true);
        let providers: Vec<Provider> = Provider::ALL
            .into_iter()
            .filter(|provider| self.cloud.reading.insert(*provider))
            .collect();
        let reads: Vec<_> = providers
            .into_iter()
            .map(|provider| self.read_provider(provider, true))
            .collect();
        self.cloud.status_checked_at = Some(Instant::now());
        self.spawn_then(futures::future::join_all(reads), move |server, reads| {
            for read in reads {
                server.keep_cloud_read(read);
            }
            server.cloud.update(|snapshot| snapshot.checking = false);
            server.respond(client, id, Ok(Response::Ok));
        });
    }

    /// A server that stops idle machines, or holds off their idle pause, watches them from the
    /// start, with no app open.
    pub(super) fn resume_cloud_watch(&mut self) {
        if self
            .cloud
            .snapshot()
            .providers
            .iter()
            .any(|state| !state.stop_when_idle.is_empty() || !state.held_awake.is_empty())
        {
            self.watch_cloud();
        }
    }

    fn watch_cloud(&mut self) {
        if !self.cloud.watching {
            self.cloud.watching = true;
            self.start_cloud_refreshes();
        }
    }

    fn start_cloud_refreshes(&mut self) {
        let inputs = self.inputs.clone();
        self.runtime.spawn(async move {
            loop {
                tokio::time::sleep(LIST_INTERVAL).await;
                let refresh = Input::Run(Box::new(|server: &mut Server| server.refresh_cloud()));
                if inputs.unbounded_send(refresh).is_err() {
                    break;
                }
            }
        });
    }

    /// Lists the signed-in providers' machines while an app is open, and looks for the CLIs
    /// again every few minutes. With no app open, only machines stopped when idle are listed,
    /// to stop them.
    fn refresh_cloud(&mut self) {
        if !self
            .clients
            .values()
            .any(|client| client.subscribed_to_session)
        {
            for provider in Provider::ALL {
                if self.cloud.needs_listing(provider) {
                    self.list_cloud_machines(provider);
                }
            }
            return;
        }
        let check_status = self
            .cloud
            .status_checked_at
            .is_none_or(|at| at.elapsed() >= STATUS_INTERVAL);
        if check_status {
            self.cloud.status_checked_at = Some(Instant::now());
        }
        for provider in Provider::ALL {
            let signed_in = self
                .cloud
                .snapshot()
                .provider(provider)
                .is_some_and(|state| state.status.is_signed_in());
            if (check_status || signed_in) && self.cloud.reading.insert(provider) {
                let read = self.read_provider(provider, check_status);
                self.spawn_then(read, |server, read| server.keep_cloud_read(read));
            }
        }
    }

    /// Lists the provider's machines again now, as after a change to one.
    fn list_cloud_machines(&mut self, provider: Provider) {
        if self.cloud.reading.insert(provider) {
            let read = self.read_provider(provider, false);
            self.spawn_then(read, |server, read| server.keep_cloud_read(read));
        }
    }

    /// Reads the provider once the login shell's environment is in: who its CLI is signed in
    /// as (with `status`), then its machines and usage while it's signed in.
    fn read_provider(
        &self,
        provider: Provider,
        status: bool,
    ) -> impl Future<Output = Read> + Send + 'static {
        let cli = self.cloud.cli(provider);
        let ready = self.shell_environment_ready.clone();
        let signed_in = self
            .cloud
            .snapshot()
            .provider(provider)
            .is_some_and(|state| state.status.is_signed_in());
        async move {
            ready.await;
            let status = if status {
                Some(cli.status().await)
            } else {
                None
            };
            let signed_in = status
                .as_ref()
                .map_or(signed_in, ProviderStatus::is_signed_in);
            let (machines, usage) = if signed_in {
                let (machines, usage) = futures::join!(cli.machines(), cli.usage());
                (Some(machines), Some(usage))
            } else {
                (None, None)
            };
            Read {
                provider,
                status,
                machines,
                usage,
            }
        }
    }

    fn keep_cloud_read(&mut self, read: Read) {
        self.cloud.reading.remove(&read.provider);
        let Read {
            provider,
            status,
            machines,
            usage,
        } = read;
        if let Some(Err(error)) = &usage {
            log::warn!("reading {provider:?}'s usage: {error:#}");
        }
        let now = Instant::now();
        let stops = match &machines {
            Some(Ok(listed)) => self.cloud.stops_seen(provider, listed, now),
            _ => Vec::new(),
        };
        self.cloud.update(|snapshot| {
            let state = CloudStore::provider(snapshot, provider);
            if let Some(status) = status {
                state.status = status;
            }
            match machines {
                Some(Ok(machines)) => {
                    state.machines = machines;
                    state.listed_at = Some(SystemTime::now());
                    state.error = None;
                }
                Some(Err(error)) => state.error = Some(format!("{error:#}")),
                None => {}
            }
            if let Some(Ok(usage)) = usage {
                state.usage = Some(usage);
            }
            let mut notices = stops;
            match low_balance(state) {
                Some(notice) if !state.balance_low => {
                    state.balance_low = true;
                    notices.push(notice);
                }
                Some(_) => {}
                None => state.balance_low = false,
            }
            for notice in notices {
                push_notice(snapshot, provider, notice);
            }
        });
        self.stop_idle_machines(provider);
        self.keep_cloud_machines_awake();
    }

    /// Stops the provider's machines left idle as long as their Stop when idle says.
    fn stop_idle_machines(&mut self, provider: Provider) {
        for machine in self.cloud.idle_machines(provider, Instant::now()) {
            log::info!("stopping {provider:?}'s {machine}, idle");
            self.run_cloud_change(provider, machine, Lifecycle::Stop, |_, stopped| {
                stopped.context("stopping an idle machine").log_err();
            });
        }
    }

    /// The app's report of what's working on and open of each provider machine.
    pub(super) fn report_cloud_activity(&mut self, client: ClientId, activity: Vec<CloudActivity>) {
        self.cloud.keep_activity(client, activity, Instant::now());
        for provider in Provider::ALL {
            self.stop_idle_machines(provider);
        }
        self.keep_cloud_machines_awake();
    }

    /// Holds off the idle pause of machines agents work on, as the server holds off sleep
    /// (topic 12 A): it's turned off, and set back once nothing works on them. An account
    /// whose pause is fixed can't turn it off, and its machines' pages say so.
    fn keep_cloud_machines_awake(&mut self) {
        for provider in Provider::ALL {
            let (hold, release) = self.cloud.awake_changes(provider);
            for machine in hold {
                self.hold_awake(provider, machine);
            }
            for (machine, pause) in release {
                self.release_awake(provider, machine, pause);
            }
        }
    }

    /// Turns the machine's idle pause off, keeping the one it had.
    fn hold_awake(&mut self, provider: Provider, machine: String) {
        let key = (provider, machine.clone());
        self.cloud.holding.insert(key.clone());
        let cli = self.cloud.cli(provider);
        let api = self.cloud.api.clone();
        let ready = self.shell_environment_ready.clone();
        let work = {
            let machine = machine.clone();
            async move {
                ready.await;
                let pause = cli.idle_pause(&machine).await?;
                if pause.is_some() {
                    let off = MachineAction::SetIdlePause(Duration::ZERO);
                    cli.act(&machine, &off, &api).await?;
                }
                anyhow::Ok(pause.unwrap_or_default())
            }
        };
        self.spawn_then(work, move |server, held| {
            server.cloud.holding.remove(&key);
            match held {
                Ok(pause) => {
                    server.cloud.update(|snapshot| {
                        CloudStore::provider(snapshot, provider)
                            .held_awake
                            .insert(machine, pause);
                    });
                    // The work may have ended meanwhile.
                    server.keep_cloud_machines_awake();
                }
                Err(error) => log::warn!("keeping {provider:?}'s {machine} awake: {error:#}"),
            }
        });
    }

    /// Sets the idle pause back on a machine nothing works on any more; one that had none, or
    /// that's gone, is only forgotten.
    fn release_awake(&mut self, provider: Provider, machine: String, pause: Duration) {
        let listed = self
            .cloud
            .snapshot()
            .provider(provider)
            .is_some_and(|state| state.machines.iter().any(|listed| listed.id == machine));
        let forget = move |server: &mut Server, machine: &str| {
            server.cloud.update(|snapshot| {
                CloudStore::provider(snapshot, provider)
                    .held_awake
                    .remove(machine);
            });
        };
        if pause.is_zero() || !listed {
            return forget(self, &machine);
        }
        let key = (provider, machine.clone());
        self.cloud.holding.insert(key.clone());
        let cli = self.cloud.cli(provider);
        let api = self.cloud.api.clone();
        let ready = self.shell_environment_ready.clone();
        let work = {
            let machine = machine.clone();
            async move {
                ready.await;
                cli.act(&machine, &MachineAction::SetIdlePause(pause), &api)
                    .await
            }
        };
        self.spawn_then(work, move |server, set| {
            server.cloud.holding.remove(&key);
            match set {
                Ok(_) => {
                    forget(server, &machine);
                    // Work may have started again meanwhile.
                    server.keep_cloud_machines_awake();
                }
                Err(error) => {
                    log::warn!("setting {provider:?}'s {machine}'s idle pause back: {error:#}")
                }
            }
        });
    }

    pub(super) fn dismiss_cloud_notices(&mut self, up_to: u64) {
        self.cloud
            .update(|snapshot| snapshot.notices.retain(|notice| notice.id > up_to));
    }

    pub(super) fn cloud_client_gone(&mut self, client: ClientId) {
        self.cloud.reporter_gone(client, Instant::now());
    }

    /// Sets how long the machine may be idle before agentZ stops it (topic 12 B).
    fn set_stop_when_idle(
        &mut self,
        provider: Provider,
        machine: &str,
        after: Option<Duration>,
    ) -> Result<()> {
        let description = provider.description();
        anyhow::ensure!(
            description.capabilities.idle_stop,
            "agentZ doesn't stop {} machines when idle",
            description.name
        );
        self.cloud.update(|snapshot| {
            let state = CloudStore::provider(snapshot, provider);
            match after {
                Some(after) => state.stop_when_idle.insert(machine.to_string(), after),
                None => state.stop_when_idle.remove(machine),
            };
        });
        if after.is_some() {
            self.watch_cloud();
            self.stop_idle_machines(provider);
        }
        Ok(())
    }

    /// Starts, stops, pauses or deletes a machine, answered once its CLI is done; the machines
    /// are listed again after.
    pub(super) fn change_cloud_machine(
        &mut self,
        client: ClientId,
        id: u64,
        provider: Provider,
        machine: String,
        change: Lifecycle,
    ) {
        self.run_cloud_change(provider, machine, change, move |server, changed| {
            server.respond(client, id, changed.map(|()| Response::Ok));
        });
    }

    /// Runs the change with its CLI, then `then` once it's done.
    fn run_cloud_change(
        &mut self,
        provider: Provider,
        machine: String,
        change: Lifecycle,
        then: impl FnOnce(&mut Server, Result<()>) + Send + 'static,
    ) {
        if let Err(error) = self.start_cloud_change(provider, &machine, Change::Lifecycle(change)) {
            return then(self, Err(error));
        }
        let stops = matches!(
            change,
            Lifecycle::Stop | Lifecycle::Pause | Lifecycle::Restart | Lifecycle::Delete
        );
        if stops {
            self.cloud
                .asked
                .insert((provider, machine.clone()), Instant::now());
        }
        let cli = self.cloud.cli(provider);
        let ready = self.shell_environment_ready.clone();
        let work = {
            let machine = machine.clone();
            async move {
                ready.await;
                cli.change(&machine, change).await
            }
        };
        self.spawn_then(work, move |server, changed| {
            if stops {
                server
                    .cloud
                    .asked
                    .insert((provider, machine.clone()), Instant::now());
            }
            server.end_cloud_change(provider, &machine);
            then(server, changed);
        });
    }

    /// What the provider's new machines can start from.
    pub(super) fn cloud_sources(&mut self, client: ClientId, id: u64, provider: Provider) {
        let cli = self.cloud.cli(provider);
        let ready = self.shell_environment_ready.clone();
        let work = async move {
            ready.await;
            cli.sources().await.map_err(|error| anyhow!("{error:#}"))
        };
        self.spawn_then(work, move |server, sources| {
            server.respond(client, id, sources.map(Response::CloudSources));
        });
    }

    /// Makes a machine, answered with it once it's made; the machines are listed again after.
    pub(super) fn create_cloud_machine(
        &mut self,
        client: ClientId,
        id: u64,
        provider: Provider,
        machine: CreateMachine,
    ) {
        if let Err(error) = self.start_cloud_change(provider, &machine.name, Change::Create) {
            return self.respond(client, id, Err(error));
        }
        let cli = self.cloud.cli(provider);
        let ready = self.shell_environment_ready.clone();
        let name = machine.name.clone();
        let work = async move {
            ready.await;
            cli.create(&machine).await
        };
        self.spawn_then(work, move |server, created| {
            if let Ok(machine) = &created {
                // Listed now, so the app finds it before the listing that follows.
                server.cloud.update(|snapshot| {
                    let state = CloudStore::provider(snapshot, provider);
                    state.machines.retain(|listed| listed.id != machine.id);
                    state.machines.push(machine.clone());
                });
            }
            server.end_cloud_change(provider, &name);
            server.respond(client, id, created.map(Response::CloudMachineCreated));
        });
    }

    /// Marks the change as under way, refusing a second change to the same machine: the CLIs
    /// wait on one operation at a time.
    fn start_cloud_change(
        &mut self,
        provider: Provider,
        machine: &str,
        change: Change,
    ) -> Result<()> {
        let description = provider.description();
        let state = self
            .cloud
            .snapshot()
            .provider(provider)
            .with_context(|| format!("{} isn't set up here", description.name))?;
        anyhow::ensure!(
            state.status.is_signed_in(),
            "{}'s CLI isn't signed in: run `{}` in a terminal",
            description.name,
            description.sign_in
        );
        if let Change::Lifecycle(lifecycle) = change {
            anyhow::ensure!(
                description.lifecycle.contains(&lifecycle),
                "{} machines can't do that",
                description.name
            );
        }
        anyhow::ensure!(
            !state
                .changing
                .iter()
                .any(|changing| changing.machine == machine),
            "{machine} is already changing"
        );
        self.cloud.update(|snapshot| {
            CloudStore::provider(snapshot, provider)
                .changing
                .push(MachineChange {
                    machine: machine.to_string(),
                    change,
                });
        });
        Ok(())
    }

    fn end_cloud_change(&mut self, provider: Provider, machine: &str) {
        self.cloud.update(|snapshot| {
            CloudStore::provider(snapshot, provider)
                .changing
                .retain(|changing| changing.machine != machine);
        });
        self.list_cloud_machines(provider);
    }

    /// What a machine's page shows beyond its listing (topic 7 A).
    pub(super) fn cloud_machine_details(
        &mut self,
        client: ClientId,
        id: u64,
        provider: Provider,
        machine: String,
    ) {
        let cli = self.cloud.cli(provider);
        let api = self.cloud.api.clone();
        let ready = self.shell_environment_ready.clone();
        let work = async move {
            ready.await;
            cli.details(&machine, &api)
                .await
                .map_err(|error| anyhow!("{error:#}"))
        };
        self.spawn_then(work, move |server, details| {
            server.respond(client, id, details.map(Response::CloudMachineDetails));
        });
    }

    /// Does a page's action to a machine, answered once its CLI is done with what it gave for
    /// the user. The machines and usage are listed again after one that changes something.
    pub(super) fn cloud_machine_action(
        &mut self,
        client: ClientId,
        id: u64,
        provider: Provider,
        machine: String,
        action: MachineAction,
    ) {
        if let MachineAction::StopWhenIdle(after) = action {
            let set = self.set_stop_when_idle(provider, &machine, after);
            return self.respond(client, id, set.map(|()| Response::Ok));
        }
        let cli = self.cloud.cli(provider);
        let api = self.cloud.api.clone();
        let ready = self.shell_environment_ready.clone();
        let changes = !matches!(
            action,
            MachineAction::SshCommand | MachineAction::DesktopLink { .. }
        );
        let sets_idle_pause = matches!(action, MachineAction::SetIdlePause(_));
        let work = {
            let machine = machine.clone();
            async move {
                ready.await;
                cli.act(&machine, &action, &api)
                    .await
                    .map_err(|error| anyhow!("{error:#}"))
            }
        };
        self.spawn_then(work, move |server, output| {
            if sets_idle_pause && output.is_ok() {
                // The user's pause is the one to set back now, and it's turned off again while
                // agents still work.
                server.cloud.update(|snapshot| {
                    CloudStore::provider(snapshot, provider)
                        .held_awake
                        .remove(&machine);
                });
                server.keep_cloud_machines_awake();
            }
            if changes {
                server.list_cloud_machines(provider);
            }
            let response = output.map(|output| output.map_or(Response::Ok, Response::CloudOutput));
            server.respond(client, id, response);
        });
    }

    /// How SSH reaches the machine, from its CLI.
    pub(super) fn cloud_ssh_options(
        &mut self,
        client: ClientId,
        id: u64,
        provider: Provider,
        machine: String,
    ) {
        let cli = self.cloud.cli(provider);
        let ready = self.shell_environment_ready.clone();
        let work = async move {
            ready.await;
            cli.ssh_options(&machine)
                .await
                .map_err(|error| anyhow!("{error:#}"))
        };
        self.spawn_then(work, move |server, options| {
            server.respond(client, id, options.map(Response::SshOptions));
        });
    }
}
