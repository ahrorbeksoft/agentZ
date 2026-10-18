//! Instabox's CLI (`instabox`, a native binary). Its `--json` outputs are taken to be its API's
//! objects, as its API reference shows them (`instabox_outputs/README.md`): the CLI wasn't
//! installed when this was written (*unverified*).

use std::time::SystemTime;

use agentz_protocol::cloud::{
    CloudMachine, CloudState, CloudTemplate, CloudUsage, CreateMachine, Lifecycle, MachineSize,
    MachineSnapshot, MachineSource,
};
use anyhow::{Context as _, Result};
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

fn time(at: Option<String>) -> Option<SystemTime> {
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
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Templates {
    Page { templates: Vec<TemplateEntry> },
    List(Vec<TemplateEntry>),
}

pub(crate) fn parse_templates(json: &str) -> Result<Vec<CloudTemplate>> {
    let templates: Templates = serde_json::from_str(json).context("parsing the templates")?;
    let templates = match templates {
        Templates::Page { templates } | Templates::List(templates) => templates,
    };
    Ok(templates
        .into_iter()
        .map(|template| CloudTemplate {
            id: template.id,
            description: template
                .description
                .filter(|description| !description.is_empty()),
            size: size(template.default_spec.unwrap_or_default()),
        })
        .collect())
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
        })
        .collect())
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
        let after = after.as_secs();
        args.extend([
            "--auto-suspend".into(),
            if after == 0 {
                "0".into()
            } else {
                format!("{after}s")
            },
        ]);
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
    use std::time::Duration;

    use agentz_protocol::cloud::MachineTimeout;

    use super::*;

    #[test]
    fn reads_the_listing() {
        let machines = parse_machines(include_str!("instabox_outputs/ls.json")).expect("parses");
        assert_eq!(machines.len(), 2);
        assert_eq!(machines[0].name, "agents-1");
        assert_eq!(machines[0].state, CloudState::Running);
        assert_eq!(machines[0].template.as_deref(), Some("universal"));
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
        assert_eq!(machines[1].state, CloudState::Paused);
    }

    #[test]
    fn reads_a_plain_list_too() {
        let machines = parse_machines(
            r#"[{"id": "m_a", "name": "a", "state": "error", "error": {"code": "no_capacity", "message": "no capacity"}}]"#,
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
                machines: 2,
                max_machines: Some(5),
                snapshots: 1,
                max_snapshots: Some(10),
                stored_bytes: Some(6442450944),
                max_stored_gib: Some(200),
            }
        );
    }

    #[test]
    fn reads_the_templates() {
        let templates =
            parse_templates(include_str!("instabox_outputs/templates.json")).expect("parses");
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].id, "universal");
        assert_eq!(
            templates[0].description.as_deref(),
            Some("Docker, Python (uv), Node, Go and Rust")
        );
        assert_eq!(templates[0].size.memory_mib, Some(8192));
        assert_eq!(
            parse_templates(r#"[{"id": "orca"}]"#).expect("parses")[0].description,
            None
        );
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
        assert!(!snapshots[1].is_ready);
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
