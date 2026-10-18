//! Proton's CLI (`proton` 0.1.2, one Python file): `list --json` gives its API's computers as
//! they are, `balance` prints text, and `start`, `restart`, `create` and `delete` ask before
//! charging unless given `--yes`: agentZ asks in its own UI first (topic 5).

use std::path::Path;
use std::time::{Duration, SystemTime};

use agentz_protocol::cloud::{
    CloudMachine, CloudState, CloudUsage, CreateMachine, Lifecycle, MachineAction, MachineDetails,
    MachineSize, SshAddress,
};
use anyhow::{Context as _, Result, anyhow};
use serde::Deserialize;

#[derive(Deserialize)]
struct Computer {
    id: String,
    name: String,
    state: String,
    #[serde(default)]
    vcpus: Option<u32>,
    #[serde(default)]
    floor_mib: Option<u64>,
    #[serde(default)]
    ceiling_mib: Option<u64>,
    #[serde(default)]
    disk_gib: Option<u64>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    created: Option<f64>,
    #[serde(default)]
    public_ssh: Option<PublicSsh>,
}

#[derive(Deserialize)]
struct PublicSsh {
    hostname: String,
    port: u16,
    /// `1` or `true` while SSH reaches it.
    #[serde(default)]
    enabled: serde_json::Value,
}

pub(crate) fn parse_machines(json: &str) -> Result<Vec<CloudMachine>> {
    let computers: Vec<Computer> = serde_json::from_str(json).context("parsing the computers")?;
    Ok(computers.into_iter().map(machine).collect())
}

fn seconds_since_epoch(seconds: f64) -> Option<SystemTime> {
    (seconds.is_finite() && seconds >= 0.)
        .then(|| SystemTime::UNIX_EPOCH + Duration::from_secs_f64(seconds))
}

#[derive(Deserialize)]
struct Show {
    machine: ShownComputer,
    #[serde(default)]
    operations: Vec<Operation>,
}

#[derive(Deserialize)]
struct ShownComputer {
    state: String,
    #[serde(default)]
    last_activity: Option<f64>,
}

#[derive(Deserialize)]
struct Operation {
    action: String,
    status: String,
    #[serde(default)]
    finished: Option<f64>,
}

/// `show <computer> --json`: when a running computer last started, which its operations say,
/// and its last activity.
pub(crate) fn parse_details(json: &str) -> Result<MachineDetails> {
    let show: Show = serde_json::from_str(json).context("parsing the computer")?;
    let started_at = (show.machine.state == "running")
        .then(|| {
            show.operations
                .iter()
                .filter(|operation| {
                    operation.status == "succeeded"
                        && matches!(operation.action.as_str(), "start" | "restart" | "create")
                })
                .filter_map(|operation| operation.finished)
                .max_by(f64::total_cmp)
        })
        .flatten()
        .and_then(seconds_since_epoch);
    Ok(MachineDetails {
        started_at,
        last_activity: show.machine.last_activity.and_then(seconds_since_epoch),
    })
}

/// A page's action: only its SSH keys, which `keys` replaces on a running computer. Stop when
/// idle is agentZ's own.
pub(crate) fn action_args(machine: &str, action: &MachineAction) -> Result<Vec<String>> {
    let MachineAction::ReplaceSshKeys(keys) = action else {
        return Err(anyhow!("Proton computers can't do that"));
    };
    anyhow::ensure!(!keys.is_empty(), "a computer needs at least one SSH key");
    let mut args = vec!["keys".to_string(), machine.to_string()];
    for key in keys {
        let key = key
            .to_str()
            .with_context(|| format!("{} isn't a path Proton's CLI can take", key.display()))?;
        args.extend(["--key".to_string(), key.to_string()]);
    }
    Ok(args)
}

fn machine(computer: Computer) -> CloudMachine {
    let ssh_address = computer
        .public_ssh
        .filter(|ssh| match &ssh.enabled {
            serde_json::Value::Bool(enabled) => *enabled,
            serde_json::Value::Number(number) => number.as_u64().is_some_and(|n| n != 0),
            _ => false,
        })
        .map(|ssh| SshAddress {
            host: ssh.hostname,
            port: ssh.port,
        });
    CloudMachine {
        id: computer.id,
        name: computer.name,
        state: state(&computer.state),
        size: MachineSize {
            vcpus: computer.vcpus,
            memory_mib: computer.floor_mib,
            memory_ceiling_mib: computer.ceiling_mib,
            disk_gib: computer.disk_gib,
        },
        ssh_address,
        created_at: computer.created.and_then(seconds_since_epoch),
        error: computer.error.filter(|error| !error.is_empty()),
    }
}

/// Proton's states. Only `running` and `stopped` have been seen; the rest are the words its
/// dashboard and CLI use (*unverified*).
fn state(state: &str) -> CloudState {
    match state {
        "running" => CloudState::Running,
        "stopped" => CloudState::Stopped,
        "provisioning" | "creating" => CloudState::Creating,
        "starting" => CloudState::Starting,
        "stopping" => CloudState::Stopping,
        "restarting" => CloudState::Starting,
        "suspended" => CloudState::Suspended,
        "deleting" => CloudState::Deleting,
        "error" | "failed" => CloudState::Failed,
        other => CloudState::Other(other.to_string()),
    }
}

/// `create` takes one public key, the one the computer is reached with: agentZ's own, or with
/// none the CLI's default, `~/.ssh/id_ed25519.pub` (or `id_ecdsa` or `id_rsa`).
pub(crate) fn create_args(machine: &CreateMachine, key: Option<&Path>) -> Result<Vec<String>> {
    let mut args = vec!["create".into(), machine.name.clone(), "--yes".into()];
    if let Some(key) = key {
        let key = key
            .to_str()
            .with_context(|| format!("{} isn't a path Proton's CLI can take", key.display()))?;
        args.extend(["--key".to_string(), key.to_string()]);
    }
    Ok(args)
}

pub(crate) fn change_args(machine: &str, change: Lifecycle) -> Vec<String> {
    let args: &[&str] = match change {
        Lifecycle::Start => &["start", machine, "--yes"],
        Lifecycle::Restart => &["restart", machine, "--yes"],
        Lifecycle::Stop => &["stop", machine],
        Lifecycle::Delete => &["delete", machine, "--yes"],
    };
    args.iter().map(|arg| arg.to_string()).collect()
}

/// `balance`'s text: "Shared Cube balance: $24.27" (or "No prepaid Cube balance…"), then
/// "Proton runtime: 26.78 hours · deducted $0.7336"; "Billing is not active yet." alone when
/// billing is off.
pub(crate) fn parse_balance(text: &str) -> Result<CloudUsage> {
    if text.contains("Billing is not active yet") {
        return Ok(CloudUsage::NotBilled);
    }
    let balance_micro_usd = match text
        .lines()
        .find_map(|line| line.trim().strip_prefix("Shared Cube balance: "))
    {
        Some(amount) => Some(micro_usd(amount)?),
        None if text.contains("No prepaid Cube balance") => None,
        None => return Err(anyhow!("no balance in {text:?}")),
    };
    let runtime = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("Proton runtime: "))
        .with_context(|| format!("no runtime in {text:?}"))?;
    let (hours, charged) = runtime
        .split_once(" hours · deducted ")
        .with_context(|| format!("can't read the runtime {runtime:?}"))?;
    Ok(CloudUsage::Balance {
        balance_micro_usd,
        running_hours: hours.trim().parse().context("reading the hours")?,
        charged_micro_usd: micro_usd(charged)?,
    })
}

/// `$24.27` in millionths of a dollar.
fn micro_usd(amount: &str) -> Result<i64> {
    let amount = amount.trim();
    let (negative, amount) = match amount.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, amount),
    };
    let digits = amount
        .strip_prefix('$')
        .with_context(|| format!("{amount:?} isn't in dollars"))?;
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    anyhow::ensure!(
        fraction.len() <= 6 && fraction.chars().all(|c| c.is_ascii_digit()),
        "can't read {amount:?}"
    );
    let whole: i64 = whole
        .parse()
        .with_context(|| format!("can't read {amount:?}"))?;
    let fraction: i64 = format!("{fraction:0<6}")
        .parse()
        .with_context(|| format!("can't read {amount:?}"))?;
    let micro = whole * 1_000_000 + fraction;
    Ok(if negative { -micro } else { micro })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_captured_list() {
        let machines = parse_machines(include_str!("proton_outputs/list.json")).expect("parses");
        assert_eq!(
            machines,
            [CloudMachine {
                id: "3f1c9a52-8b7e-4d21-9c0a-6e5f4b3a2d10".into(),
                name: "devbox-3".into(),
                state: CloudState::Running,
                size: MachineSize {
                    vcpus: Some(8),
                    memory_mib: Some(2048),
                    memory_ceiling_mib: Some(65536),
                    disk_gib: Some(50),
                },
                ssh_address: Some(SshAddress {
                    host: "ssh.proton.computer".into(),
                    port: 22021,
                }),
                created_at: Some(
                    SystemTime::UNIX_EPOCH + Duration::from_secs_f64(1792024266.4963777)
                ),
                error: None,
            }]
        );
    }

    #[test]
    fn show_has_the_same_computer() {
        #[derive(Deserialize)]
        struct Show {
            machine: Computer,
        }
        let show: Show =
            serde_json::from_str(include_str!("proton_outputs/show.json")).expect("parses");
        assert_eq!(machine(show.machine).name, "devbox-3");
    }

    #[test]
    fn show_says_when_it_started() {
        let details = parse_details(include_str!("proton_outputs/show.json")).expect("parses");
        assert_eq!(
            details.started_at,
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs_f64(1792244584.507196))
        );
        assert_eq!(
            details.last_activity,
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs_f64(1792244584.5072565))
        );
        let stopped = include_str!("proton_outputs/show.json").replacen(
            r#""state": "running""#,
            r#""state": "stopped""#,
            1,
        );
        assert_eq!(parse_details(&stopped).expect("parses").started_at, None);
    }

    #[test]
    fn replaces_the_ssh_keys_with_the_files_picked() {
        let keys = MachineAction::ReplaceSshKeys(vec![
            "/Users/me/.ssh/id_ed25519.pub".into(),
            "/Users/me/.ssh/work.pub".into(),
        ]);
        assert_eq!(
            action_args("devbox-3", &keys).expect("has args"),
            [
                "keys",
                "devbox-3",
                "--key",
                "/Users/me/.ssh/id_ed25519.pub",
                "--key",
                "/Users/me/.ssh/work.pub",
            ]
        );
        assert!(action_args("devbox-3", &MachineAction::ReplaceSshKeys(Vec::new())).is_err());
        assert!(action_args("devbox-3", &MachineAction::StopWhenIdle(None)).is_err());
    }

    #[test]
    fn a_stopped_computer_has_no_ssh() {
        let machines = parse_machines(
            r#"[{"id": "c1", "name": "scratch", "state": "stopped", "error": "",
                 "public_ssh": {"hostname": "ssh.proton.computer", "port": 22030, "enabled": 0}}]"#,
        )
        .expect("parses");
        assert_eq!(machines[0].state, CloudState::Stopped);
        assert_eq!(machines[0].ssh_address, None);
    }

    #[test]
    fn reads_the_captured_balance() {
        assert_eq!(
            parse_balance(include_str!("proton_outputs/balance.txt")).expect("parses"),
            CloudUsage::Balance {
                balance_micro_usd: Some(24_270_000),
                running_hours: 26.78,
                charged_micro_usd: 733_600,
            }
        );
    }

    /// The other forms, as the CLI's source prints them.
    #[test]
    fn reads_a_balance_with_no_credit_or_no_billing() {
        assert_eq!(
            parse_balance(
                "No prepaid Cube balance. Add credit: https://app.cube.computer/billing\n\
                 Proton runtime: 0.00 hours · deducted $0.0000\n\
                 Price: $20 per 730 running hours\n"
            )
            .expect("parses"),
            CloudUsage::Balance {
                balance_micro_usd: None,
                running_hours: 0.,
                charged_micro_usd: 0,
            }
        );
        assert_eq!(
            parse_balance("Billing is not active yet.\n").expect("parses"),
            CloudUsage::NotBilled
        );
    }

    #[test]
    fn creating_gives_the_key_it_has() {
        let machine = CreateMachine {
            name: "scratch".into(),
        };
        assert_eq!(
            create_args(&machine, None).expect("has args"),
            ["create", "scratch", "--yes"]
        );
        assert_eq!(
            create_args(&machine, Some(Path::new("/data/cloud/id_ed25519.pub"))).expect("has args"),
            [
                "create",
                "scratch",
                "--yes",
                "--key",
                "/data/cloud/id_ed25519.pub"
            ]
        );
    }
}
