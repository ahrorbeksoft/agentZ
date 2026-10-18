//! Cloud providers (design/cloud): their CLIs, looked for and run on this Mac's server once an
//! app asks ([`Request::CheckCloud`]), then every minute while one is open. Only this Mac's
//! server is asked, since the CLIs are installed here; another machine's never runs them.
//! What they report is kept in `cloud.json`, so a machine that's off is still known after a
//! restart.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use agentz_protocol::Response;
use agentz_protocol::cloud::{
    Change, CloudMachine, CloudSnapshot, CloudUsage, CreateMachine, Lifecycle, MachineAction,
    MachineChange, Provider, ProviderState, ProviderStatus,
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
        if !self.cloud.watching {
            self.cloud.watching = true;
            self.start_cloud_refreshes();
        }
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
    /// again every few minutes.
    fn refresh_cloud(&mut self) {
        if !self
            .clients
            .values()
            .any(|client| client.subscribed_to_session)
        {
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
        });
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
        if let Err(error) = self.start_cloud_change(provider, &machine, Change::Lifecycle(change)) {
            return self.respond(client, id, Err(error));
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
            server.end_cloud_change(provider, &machine);
            server.respond(client, id, changed.map(|()| Response::Ok));
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
        let cli = self.cloud.cli(provider);
        let api = self.cloud.api.clone();
        let ready = self.shell_environment_ready.clone();
        let changes = !matches!(
            action,
            MachineAction::SshCommand | MachineAction::DesktopLink { .. }
        );
        let work = async move {
            ready.await;
            cli.act(&machine, &action, &api)
                .await
                .map_err(|error| anyhow!("{error:#}"))
        };
        self.spawn_then(work, move |server, output| {
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
