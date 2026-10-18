//! Instabox's CLI (`instabox`, a native binary). Its `--json` outputs are its API's objects
//! (`instabox_outputs/README.md` says which fixtures were captured from it).

use std::time::{Duration, SystemTime};

use agentz_protocol::cloud::{
    AccountRules, CloudMachine, CloudState, CloudTemplate, CloudUsage, CreateMachine, Lifecycle,
    MachineAction, MachineDetails, MachineSize, MachineSnapshot, MachineSource, MachineVault,
    PublishedPort, TimeoutAt, VaultItem, VaultWrite,
};
use anyhow::{Context as _, Result, anyhow};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Machine {
    id: String,
    name: String,
    state: String,
    #[serde(default)]
    template: Option<Template>,
    #[serde(default)]
    spec: Option<Spec>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    error: Option<MachineError>,
}

#[derive(Deserialize)]
struct Template {
    id: String,
    #[serde(default)]
    version: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Spec {
    #[serde(default)]
    vcpus: Option<u32>,
    #[serde(default)]
    mem_mi_b: Option<u64>,
    #[serde(default)]
    persistent_disk_gi_b: Option<u64>,
}

#[derive(Deserialize)]
struct MachineError {
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    code: Option<String>,
}

/// The listing as the API pages it (`{"machines": […]}`), or as a plain list.
#[derive(Deserialize)]
#[serde(untagged)]
enum Listing {
    Page { machines: Vec<Machine> },
    List(Vec<Machine>),
}

pub(crate) fn parse_machines(json: &str) -> Result<Vec<CloudMachine>> {
    let listing: Listing = serde_json::from_str(json).context("parsing the machines")?;
    let machines = match listing {
        Listing::Page { machines } | Listing::List(machines) => machines,
    };
    Ok(machines
        .into_iter()
        .filter(|machine| machine.state != "deleted")
        .map(machine)
        .collect())
}

fn machine(machine: Machine) -> CloudMachine {
    CloudMachine {
        id: machine.id,
        name: machine.name,
        state: state(&machine.state),
        size: size(machine.spec.unwrap_or_default()),
        template: machine.template.map(|template| template.id),
        // Its gateway reaches every machine by its id, so the address names none of them.
        ssh_address: None,
        created_at: time(machine.created_at),
        error: machine
            .error
            .and_then(|error| error.message.or(error.code))
            .filter(|message| !message.is_empty()),
    }
}

fn size(spec: Spec) -> MachineSize {
    MachineSize {
        vcpus: spec.vcpus,
        memory_mib: spec.mem_mi_b,
        memory_ceiling_mib: None,
        disk_gib: spec.persistent_disk_gi_b,
    }
}

pub(crate) fn time(at: Option<String>) -> Option<SystemTime> {
    at.and_then(|at| chrono::DateTime::parse_from_rfc3339(&at).ok())
        .map(SystemTime::from)
}

/// The API's states: it says `suspend` for pause.
fn state(state: &str) -> CloudState {
    match state {
        "creating" => CloudState::Creating,
        "starting" => CloudState::Starting,
        "running" => CloudState::Running,
        "stopping" => CloudState::Stopping,
        "stopped" => CloudState::Stopped,
        "suspending" => CloudState::Pausing,
        "suspended" => CloudState::Paused,
        "resuming" => CloudState::Resuming,
        "deleting" => CloudState::Deleting,
        "error" => CloudState::Failed,
        other => CloudState::Other(other.to_string()),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemplateEntry {
    id: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    default_spec: Option<Spec>,
    /// Kept for machines made from it, not offered for new ones.
    #[serde(default)]
    hidden: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Templates {
    Page { templates: Vec<TemplateEntry> },
    List(Vec<TemplateEntry>),
}

/// The templates new machines can start from. The listing has every version of each, oldest
/// first, and `create --template` takes the latest, so each is kept once, as that one.
pub(crate) fn parse_templates(json: &str) -> Result<Vec<CloudTemplate>> {
    let templates: Templates = serde_json::from_str(json).context("parsing the templates")?;
    let templates = match templates {
        Templates::Page { templates } | Templates::List(templates) => templates,
    };
    let mut kept: Vec<CloudTemplate> = Vec::new();
    for template in templates.into_iter().filter(|template| !template.hidden) {
        let template = CloudTemplate {
            id: template.id,
            description: template
                .description
                .filter(|description| !description.is_empty()),
            size: size(template.default_spec.unwrap_or_default()),
        };
        match kept.iter_mut().find(|kept| kept.id == template.id) {
            Some(older) => *older = template,
            None => kept.push(template),
        }
    }
    Ok(kept)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Whoami {
    login: String,
    #[serde(default)]
    fixed_auto_suspend_seconds: Option<u64>,
    #[serde(default)]
    max_timeout_seconds: Option<u64>,
}

/// `whoami --json`: who's signed in, and what the account lets its machines change.
pub(crate) fn parse_whoami(json: &str) -> Result<(String, AccountRules)> {
    let whoami: Whoami = serde_json::from_str(json).context("parsing whoami")?;
    let seconds = |seconds: Option<u64>| {
        seconds
            .filter(|seconds| *seconds > 0)
            .map(Duration::from_secs)
    };
    Ok((
        whoami.login,
        AccountRules {
            fixed_idle_pause: seconds(whoami.fixed_auto_suspend_seconds),
            max_timeout: seconds(whoami.max_timeout_seconds),
        },
    ))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    id: String,
    name: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    source_machine_id: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    root_size_bytes: Option<f64>,
    /// Its `/data` disk's, which a snapshot saved with `--root-only` doesn't have.
    #[serde(default)]
    data_size_bytes: Option<f64>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Snapshots {
    Page { snapshots: Vec<Snapshot> },
    List(Vec<Snapshot>),
}

pub(crate) fn parse_snapshots(json: &str) -> Result<Vec<MachineSnapshot>> {
    let snapshots: Snapshots = serde_json::from_str(json).context("parsing the snapshots")?;
    let snapshots = match snapshots {
        Snapshots::Page { snapshots } | Snapshots::List(snapshots) => snapshots,
    };
    Ok(snapshots
        .into_iter()
        .map(|snapshot| MachineSnapshot {
            id: snapshot.id,
            name: snapshot.name,
            is_ready: snapshot.status.as_deref() == Some("ready"),
            source_machine: snapshot.source_machine_id,
            created_at: time(snapshot.created_at),
            software_only: snapshot.data_size_bytes.is_none_or(|bytes| bytes <= 0.),
            size_bytes: snapshot
                .root_size_bytes
                .map(|root| (root + snapshot.data_size_bytes.unwrap_or(0.)) as u64),
        })
        .collect())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MachineInfo {
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    template: Option<Template>,
    #[serde(default)]
    timeout_at: Option<String>,
    #[serde(default)]
    on_timeout: Option<String>,
    #[serde(default)]
    auto_suspend_after_seconds: Option<u64>,
    #[serde(default)]
    auto_suspended_at: Option<String>,
    #[serde(default)]
    metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

/// `get <machine> --json`: what its listing doesn't have, and whether it runs.
pub(crate) fn parse_details(json: &str) -> Result<(MachineDetails, bool)> {
    let info: MachineInfo = serde_json::from_str(json).context("parsing the machine")?;
    let template = info.template.map(|template| match template.version {
        Some(version) if !version.is_empty() => format!("{} {version}", template.id),
        _ => template.id,
    });
    let delete = info.on_timeout.as_deref() == Some("kill");
    let labels = info
        .metadata
        .unwrap_or_default()
        .into_iter()
        .map(|(key, value)| match value {
            serde_json::Value::String(value) => (key, value),
            value => (key, value.to_string()),
        })
        .collect();
    let details = MachineDetails {
        template,
        timeout: time(info.timeout_at).map(|at| TimeoutAt { at, delete }),
        idle_pause: info
            .auto_suspend_after_seconds
            .filter(|seconds| *seconds > 0)
            .map(Duration::from_secs),
        idle_paused_at: time(info.auto_suspended_at),
        labels,
        ..MachineDetails::default()
    };
    Ok((details, info.state.as_deref() == Some("running")))
}

#[derive(Deserialize)]
struct Vault {
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    items: Vec<VaultEntry>,
    #[serde(default)]
    applied: Option<VaultApplied>,
}

#[derive(Deserialize)]
struct VaultEntry {
    name: String,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    path: Option<String>,
}

#[derive(Deserialize)]
struct VaultApplied {
    #[serde(default)]
    at: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// `vault show <machine> --json`: what it gets, and its last write.
pub(crate) fn parse_vault(json: &str) -> Result<MachineVault> {
    let vault: Vault = serde_json::from_str(json).context("parsing the Vault")?;
    Ok(MachineVault {
        enabled: vault.enabled,
        items: vault
            .items
            .into_iter()
            .map(|item| VaultItem {
                path: item.path.filter(|_| item.kind.as_deref() != Some("env")),
                name: item.name,
            })
            .collect(),
        last_write: vault.applied.map(|applied| VaultWrite {
            at: time(applied.at),
            error: applied.error.filter(|error| !error.is_empty()),
        }),
    })
}

#[derive(Deserialize)]
struct Port {
    port: u16,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Ports {
    Page { ports: Vec<Port> },
    List(Vec<Port>),
}

/// `port ls <machine> --json` (`{"enabled": true, "ports": []}` captured; its ports'
/// fields are *unverified*).
pub(crate) fn parse_ports(json: &str) -> Result<Vec<PublishedPort>> {
    let ports: Ports = serde_json::from_str(json).context("parsing the ports")?;
    let (Ports::Page { ports } | Ports::List(ports)) = ports;
    let mut ports: Vec<PublishedPort> = ports
        .into_iter()
        .map(|port| PublishedPort {
            port: port.port,
            name: port.name.filter(|name| !name.is_empty()),
            url: port.url.filter(|url| !url.is_empty()),
        })
        .collect();
    ports.sort_by_key(|port| port.port);
    Ok(ports)
}

/// The URL `port publish --json` gives the port, where it's found.
pub(crate) fn published_url(json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    find_string(&value, &|text| text.starts_with("https://")).map(str::to_string)
}

/// `ssh-token <machine> --json`'s command, for any terminal.
pub(crate) fn parse_ssh_command(stdout: &str) -> Result<String> {
    let command = match serde_json::from_str::<serde_json::Value>(stdout) {
        Ok(value) => value
            .get("command")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        Err(_) => stdout
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("ssh "))
            .map(str::to_string),
    };
    command
        .filter(|command| !command.is_empty())
        .context("Instabox gave no SSH command")
}

/// The first string in `value` that `matches`, depth first.
pub(crate) fn find_string<'a>(
    value: &'a serde_json::Value,
    matches: &dyn Fn(&str) -> bool,
) -> Option<&'a str> {
    match value {
        serde_json::Value::String(text) => Some(text.as_str()).filter(|text| matches(text)),
        serde_json::Value::Array(values) => {
            values.iter().find_map(|value| find_string(value, matches))
        }
        serde_json::Value::Object(fields) => fields
            .values()
            .find_map(|value| find_string(value, matches)),
        _ => None,
    }
}

/// What the CLI runs for a page's action, all but the desktop link, which only its API makes.
pub(crate) fn action_args(machine: &str, action: &MachineAction) -> Result<Vec<String>> {
    let mut args: Vec<String> = Vec::new();
    let mut push = |items: &[&str]| args.extend(items.iter().map(|item| item.to_string()));
    match action {
        MachineAction::SaveSnapshot {
            name,
            software_only,
        } => {
            push(&["snapshot", "save", machine, "--name", name]);
            if *software_only {
                push(&["--root-only"]);
            }
            push(&["--json"]);
        }
        MachineAction::DeleteSnapshot(snapshot) => push(&["snapshot", "rm", snapshot]),
        MachineAction::PublishPort { port, name } => {
            push(&["port", "publish", machine, &port.to_string()]);
            if let Some(name) = name.as_deref().filter(|name| !name.is_empty()) {
                push(&["--name", name]);
            }
            push(&["--json"]);
        }
        MachineAction::UnpublishPort(port) => {
            push(&["port", "unpublish", machine, &port.to_string()]);
        }
        MachineAction::PushVault => push(&["vault", "push", machine, "--json"]),
        MachineAction::SetTimeout(Some(timeout)) => push(&[
            "timeout",
            machine,
            &duration_arg(timeout.after),
            "--on-timeout",
            if timeout.delete { "kill" } else { "pause" },
        ]),
        MachineAction::SetTimeout(None) => push(&["timeout", machine, "0"]),
        MachineAction::SetIdlePause(after) => {
            push(&["auto-suspend", machine, &duration_arg(*after)])
        }
        MachineAction::SshCommand => push(&["ssh-token", machine, "--json"]),
        MachineAction::RevokeSshCommands => push(&["ssh-token", "--revoke", machine]),
        MachineAction::DesktopLink { .. } => {
            return Err(anyhow!("Instabox's CLI makes no desktop links"));
        }
        MachineAction::ReplaceSshKeys(_) => {
            return Err(anyhow!(
                "Instabox machines are reached with its certificates, not keys"
            ));
        }
    }
    Ok(args)
}

/// A duration as the CLI takes it, where zero turns the setting off.
fn duration_arg(after: Duration) -> String {
    match after.as_secs() {
        0 => "0".to_string(),
        seconds => format!("{seconds}s"),
    }
}

pub(crate) fn create_args(machine: &CreateMachine) -> Vec<String> {
    let mut args = vec!["create".to_string(), "--name".into(), machine.name.clone()];
    match &machine.source {
        Some(MachineSource::Template(template)) => {
            args.extend(["--template".into(), template.clone()]);
        }
        Some(MachineSource::Snapshot(snapshot)) => {
            args.extend(["--snapshot".into(), snapshot.clone()]);
        }
        None => {}
    }
    if let Some(timeout) = machine.timeout {
        args.extend([
            "--timeout".into(),
            format!("{}s", timeout.after.as_secs()),
            "--on-timeout".into(),
            if timeout.delete { "kill" } else { "pause" }.into(),
        ]);
    }
    if let Some(after) = machine.auto_suspend {
        args.extend(["--auto-suspend".into(), duration_arg(after)]);
    }
    args
}

/// The CLI says `suspend` for pause and `rm` for delete.
pub(crate) fn change_args(machine: &str, change: Lifecycle) -> Vec<String> {
    let verb = match change {
        Lifecycle::Start => "start",
        Lifecycle::Stop => "stop",
        Lifecycle::Pause => "suspend",
        Lifecycle::Resume => "resume",
        // Its API has no restart.
        Lifecycle::Restart => "start",
        Lifecycle::Delete => "rm",
    };
    vec![verb.to_string(), machine.to_string()]
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Usage {
    machines: u32,
    snapshots: u32,
    #[serde(default)]
    stored_bytes: Option<u64>,
    #[serde(default)]
    limits: Option<Limits>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Limits {
    #[serde(default)]
    max_machines: Option<u32>,
    #[serde(default)]
    max_snapshots: Option<u32>,
    #[serde(default)]
    max_stored_gi_b: Option<u64>,
}

pub(crate) fn parse_usage(json: &str) -> Result<CloudUsage> {
    let usage: Usage = serde_json::from_str(json).context("parsing the usage")?;
    let limits = usage.limits.unwrap_or(Limits {
        max_machines: None,
        max_snapshots: None,
        max_stored_gi_b: None,
    });
    Ok(CloudUsage::Limits {
        machines: usage.machines,
        max_machines: limits.max_machines,
        snapshots: usage.snapshots,
        max_snapshots: limits.max_snapshots,
        stored_bytes: usage.stored_bytes,
        max_stored_gib: limits.max_stored_gi_b,
    })
}

#[cfg(test)]
mod tests {
    use agentz_protocol::cloud::MachineTimeout;

    use super::*;

    #[test]
    fn reads_the_listing() {
        let machines = parse_machines(include_str!("instabox_outputs/ls.json")).expect("parses");
        assert_eq!(machines.len(), 1);
        assert_eq!(machines[0].id, "m_q4mz7tk2bx5rn3wd6vp2hj4sye");
        assert_eq!(machines[0].name, "agents-1");
        assert_eq!(machines[0].state, CloudState::Paused);
        assert_eq!(machines[0].template.as_deref(), Some("agent-browser"));
        assert_eq!(
            machines[0].size,
            MachineSize {
                vcpus: Some(4),
                memory_mib: Some(8192),
                memory_ceiling_mib: None,
                disk_gib: Some(20),
            }
        );
        assert!(machines[0].created_at.is_some());
        assert_eq!(machines[0].error, None);
    }

    #[test]
    fn reads_a_page_and_errors_too() {
        let machines = parse_machines(
            r#"{"machines": [{"id": "m_a", "name": "a", "state": "error", "error": {"code": "no_capacity", "message": "no capacity"}}], "nextCursor": null}"#,
        )
        .expect("parses");
        assert_eq!(machines[0].state, CloudState::Failed);
        assert_eq!(machines[0].error.as_deref(), Some("no capacity"));
    }

    #[test]
    fn reads_the_usage() {
        assert_eq!(
            parse_usage(include_str!("instabox_outputs/usage.json")).expect("parses"),
            CloudUsage::Limits {
                machines: 1,
                max_machines: Some(5),
                snapshots: 0,
                max_snapshots: Some(10),
                stored_bytes: Some(6643454),
                max_stored_gib: Some(200),
            }
        );
    }

    #[test]
    fn offers_each_template_once_without_the_hidden_ones() {
        let templates =
            parse_templates(include_str!("instabox_outputs/templates.json")).expect("parses");
        let ids: Vec<&str> = templates
            .iter()
            .map(|template| template.id.as_str())
            .collect();
        assert_eq!(ids, ["herdr", "orca", "universal", "agent-browser"]);
        assert_eq!(templates[0].size.memory_mib, Some(8192));
        assert_eq!(templates[0].description, None);
        assert_eq!(
            parse_templates(r#"[{"id": "orca", "description": "Orca's runtime"}]"#)
                .expect("parses")[0]
                .description
                .as_deref(),
            Some("Orca's runtime")
        );
    }

    #[test]
    fn reads_who_is_signed_in_and_the_accounts_rules() {
        let (account, rules) =
            parse_whoami(include_str!("instabox_outputs/whoami.json")).expect("parses");
        assert_eq!(account, "me@example.com");
        assert_eq!(
            rules,
            AccountRules {
                fixed_idle_pause: Some(Duration::from_secs(1800)),
                max_timeout: Some(Duration::from_secs(4 * 3600)),
            }
        );
        let (_, rules) = parse_whoami(r#"{"login": "me@example.com"}"#).expect("parses");
        assert_eq!(rules, AccountRules::default());
    }

    #[test]
    fn reads_the_snapshots() {
        let snapshots =
            parse_snapshots(include_str!("instabox_outputs/snapshots.json")).expect("parses");
        assert_eq!(snapshots.len(), 2);
        assert_eq!(snapshots[0].name, "dev-base");
        assert!(snapshots[0].is_ready);
        assert_eq!(
            snapshots[0].source_machine.as_deref(),
            Some("m_q4mz7tk2bx5rn3wd6vp2hj4sye")
        );
        assert!(snapshots[0].created_at.is_some());
        assert!(!snapshots[0].software_only);
        assert_eq!(snapshots[0].size_bytes, Some(38654705664));
        assert!(!snapshots[1].is_ready);
        assert!(snapshots[1].software_only);
        assert_eq!(parse_snapshots("[]").expect("parses"), []);
    }

    #[test]
    fn reads_what_get_has_beyond_the_listing() {
        let (details, running) =
            parse_details(include_str!("instabox_outputs/get.json")).expect("parses");
        assert!(!running);
        assert_eq!(
            details.template.as_deref(),
            Some("agent-browser 2026-10-15.4")
        );
        assert_eq!(details.timeout, None);
        assert_eq!(details.idle_pause, Some(Duration::from_secs(1800)));
        assert!(details.idle_paused_at.is_some());
        assert_eq!(details.labels, []);

        let (details, running) = parse_details(
            r#"{"state": "running", "timeoutAt": "2026-10-18T12:00:00Z", "onTimeout": "kill",
                "autoSuspendAfterSeconds": 0, "metadata": {"team": "web", "tier": 2}}"#,
        )
        .expect("parses");
        assert!(running);
        assert!(details.timeout.is_some_and(|timeout| timeout.delete));
        assert_eq!(details.idle_pause, None);
        assert_eq!(
            details.labels,
            [("team".into(), "web".into()), ("tier".into(), "2".into())]
        );
    }

    #[test]
    fn reads_the_vault_and_ports() {
        assert_eq!(
            parse_vault(include_str!("instabox_outputs/vault-show.json")).expect("parses"),
            MachineVault {
                enabled: true,
                items: Vec::new(),
                last_write: None,
            }
        );
        let vault = parse_vault(
            r#"{"enabled": true, "items": [{"name": "GITHUB_TOKEN", "kind": "env"},
                {"name": "npmrc", "kind": "file", "path": ".npmrc"}],
                "applied": {"at": "2026-10-18T09:00:00Z", "env": ["GITHUB_TOKEN"], "files": []}}"#,
        )
        .expect("parses");
        assert_eq!(vault.items[0].path, None);
        assert_eq!(vault.items[1].path.as_deref(), Some(".npmrc"));
        assert!(
            vault
                .last_write
                .is_some_and(|write| write.at.is_some() && write.error.is_none())
        );

        assert_eq!(
            parse_ports(include_str!("instabox_outputs/port-ls.json")).expect("parses"),
            []
        );
        let ports = parse_ports(
            r#"{"enabled": true, "ports": [{"port": 8080, "name": "", "url": "https://p-8080.instabox.app"},
                {"port": 3000, "name": "web"}]}"#,
        )
        .expect("parses");
        assert_eq!(
            ports,
            [
                PublishedPort {
                    port: 3000,
                    name: Some("web".into()),
                    url: None,
                },
                PublishedPort {
                    port: 8080,
                    name: None,
                    url: Some("https://p-8080.instabox.app".into()),
                },
            ]
        );
        assert_eq!(
            published_url(r#"{"port": {"port": 3000, "url": "https://web.instabox.app"}}"#)
                .as_deref(),
            Some("https://web.instabox.app")
        );
    }

    #[test]
    fn reads_the_ssh_command() {
        assert_eq!(
            parse_ssh_command(
                r#"{"command": "ssh -p 2222 t_abc@api.instabox.sh", "expiresAt": "2026-10-25T00:00:00Z"}"#
            )
            .expect("parses"),
            "ssh -p 2222 t_abc@api.instabox.sh"
        );
        assert_eq!(
            parse_ssh_command("Works for 7 days:\n  ssh -p 2222 t_abc@api.instabox.sh\n")
                .expect("parses"),
            "ssh -p 2222 t_abc@api.instabox.sh"
        );
        assert!(parse_ssh_command("{}").is_err());
    }

    #[test]
    fn the_pages_actions_run_the_cli() {
        let args = |action| action_args("m_1", &action).expect("has args").join(" ");
        assert_eq!(
            args(MachineAction::SaveSnapshot {
                name: "base".into(),
                software_only: true,
            }),
            "snapshot save m_1 --name base --root-only --json"
        );
        assert_eq!(
            args(MachineAction::DeleteSnapshot("s_1".into())),
            "snapshot rm s_1"
        );
        assert_eq!(
            args(MachineAction::PublishPort {
                port: 3000,
                name: Some("web".into()),
            }),
            "port publish m_1 3000 --name web --json"
        );
        assert_eq!(
            args(MachineAction::UnpublishPort(3000)),
            "port unpublish m_1 3000"
        );
        assert_eq!(args(MachineAction::PushVault), "vault push m_1 --json");
        assert_eq!(
            args(MachineAction::SetTimeout(Some(MachineTimeout {
                after: Duration::from_secs(3600),
                delete: false,
            }))),
            "timeout m_1 3600s --on-timeout pause"
        );
        assert_eq!(args(MachineAction::SetTimeout(None)), "timeout m_1 0");
        assert_eq!(
            args(MachineAction::SetIdlePause(Duration::ZERO)),
            "auto-suspend m_1 0"
        );
        assert_eq!(args(MachineAction::SshCommand), "ssh-token m_1 --json");
        assert_eq!(
            args(MachineAction::RevokeSshCommands),
            "ssh-token --revoke m_1"
        );
        assert!(action_args("m_1", &MachineAction::DesktopLink { view_only: true }).is_err());
    }

    #[test]
    fn creating_passes_what_was_picked() {
        let machine = CreateMachine {
            name: "agents-2".into(),
            source: Some(MachineSource::Snapshot("dev-base".into())),
            timeout: Some(MachineTimeout {
                after: Duration::from_secs(30 * 60),
                delete: true,
            }),
            auto_suspend: Some(Duration::ZERO),
        };
        assert_eq!(
            create_args(&machine),
            [
                "create",
                "--name",
                "agents-2",
                "--snapshot",
                "dev-base",
                "--timeout",
                "1800s",
                "--on-timeout",
                "kill",
                "--auto-suspend",
                "0",
            ]
        );
    }
}
