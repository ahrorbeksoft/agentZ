//! Words and marks for cloud providers' machines (design/cloud), wherever they show, and the
//! question asked before one starts charging.

use std::rc::Rc;
use std::time::SystemTime;

use agentz_protocol::cloud::{
    Change, CloudMachine, CloudState, CloudUsage, Lifecycle, MachineSize, Provider, ProviderState,
    ProviderStatus,
};
use gpui::{App, Window};
use ui::{Color, IconName};

use crate::app_settings::AppSettingsStore;
use crate::confirm_dialog::{ConfirmCheckbox, ConfirmRequest};
use crate::machines::Machines;

/// A state as its badge says it.
pub fn state_label(state: &CloudState) -> &str {
    match state {
        CloudState::Creating => "Creating",
        CloudState::Starting => "Starting",
        CloudState::Running => "Running",
        CloudState::Stopping => "Stopping",
        CloudState::Stopped => "Stopped",
        CloudState::Pausing => "Pausing",
        CloudState::Paused => "Paused",
        CloudState::Resuming => "Resuming",
        CloudState::Deleting => "Deleting",
        CloudState::Suspended => "Suspended",
        CloudState::Failed => "Failed",
        CloudState::Other(state) => state,
    }
}

/// The badge's color: running is good, on its way is the accent's, failed is an error.
pub fn state_color(state: &CloudState) -> Color {
    match state {
        CloudState::Running => Color::Success,
        CloudState::Creating
        | CloudState::Starting
        | CloudState::Resuming
        | CloudState::Stopping
        | CloudState::Pausing
        | CloudState::Deleting => Color::Accent,
        CloudState::Paused => Color::Warning,
        CloudState::Suspended | CloudState::Failed => Color::Error,
        CloudState::Stopped | CloudState::Other(_) => Color::Muted,
    }
}

/// What a change is called on its button or in its menu.
pub fn lifecycle_label(change: Lifecycle) -> &'static str {
    match change {
        Lifecycle::Start => "Start",
        Lifecycle::Stop => "Stop",
        Lifecycle::Restart => "Restart",
        Lifecycle::Pause => "Pause",
        Lifecycle::Resume => "Resume",
        Lifecycle::Delete => "Delete",
    }
}

/// A change under way, in its machine's badge.
pub fn change_label(change: Change) -> &'static str {
    match change {
        Change::Create => "Creating",
        Change::Lifecycle(Lifecycle::Start) => "Starting",
        Change::Lifecycle(Lifecycle::Stop) => "Stopping",
        Change::Lifecycle(Lifecycle::Restart) => "Restarting",
        Change::Lifecycle(Lifecycle::Pause) => "Pausing",
        Change::Lifecycle(Lifecycle::Resume) => "Resuming",
        Change::Lifecycle(Lifecycle::Delete) => "Deleting",
    }
}

/// What a machine's row offers in its button: the change it most likely needs, or adding
/// credit to a provider that suspended it (design/cloud topic 6 B).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MainAction {
    Change(Lifecycle),
    AddCredit(&'static str),
}

pub fn main_action(provider: Provider, state: &CloudState) -> Option<MainAction> {
    let description = provider.description();
    let offers = |change: Lifecycle| {
        description
            .lifecycle
            .contains(&change)
            .then_some(MainAction::Change(change))
    };
    match state {
        // Pausing keeps what's running and costs less to come back from.
        CloudState::Running => offers(Lifecycle::Pause).or_else(|| offers(Lifecycle::Stop)),
        CloudState::Paused => offers(Lifecycle::Resume),
        CloudState::Stopped => offers(Lifecycle::Start),
        CloudState::Suspended => description.add_credit.map(MainAction::AddCredit),
        _ => None,
    }
}

/// The changes a machine's menu offers besides its button's.
pub fn other_changes(provider: Provider, state: &CloudState) -> Vec<Lifecycle> {
    let candidates: &[Lifecycle] = match state {
        CloudState::Running => &[Lifecycle::Pause, Lifecycle::Stop, Lifecycle::Restart],
        CloudState::Paused => &[Lifecycle::Stop],
        _ => &[],
    };
    let main = main_action(provider, state);
    candidates
        .iter()
        .copied()
        .filter(|change| provider.description().lifecycle.contains(change))
        .filter(|change| main != Some(MainAction::Change(*change)))
        .collect()
}

/// Whether the change starts the provider's billing, which is asked about first (topic 5). A
/// restart doesn't: the machine was already running.
pub fn charges(provider: Provider, change: Lifecycle) -> Option<&'static str> {
    provider
        .description()
        .charge
        .filter(|_| change == Lifecycle::Start)
}

/// What a machine that charges costs, as starting or making one asks: "It costs $20 per 730
/// running hours, billed by the second from your Cube balance ($14.20 left). Stopping it stops
/// the charge."
pub fn charge_message(charge: &str, usage: Option<&CloudUsage>) -> String {
    let balance = usage
        .and_then(balance_left)
        .map(|left| format!(" ({left})"))
        .unwrap_or_default();
    format!("It costs {charge}{balance}. Stopping it stops the charge.")
}

/// The account the provider's CLI is signed in to.
pub fn signed_in_account(state: Option<&ProviderState>) -> Option<String> {
    match &state?.status {
        ProviderStatus::SignedIn { account, .. } => Some(account.clone()),
        _ => None,
    }
}

/// The question asked before something starts the provider's billing (topic 5 C): what it
/// costs, the balance left, and "Don't ask again for this account". `None` once the account
/// said not to ask.
pub fn charge_question(
    provider: Provider,
    charge: &str,
    title: String,
    confirm_label: &'static str,
    on_confirm: Rc<dyn Fn(&mut Window, &mut App)>,
    cx: &App,
) -> Option<ConfirmRequest> {
    let local = Machines::local(cx);
    let state = local.read(cx).cloud().provider(provider);
    let account = signed_in_account(state);
    let app_settings = AppSettingsStore::global(cx);
    if account.as_ref().is_some_and(|account| {
        app_settings
            .read(cx)
            .settings()
            .charges_without_asking(provider, account)
    }) {
        return None;
    }
    let checkbox = account.map(|account| ConfirmCheckbox {
        label: "Don’t ask again for this account".into(),
        type_to_confirm: None,
        on_confirm_ticked: Rc::new(move |cx| {
            app_settings.update(cx, |store, cx| {
                store.update(
                    |settings| settings.set_asks_before_charging(provider, &account, false),
                    cx,
                )
            })
        }),
    });
    Some(ConfirmRequest {
        icon: IconName::Coins,
        title: title.into(),
        message: charge_message(charge, state.and_then(|state| state.usage.as_ref())).into(),
        confirm_label: confirm_label.into(),
        on_confirm,
        checkbox,
        is_charge: true,
    })
}

/// What the provider calls its machines, as a title ("Computer").
pub fn noun_title(provider: Provider) -> String {
    let mut chars = provider.description().noun.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// What's left of a prepaid balance ("$14.20 left").
pub fn balance_left(usage: &CloudUsage) -> Option<String> {
    match usage {
        CloudUsage::Balance {
            balance_micro_usd: Some(balance),
            ..
        } => Some(format!("{} left", dollars(*balance))),
        _ => None,
    }
}

pub fn provider_icon(provider: Provider) -> IconName {
    match provider {
        Provider::Instabox => IconName::CloudInstabox,
        Provider::Proton => IconName::CloudProton,
    }
}

/// What a machine is: its template and size ("universal · 4 vCPUs · 8 GB · 20 GB disk").
pub fn machine_summary(machine: &CloudMachine) -> String {
    [machine.template.clone(), Some(size_summary(&machine.size))]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

/// A machine's size ("4 vCPUs · 8 GB · 20 GB disk").
pub fn size_summary(size: &MachineSize) -> String {
    let memory = match (size.memory_mib, size.memory_ceiling_mib) {
        (Some(floor), Some(ceiling)) if ceiling > floor => {
            Some(format!("{}–{} GB", gib(floor), gib(ceiling)))
        }
        (Some(memory), _) => Some(format!("{} GB", gib(memory))),
        (None, Some(ceiling)) => Some(format!("up to {} GB", gib(ceiling))),
        (None, None) => None,
    };
    [
        size.vcpus.map(|vcpus| {
            if vcpus == 1 {
                "1 vCPU".to_string()
            } else {
                format!("{vcpus} vCPUs")
            }
        }),
        memory,
        size.disk_gib.map(|disk| format!("{disk} GB disk")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ")
}

fn gib(mib: u64) -> String {
    let gib = mib as f64 / 1024.;
    if gib.fract() == 0. {
        format!("{gib:.0}")
    } else {
        format!("{gib:.1}")
    }
}

/// An account's usage in a line: Proton's balance and hours, or Instabox's counts against its
/// limits.
pub fn usage_summary(usage: &CloudUsage) -> String {
    match usage {
        CloudUsage::Balance {
            balance_micro_usd,
            running_hours,
            ..
        } => {
            let balance = match balance_micro_usd {
                Some(balance) => format!("{} left", dollars(*balance)),
                None => "No prepaid balance".to_string(),
            };
            format!("{balance} · {running_hours:.0} h running")
        }
        CloudUsage::Limits {
            machines,
            max_machines,
            snapshots,
            max_snapshots,
            stored_bytes,
            max_stored_gib,
        } => {
            let of = |count: u32, max: Option<u32>, noun: &str| match max {
                Some(max) => format!("{count} of {max} {noun}"),
                None => format!("{count} {noun}"),
            };
            let storage = stored_bytes.map(|bytes| {
                let gb = bytes as f64 / 1e9;
                match max_stored_gib {
                    Some(max) => format!("{gb:.0} of {max} GB"),
                    None => format!("{gb:.0} GB"),
                }
            });
            [
                Some(of(*machines, *max_machines, "machines")),
                Some(of(*snapshots, *max_snapshots, "snapshots")),
                storage,
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ")
        }
        CloudUsage::NotBilled => "Billing isn't on".to_string(),
    }
}

/// How long something ran ("31 h 12 min", "4 min").
pub fn duration_label(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = seconds % 3600 / 60;
    match hours {
        0 => format!("{minutes} min"),
        hours => format!("{hours} h {minutes} min"),
    }
}

/// A size in Instabox's gigabytes, of 10⁹ bytes ("3.1 GB").
pub fn gigabytes(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

/// Memory used of a machine's, in the gigabytes of 2³⁰ bytes its size is given in ("2.1 of
/// 8 GB").
pub fn memory_label(used_bytes: u64, total_bytes: u64) -> String {
    let used = used_bytes as f64 / f64::from(1u32 << 30);
    format!("{used:.1} of {} GB", gib(total_bytes >> 20))
}

/// When something happened or happens: the time today, or the day and time ("Oct 19, 14:02").
pub fn time_label(time: SystemTime, now: SystemTime) -> String {
    let time = chrono::DateTime::<chrono::Local>::from(time);
    let now = chrono::DateTime::<chrono::Local>::from(now);
    if time.date_naive() == now.date_naive() {
        time.format("%H:%M").to_string()
    } else {
        time.format("%b %-d, %H:%M").to_string()
    }
}

pub fn dollars(micro_usd: i64) -> String {
    let cents = (micro_usd as f64 / 10_000.).round() as i64;
    let sign = if cents < 0 { "-" } else { "" };
    let cents = cents.abs();
    format!("{sign}${}.{:02}", cents / 100, cents % 100)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_a_machine_and_its_account() {
        let machine = CloudMachine {
            id: "c-1".into(),
            name: "devbox-3".into(),
            state: CloudState::Running,
            size: MachineSize {
                vcpus: Some(8),
                memory_mib: Some(2048),
                memory_ceiling_mib: Some(65536),
                disk_gib: Some(50),
            },
            template: None,
            ssh_address: None,
            created_at: None,
            error: None,
        };
        assert_eq!(machine_summary(&machine), "8 vCPUs · 2–64 GB · 50 GB disk");
        assert_eq!(duration_label(31 * 3600 + 12 * 60 + 5), "31 h 12 min");
        assert_eq!(duration_label(240), "4 min");
        assert_eq!(gigabytes(3_140_000_000), "3.1 GB");
        assert_eq!(memory_label(2_254_857_830, 8 << 30), "2.1 of 8 GB");
        assert_eq!(
            usage_summary(&CloudUsage::Balance {
                balance_micro_usd: Some(24_270_000),
                running_hours: 26.78,
                charged_micro_usd: 733_600,
            }),
            "$24.27 left · 27 h running"
        );
        assert_eq!(
            usage_summary(&CloudUsage::Limits {
                machines: 2,
                max_machines: Some(5),
                snapshots: 1,
                max_snapshots: Some(10),
                stored_bytes: Some(14_000_000_000),
                max_stored_gib: Some(200),
            }),
            "2 of 5 machines · 1 of 10 snapshots · 14 of 200 GB"
        );
    }

    #[test]
    fn offers_the_change_a_machine_most_likely_needs() {
        // Instabox pauses what's running; Proton, which can't, stops it.
        assert_eq!(
            main_action(Provider::Instabox, &CloudState::Running),
            Some(MainAction::Change(Lifecycle::Pause))
        );
        assert_eq!(
            other_changes(Provider::Instabox, &CloudState::Running),
            [Lifecycle::Stop]
        );
        assert_eq!(
            main_action(Provider::Proton, &CloudState::Running),
            Some(MainAction::Change(Lifecycle::Stop))
        );
        assert_eq!(
            other_changes(Provider::Proton, &CloudState::Running),
            [Lifecycle::Restart]
        );
        assert_eq!(
            main_action(Provider::Instabox, &CloudState::Paused),
            Some(MainAction::Change(Lifecycle::Resume))
        );
        assert_eq!(
            other_changes(Provider::Instabox, &CloudState::Paused),
            [Lifecycle::Stop]
        );
        assert_eq!(main_action(Provider::Proton, &CloudState::Starting), None);

        // A suspended Proton computer needs credit; starting one is what charges.
        assert_eq!(
            main_action(Provider::Proton, &CloudState::Suspended),
            Some(MainAction::AddCredit("https://app.cube.computer/billing"))
        );
        assert!(charges(Provider::Proton, Lifecycle::Start).is_some());
        assert!(charges(Provider::Proton, Lifecycle::Restart).is_none());
        assert!(charges(Provider::Instabox, Lifecycle::Start).is_none());
        assert_eq!(
            charge_message(
                "$20 per 730 running hours",
                Some(&CloudUsage::Balance {
                    balance_micro_usd: Some(14_200_000),
                    running_hours: 1.,
                    charged_micro_usd: 0,
                })
            ),
            "It costs $20 per 730 running hours ($14.20 left). Stopping it stops the charge."
        );
        assert_eq!(noun_title(Provider::Proton), "Computer");
    }
}
