//! Cloud providers (design/cloud): machines a provider's CLI makes, starts and stops, which
//! agentZ then reaches over SSH like any other machine. The user installs each CLI and signs in
//! with it; this Mac's server runs it, and reports what it found in these types.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    Proton,
}

impl Provider {
    /// In the order Settings › Machines lists them.
    pub const ALL: [Provider; 1] = [Provider::Proton];

    pub fn description(self) -> &'static ProviderDescription {
        match self {
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
    /// Its machines' SSH keys can be replaced.
    pub ssh_keys: bool,
    /// agentZ stops its machines once they've been idle the time picked (topic 12 B): they
    /// bill while they run.
    pub idle_stop: bool,
}

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
        ssh_keys: true,
        idle_stop: true,
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
    },
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
    Delete,
}

/// What a machine's page shows (topic 7 A) beyond its listing, as its provider tells it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MachineDetails {
    /// When it last started, while it runs.
    pub started_at: Option<SystemTime>,
    pub last_activity: Option<SystemTime>,
}

/// What a machine's page does beyond its lifecycle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MachineAction {
    /// Replaces the SSH keys that reach it with these public key files on this Mac.
    ReplaceSshKeys(Vec<std::path::PathBuf>),
    /// Stops it once no agent has worked on it and nothing on it has been open this long;
    /// `None` never does. This Mac's server keeps it and does it, not the provider.
    StopWhenIdle(Option<Duration>),
}

/// What a new machine is made with.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CreateMachine {
    pub name: String,
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
    /// What happened to the machines that the app tells the user of (topic 12 C and D),
    /// oldest first, until an app has.
    #[serde(default)]
    pub notices: Vec<CloudNotice>,
    /// The last notice's id; ids aren't reused.
    #[serde(default)]
    pub last_notice: u64,
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
    /// How long each machine, by its id, may be idle before agentZ stops it (topic 12 B).
    #[serde(default)]
    pub stop_when_idle: BTreeMap<String, Duration>,
    /// The balance's low-balance notice was given, until the balance lasts a day again.
    #[serde(default)]
    pub balance_low: bool,
}

/// Something the user is told of with a notification, from this Mac's server.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloudNotice {
    pub id: u64,
    pub provider: Provider,
    pub kind: NoticeKind,
    pub at: SystemTime,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NoticeKind {
    /// It stopped without agentZ asking (topic 12 D), or was suspended when its balance ran
    /// out.
    Stopped {
        machine: String,
        name: String,
        state: CloudState,
        /// What the app last said was working on it.
        working: Vec<String>,
    },
    /// The balance runs out within a day at the rate its running machines cost (topic 12 C).
    LowBalance {
        balance_micro_usd: i64,
        hours_left: f64,
        /// The running machines' names.
        running: Vec<String>,
    },
}

/// What the app says of a provider machine's use, for stopping it once it's idle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CloudActivity {
    pub provider: Provider,
    pub machine: String,
    /// What's working on it: its threads' titles, and the agents in its workspaces.
    pub working: Vec<String>,
    /// A thread, terminal or workspace of it is open in the app.
    pub open: bool,
}

/// How long a balance lasts at a price an hour, in hours.
pub fn hours_of_balance(balance_micro_usd: i64, hourly_micro_usd: u64) -> f64 {
    if hourly_micro_usd == 0 {
        return f64::INFINITY;
    }
    balance_micro_usd.max(0) as f64 / hourly_micro_usd as f64
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
