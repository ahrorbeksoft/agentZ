//! What kind of machine the server runs on, for its icon: t3code's best-effort hardware
//! detection (`apps/server/src/environment/ServerEnvironmentMachine.ts`). Every probe may fail;
//! `None` means no signal, and clients draw a server until the user picks a kind.

use std::path::Path;
use std::time::Duration;

use agentz_protocol::MachineKind;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};

const DMI_ROOT: &str = "/sys/class/dmi/id";
const KERNEL_RELEASE_PATH: &str = "/proc/sys/kernel/osrelease";
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// Hypervisors and cloud providers write themselves into the DMI vendor or product, and a VM
/// reads as a cloud machine whatever chassis it claims. Hyper-V is matched on its "Virtual
/// Machine" product, not the "Microsoft Corporation" vendor Surface devices share.
const VIRTUALIZATION_MARKERS: &[&str] = &[
    "qemu",
    "kvm",
    "bochs",
    "vmware",
    "virtualbox",
    "innotek",
    "xen",
    "parallels",
    "amazon ec2",
    "google compute engine",
    "digitalocean",
    "hetzner",
    "linode",
    "vultr",
    "scaleway",
    "openstack",
    "cloud",
    "virtual machine",
];

/// SMBIOS System Enclosure types. Codes for shapes rather than machines (docking stations,
/// blade enclosures) give no signal.
fn kind_from_chassis_type(chassis_type: &str) -> Option<MachineKind> {
    match chassis_type {
        "3" | "4" | "5" | "6" | "7" | "13" | "15" | "16" | "35" => Some(MachineKind::Desktop),
        "8" | "9" | "10" | "14" | "31" | "32" => Some(MachineKind::Laptop),
        "17" | "18" | "19" | "20" | "21" | "22" | "23" | "24" | "28" => Some(MachineKind::Server),
        _ => None,
    }
}

/// Marketing names ("Mac mini (2024)") and Intel-era model identifiers ("Macmini8,1") share
/// these prefixes.
fn kind_from_apple_product_name(name: &str) -> Option<MachineKind> {
    let name: String = name
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_lowercase();
    if name.starts_with("macmini") {
        Some(MachineKind::MacMini)
    } else if name.starts_with("macstudio") {
        Some(MachineKind::MacStudio)
    } else if name.starts_with("macbook") {
        Some(MachineKind::Laptop)
    } else if name.starts_with("imac") || name.starts_with("macpro") {
        Some(MachineKind::Desktop)
    } else {
        None
    }
}

fn kind_from_dmi(
    chassis_type: Option<&str>,
    sys_vendor: Option<&str>,
    product_name: Option<&str>,
) -> Option<MachineKind> {
    let product_name = product_name.unwrap_or_default();
    let vendor_and_product =
        format!("{} {product_name}", sys_vendor.unwrap_or_default()).to_lowercase();
    if VIRTUALIZATION_MARKERS
        .iter()
        .any(|marker| vendor_and_product.contains(marker))
    {
        return Some(MachineKind::Cloud);
    }
    // Apple hardware booting Linux (Asahi) still reports the Apple product name.
    kind_from_apple_product_name(product_name).or_else(|| kind_from_chassis_type(chassis_type?))
}

/// The machine's kind, from its hardware.
pub(crate) async fn detect() -> Option<MachineKind> {
    match std::env::consts::OS {
        "macos" => detect_mac().await,
        "linux" => detect_linux().await,
        _ => None,
    }
}

/// IOKit's `product` node has the marketing name on Apple silicon; Intel Macs lack it, so
/// `hw.model` is the fallback.
async fn detect_mac() -> Option<MachineKind> {
    let product_name = probe("ioreg", &["-rd1", "-n", "product"])
        .await
        .and_then(|output| ioreg_product_name(&output));
    if let Some(kind) = product_name
        .as_deref()
        .and_then(kind_from_apple_product_name)
    {
        return Some(kind);
    }
    let model = probe("sysctl", &["-n", "hw.model"]).await?;
    kind_from_apple_product_name(&model)
}

/// The value of `"product-name" = <"…">` in `ioreg`'s output.
fn ioreg_product_name(output: &str) -> Option<String> {
    let line = output
        .lines()
        .find(|line| line.trim_start().starts_with("\"product-name\""))?;
    let start = line.find("<\"")? + 2;
    let end = start + line[start..].find("\">")?;
    Some(line[start..end].to_string())
}

async fn detect_linux() -> Option<MachineKind> {
    let read = |path: String| async move {
        tokio::fs::read_to_string(path)
            .await
            .ok()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty())
    };
    // WSL names Microsoft in its kernel release on both WSL 1 and 2. It's checked before DMI
    // because WSL 2 presents as a Hyper-V VM.
    if read(KERNEL_RELEASE_PATH.to_string())
        .await
        .is_some_and(|release| release.to_lowercase().contains("microsoft"))
    {
        return Some(MachineKind::Linux);
    }
    let chassis_type = read(format!("{DMI_ROOT}/chassis_type")).await;
    let sys_vendor = read(format!("{DMI_ROOT}/sys_vendor")).await;
    let product_name = read(format!("{DMI_ROOT}/product_name")).await;
    kind_from_dmi(
        chassis_type.as_deref(),
        sys_vendor.as_deref(),
        product_name.as_deref(),
    )
}

/// A command's trimmed output, or `None` if it fails or takes too long.
async fn probe(program: &str, args: &[&str]) -> Option<String> {
    let output = tokio::time::timeout(
        PROBE_TIMEOUT,
        tokio::process::Command::new(program)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// The icon chosen for this machine, kept in `machine.json` in the data directory.
#[derive(Default, Serialize, Deserialize)]
struct SavedChoice {
    #[serde(default)]
    icon: Option<MachineKind>,
}

pub(crate) fn load_choice(path: &Path) -> Option<MachineKind> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<SavedChoice>(&text).ok()?.icon
}

pub(crate) fn save_choice(path: &Path, icon: Option<MachineKind>) -> Result<()> {
    let text = serde_json::to_string_pretty(&SavedChoice { icon })?;
    std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_names_and_models() {
        let kind = kind_from_apple_product_name;
        assert_eq!(kind("Mac mini (2024)"), Some(MachineKind::MacMini));
        assert_eq!(kind("Macmini8,1"), Some(MachineKind::MacMini));
        assert_eq!(kind("Mac Studio (2023)"), Some(MachineKind::MacStudio));
        assert_eq!(kind("MacBook Pro (14-inch)"), Some(MachineKind::Laptop));
        assert_eq!(kind("MacBookAir10,1"), Some(MachineKind::Laptop));
        assert_eq!(kind("iMac (24-inch)"), Some(MachineKind::Desktop));
        assert_eq!(kind("MacPro7,1"), Some(MachineKind::Desktop));
        assert_eq!(kind("Mac16,10"), None);
    }

    #[test]
    fn dmi_reads_vms_as_cloud_before_chassis() {
        assert_eq!(
            kind_from_dmi(Some("1"), Some("QEMU"), Some("Standard PC")),
            Some(MachineKind::Cloud)
        );
        assert_eq!(
            kind_from_dmi(
                Some("3"),
                Some("Microsoft Corporation"),
                Some("Virtual Machine")
            ),
            Some(MachineKind::Cloud)
        );
        assert_eq!(
            kind_from_dmi(Some("10"), Some("LENOVO"), Some("ThinkPad X1")),
            Some(MachineKind::Laptop)
        );
        assert_eq!(
            kind_from_dmi(Some("23"), Some("Dell Inc."), Some("PowerEdge R640")),
            Some(MachineKind::Server)
        );
        assert_eq!(kind_from_dmi(Some("2"), Some("ASUS"), Some("Board")), None);
        assert_eq!(
            kind_from_dmi(None, Some("Apple Inc."), Some("MacBookPro18,3")),
            Some(MachineKind::Laptop)
        );
    }

    #[test]
    fn ioreg_product_name_is_read() {
        let output = "+-o product  <class IOPlatformDevice>\n  {\n    \"product-name\" = <\"Mac mini (2024)\">\n  }";
        assert_eq!(
            ioreg_product_name(output).as_deref(),
            Some("Mac mini (2024)")
        );
    }

    #[test]
    fn the_choice_is_saved() {
        let directory = tempfile::tempdir().expect("temp dir");
        let path = directory.path().join("machine.json");
        assert_eq!(load_choice(&path), None);
        save_choice(&path, Some(MachineKind::MacStudio)).expect("saved");
        assert_eq!(load_choice(&path), Some(MachineKind::MacStudio));
        save_choice(&path, None).expect("saved");
        assert_eq!(load_choice(&path), None);
    }
}
