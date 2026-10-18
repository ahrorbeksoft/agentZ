//! Cloud providers (design/cloud): machines a provider's CLI makes, starts and stops, which
//! agentZ then reaches over SSH like any other machine. The user installs each CLI and signs in
//! with it; this Mac's server runs it, and reports what it found in these types.

use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Instabox,
    Proton,
}

impl Provider {
    /// In the order Settings › Machines lists them.
    pub const ALL: [Provider; 2] = [Provider::Instabox, Provider::Proton];

    pub fn description(self) -> &'static ProviderDescription {
        match self {
            Provider::Instabox => &INSTABOX,
            Provider::Proton => &PROTON,
        }
    }
}

/// What agentZ knows about a provider before finding its CLI.
#[derive(Debug, PartialEq)]
pub struct ProviderDescription {
    pub name: &'static str,
    /// What it calls its machines ("computer"), in "New Proton Computer".
    pub noun: &'static str,
    /// The CLI's program, looked up on the login shell's `PATH`.
    pub program: &'static str,
    /// The command that installs the CLI, shown to copy.
    pub install: &'static str,
    /// What signs the CLI in, shown to run in a terminal.
    pub sign_in: &'static str,
    pub learn_more: &'static str,
    /// Where the account is managed.
    pub dashboard: &'static str,
    /// Where credit is added, for a provider that bills a balance.
    pub add_credit: Option<&'static str>,
    /// One line on what it is.
    pub pitch: &'static str,
    pub price: &'static str,
    /// What a new machine is, where every one is the same.
    pub new_machine: Option<&'static str>,
    /// agentZ's promo, invite or referral code, once the provider offers one.
    pub code: Option<ProviderCode>,
    /// What a running machine costs, for a provider whose machines bill from when they're
    /// created or started; starting one is asked about first (topic 5).
    pub charge: Option<&'static str>,
    /// The same, an hour, in millionths of a dollar, for what a run has cost so far.
    pub hourly_micro_usd: Option<u64>,
    /// The lifecycle changes it has.
    pub lifecycle: &'static [Lifecycle],
    pub capabilities: Capabilities,
}

#[derive(Debug, PartialEq)]
pub struct ProviderCode {
    /// What it's for, as shown beside it ("Invite code 482913").
    pub text: &'static str,
    pub code: &'static str,
}

/// The provider's features beyond machines and their lifecycle; the UI shows only these.
#[derive(Debug, Default, PartialEq)]
pub struct Capabilities {
    pub templates: bool,
    pub snapshots: bool,
    pub public_ports: bool,
    pub vault: bool,
    pub ssh_tokens: bool,
    pub desktop: bool,
    pub metrics: bool,
    pub timeouts: bool,
    /// Its machines' SSH keys can be replaced.
    pub ssh_keys: bool,
}

static INSTABOX: ProviderDescription = ProviderDescription {
    name: "Instabox",
    noun: "machine",
    program: "instabox",
    install: "curl -fsSL https://api.instabox.sh/install.sh | sh",
    sign_in: "instabox login",
    learn_more: "https://instabox.sh",
    dashboard: "https://instabox.sh",
    add_credit: None,
    pitch: "Linux machines for agents: pause and resume, snapshots, public ports, a desktop.",
    price: "Free during the beta",
    // Its templates say.
    new_machine: None,
    code: None,
    charge: None,
    hourly_micro_usd: None,
    lifecycle: &[
        Lifecycle::Start,
        Lifecycle::Stop,
        Lifecycle::Pause,
        Lifecycle::Resume,
        Lifecycle::Delete,
    ],
    capabilities: Capabilities {
        templates: true,
        snapshots: true,
        public_ports: true,
        vault: true,
        ssh_tokens: true,
        desktop: true,
        metrics: true,
        timeouts: true,
        ssh_keys: false,
    },
};

static PROTON: ProviderDescription = ProviderDescription {
    name: "Proton",
    noun: "computer",
    program: "proton",
    install: "curl -fsSL https://app.proton.computer/install.sh | sh",
    sign_in: "proton login",
    learn_more: "https://proton.computer",
    dashboard: "https://app.proton.computer",
    add_credit: Some("https://app.cube.computer/billing"),
    pitch: "One persistent Linux computer, with Claude Code, Codex and Docker installed.",
    price: "$20 a month while running",
    new_machine: Some("8 shared vCPUs · 2–64 GB memory · 50 GB disk · Ubuntu 24.04"),
    code: None,
    charge: Some("$20 per 730 running hours, billed by the second from your Cube balance"),
    // $20 / 730 hours.
    hourly_micro_usd: Some(27_397),
    lifecycle: &[
        Lifecycle::Start,
        Lifecycle::Stop,
        Lifecycle::Restart,
        Lifecycle::Delete,
    ],
    capabilities: Capabilities {
        templates: false,
        snapshots: false,
        public_ports: false,
        vault: false,
        ssh_tokens: false,
        desktop: false,
        metrics: false,
        timeouts: false,
        ssh_keys: true,
    },
};

/// Whether the provider's CLI is there and signed in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ProviderStatus {
    NotInstalled,
    SignedOut {
        version: String,
    },
    SignedIn {
        version: String,
        /// Who `whoami` says is signed in.
        account: String,
        #[serde(default)]
        rules: AccountRules,
    },
}

/// What the provider lets the account change about its machines.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AccountRules {
    /// Its machines pause after this long idle, whatever they're asked.
    pub fixed_idle_pause: Option<Duration>,
    /// The longest timeout a machine can have.
    pub max_timeout: Option<Duration>,
}

impl ProviderStatus {
    pub fn is_signed_in(&self) -> bool {
        matches!(self, ProviderStatus::SignedIn { .. })
    }
}

/// A provider's machine, as its CLI lists it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloudMachine {
    /// The provider's id for it, which its CLI takes wherever it takes a name.
    pub id: String,
    pub name: String,
    pub state: CloudState,
    pub size: MachineSize,
    /// The template it started from, where the provider has templates.
    pub template: Option<String>,
    /// Where SSH reaches it, when the listing says: what links an SSH machine already saved.
    pub ssh_address: Option<SshAddress>,
    pub created_at: Option<SystemTime>,
    /// Why it failed, when it did.
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MachineSize {
    pub vcpus: Option<u32>,
    /// Its memory, or the memory it's guaranteed where it grows (Proton's).
    pub memory_mib: Option<u64>,
    /// What its memory can grow to.
    pub memory_ceiling_mib: Option<u64>,
    pub disk_gib: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SshAddress {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudState {
    Creating,
    Starting,
    Running,
    Stopping,
    Stopped,
    Pausing,
    Paused,
    Resuming,
    Deleting,
    /// Proton's, when the balance ran out.
    Suspended,
    Failed,
    /// A state agentZ doesn't know, as the provider named it.
    Other(String),
}

impl CloudState {
    /// Running, so it can be connected to.
    pub fn is_running(&self) -> bool {
        matches!(self, CloudState::Running)
    }
}

/// A change to a machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lifecycle {
    Start,
    Stop,
    Restart,
    Pause,
    Resume,
    Delete,
}

/// What a provider's machines can start from, for a new one.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CloudSources {
    pub templates: Vec<CloudTemplate>,
    pub snapshots: Vec<MachineSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloudTemplate {
    pub id: String,
    /// What it has ("dev plus Docker, Python (uv), Go and Rust").
    pub description: Option<String>,
    pub size: MachineSize,
}

/// A saved machine (Instabox's snapshot), which new machines start from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MachineSnapshot {
    pub id: String,
    pub name: String,
    /// Saved and uploaded, so a machine can start from it.
    pub is_ready: bool,
    /// The machine it was saved from, by its id.
    pub source_machine: Option<String>,
    pub created_at: Option<SystemTime>,
    /// It has the machine's software only, without its home and workspace.
    #[serde(default)]
    pub software_only: bool,
    /// The saved disks' full size, not the storage they use.
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// What a machine's page shows (topic 7 A) beyond its listing, as its provider tells it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MachineDetails {
    /// The template it started from, with its version ("universal 2026-10-13").
    pub template: Option<String>,
    /// When it's paused or deleted (Instabox's timeout).
    pub timeout: Option<TimeoutAt>,
    /// It pauses once it's been idle this long; `None` never does.
    pub idle_pause: Option<Duration>,
    /// When it paused itself for being idle, until it's next changed.
    pub idle_paused_at: Option<SystemTime>,
    /// Its labels (Instabox's metadata).
    pub labels: Vec<(String, String)>,
    /// What it gets from the account's Vault, where the provider has one.
    pub vault: Option<MachineVault>,
    /// The account's snapshots saved from it, newest first.
    pub snapshots: Vec<MachineSnapshot>,
    pub ports: Vec<PublishedPort>,
    /// CPU and memory, oldest first: Instabox samples them every 30 s, and gives the last 5
    /// minutes.
    pub metrics: Vec<MetricSample>,
    /// How long it ran this month.
    pub running: Option<RunningTime>,
    /// When it last started, while it runs (Proton's).
    pub started_at: Option<SystemTime>,
    pub last_activity: Option<SystemTime>,
    /// What couldn't be read ("Its ports: …"); the rest still is.
    pub errors: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimeoutAt {
    pub at: SystemTime,
    /// It's deleted then, rather than paused.
    pub delete: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MachineVault {
    /// It takes the Vault; turned off, it is for good.
    pub enabled: bool,
    /// The items it gets with its labels.
    pub items: Vec<VaultItem>,
    /// The last write into it.
    pub last_write: Option<VaultWrite>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultItem {
    pub name: String,
    /// Where a file is written under its home; `None` for an environment variable.
    pub path: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VaultWrite {
    pub at: Option<SystemTime>,
    /// Why it failed, when it did.
    pub error: Option<String>,
}

/// A port with a public HTTPS URL (Instabox's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PublishedPort {
    pub port: u16,
    pub name: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricSample {
    pub at: SystemTime,
    /// CPU time used over its vCPUs, 0 to 100.
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_total_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunningTime {
    /// Since when it's counted.
    pub since: SystemTime,
    pub seconds: u64,
    pub vcpu_seconds: Option<u64>,
}

/// What a machine's page does with its provider's extras.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MachineAction {
    /// Saves a snapshot of it; `software_only` leaves out its home and workspace.
    SaveSnapshot {
        name: String,
        software_only: bool,
    },
    /// Deletes one of the account's snapshots, by its id.
    DeleteSnapshot(String),
    PublishPort {
        port: u16,
        name: Option<String>,
    },
    UnpublishPort(u16),
    /// Writes the Vault's items into it now.
    PushVault,
    /// Pauses or deletes it this long from now; `None` clears its timeout.
    SetTimeout(Option<MachineTimeout>),
    /// Pauses it once it's been idle this long; zero never does.
    SetIdlePause(Duration),
    /// A plain `ssh` command for any terminal, which works for 7 days, answered with
    /// [`crate::Response::CloudOutput`].
    SshCommand,
    /// Stops every SSH command made for it, and closes their sessions.
    RevokeSshCommands,
    /// A link to its desktop, installed and started first, answered with
    /// [`crate::Response::CloudOutput`]: one to use it, or to watch it for an hour.
    DesktopLink {
        view_only: bool,
    },
    /// Replaces the SSH keys that reach it with these public key files on this Mac (Proton's).
    ReplaceSshKeys(Vec<std::path::PathBuf>),
}

/// What a new machine is made with.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateMachine {
    pub name: String,
    /// A template, or one of the account's snapshots; the provider's default without either.
    pub source: Option<MachineSource>,
    /// Pause it, or delete it, this long after it's made.
    pub timeout: Option<MachineTimeout>,
    /// Pause it once it's been idle this long; zero never does.
    pub auto_suspend: Option<Duration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MachineSource {
    Template(String),
    Snapshot(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MachineTimeout {
    pub after: Duration,
    pub delete: bool,
}

/// How a provider's machine is reached: its CLI's `ssh-config` Host block.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SshOptions {
    pub host_name: String,
    pub port: Option<u16>,
    pub user: Option<String>,
    /// The block's other options, in its order, given to `ssh` as `-o Key=Value`.
    pub options: Vec<(String, String)>,
}

/// What an account has used, as its CLI reports it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CloudUsage {
    /// Instabox's counts against its limits.
    Limits {
        machines: u32,
        max_machines: Option<u32>,
        snapshots: u32,
        max_snapshots: Option<u32>,
        stored_bytes: Option<u64>,
        max_stored_gib: Option<u64>,
    },
    /// Proton's balance, shared with Cube, and what its computers ran.
    Balance {
        /// `None` with no prepaid balance.
        balance_micro_usd: Option<i64>,
        running_hours: f64,
        charged_micro_usd: i64,
    },
    /// Billing isn't on for the account.
    NotBilled,
}

/// What this Mac's server knows of the providers, kept in its `cloud.json` and sent to apps.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CloudSnapshot {
    /// One per provider, in [`Provider::ALL`]'s order, once looked for.
    pub providers: Vec<ProviderState>,
    /// Looking for the CLIs now (Check Again).
    #[serde(default)]
    pub checking: bool,
}

impl CloudSnapshot {
    pub fn provider(&self, provider: Provider) -> Option<&ProviderState> {
        self.providers
            .iter()
            .find(|state| state.provider == provider)
    }

    pub fn machine(&self, provider: Provider, machine: &str) -> Option<&CloudMachine> {
        self.provider(provider)?
            .machines
            .iter()
            .find(|candidate| candidate.id == machine)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProviderState {
    pub provider: Provider,
    pub status: ProviderStatus,
    /// As last listed; kept while it's signed out or failing, so they're still shown.
    pub machines: Vec<CloudMachine>,
    pub usage: Option<CloudUsage>,
    pub listed_at: Option<SystemTime>,
    /// Why the last listing failed, while it does.
    pub error: Option<String>,
    /// What the CLI is doing to its machines now.
    #[serde(default)]
    pub changing: Vec<MachineChange>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MachineChange {
    /// The machine's id, or the name of one being made.
    pub machine: String,
    pub change: Change,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Change {
    Create,
    Lifecycle(Lifecycle),
}
