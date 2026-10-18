//! Zed's developer diagnostics, from GPUI's profiler: the frame-time overlay
//! (zed-industries/zed#62749), the input-latency report, and the hang monitor
//! (zed-industries/zed#64944). Each one is off until asked for, and what they record stays
//! on this machine.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use gpui::profiler::hang::{HangMonitorConfig, SerializedHangIncident};
use gpui::{App, AppContext as _, Global, InputLatencySnapshot, TaskExt as _, Window, actions};
use hdrhistogram::Histogram;
use util::ResultExt as _;

actions!(
    dev,
    [
        /// Cycles the debug frame-time overlay between hidden, current
        /// frame-time, and detailed frame-time statistics.
        ToggleFpsOverlay,
        /// Resets the debug frame-time overlay's statistics, except for the
        /// total frame count.
        ResetFrameOverlayStats,
        /// Opens a report of the input-to-frame latency histogram for the current window.
        DumpInputLatencyHistogram,
    ]
);

/// Set, it starts GPUI's hang monitor, which appends each hang to `paths::hang_log_file`.
const HANG_MONITOR_ENV_VAR: &str = "AGENTZ_HANG_MONITOR";

const MAX_SERIALIZED_CONTRIBUTORS: usize = 8;

pub fn init(cx: &mut App) {
    if std::env::var_os(HANG_MONITOR_ENV_VAR).is_some() {
        start_hang_monitor(paths::hang_log_file(), cx);
    }
}

/// Zed's thresholds (`hang_telemetry`).
fn hang_monitor_config() -> HangMonitorConfig {
    let (threshold, frame_budget) = if cfg!(debug_assertions) {
        // Unoptimized builds routinely spend more than a release frame budget on ordinary
        // frames.
        (Duration::from_secs(5), Duration::from_millis(100))
    } else {
        (Duration::from_millis(100), Duration::from_millis(24))
    };
    HangMonitorConfig {
        threshold,
        frame_budget,
        interval: Duration::from_secs(1),
    }
}

fn start_hang_monitor(log_file: PathBuf, cx: &mut App) {
    let startup = Instant::now();
    let started = cx.start_hang_monitor(hang_monitor_config(), {
        let log_file = log_file.clone();
        move |poll| {
            let incidents = poll
                .incidents
                .iter()
                .map(|incident| {
                    SerializedHangIncident::convert(
                        startup,
                        incident,
                        MAX_SERIALIZED_CONTRIBUTORS,
                        poll.first_present_at,
                    )
                })
                .collect::<Vec<_>>();
            append_hang_incidents(&log_file, &incidents, chrono::Local::now()).log_err();
        }
    });
    match started {
        Ok(()) => log::info!("logging hangs to {}", log_file.display()),
        Err(error) => log::error!("failed to start the hang monitor: {error}"),
    }
}

/// One JSON line each: when it was logged, and the incident as Zed's telemetry sends it.
fn append_hang_incidents(
    path: &Path,
    incidents: &[SerializedHangIncident],
    logged_at: chrono::DateTime<chrono::Local>,
) -> Result<()> {
    if incidents.is_empty() {
        return Ok(());
    }
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    for incident in incidents {
        log::warn!(
            "the main thread hung for {:.0} ms ({:?})",
            incident.stall_ms,
            incident.trigger
        );
        let line = serde_json::json!({
            "logged_at": logged_at.to_rfc3339(),
            "incident": incident,
        });
        writeln!(file, "{line}")?;
    }
    Ok(())
}

/// Zed's `DumpInputLatencyHistogram`, which writes the report to a file and opens it, where
/// Zed opens a buffer.
pub fn dump_input_latency_histogram(window: &Window, cx: &mut App) {
    let data = snapshot_input_latency_report(window, cx);
    let path = paths::input_latency_report_file();
    cx.spawn(async move |cx| {
        let written = path.clone();
        cx.background_spawn(async move {
            if let Some(folder) = written.parent() {
                fs::create_dir_all(folder)?;
            }
            fs::write(&written, format_report(&data))?;
            anyhow::Ok(())
        })
        .await?;
        cx.update(|cx| cx.open_with_system(&path));
        anyhow::Ok(())
    })
    .detach_and_log_err(cx);
}

/// The data needed to format an input-latency report, captured synchronously by
/// [`snapshot_input_latency_report`]. Formatting it can happen on a background thread.
struct InputLatencyReportData {
    snapshot: InputLatencySnapshot,
    previous_snapshot: Option<InputLatencySnapshot>,
    previous_timestamp: Option<chrono::DateTime<chrono::Local>>,
    timestamp: chrono::DateTime<chrono::Local>,
}

/// Captures the input-to-frame latency histogram for the given window, along with the
/// previous report's baseline for delta computation. Advances the baseline to this snapshot.
fn snapshot_input_latency_report(window: &Window, cx: &mut App) -> InputLatencyReportData {
    let snapshot = window.input_latency_snapshot();
    let timestamp = chrono::Local::now();
    let state = cx.default_global::<ReporterState>();

    let data = InputLatencyReportData {
        snapshot: snapshot.clone(),
        previous_snapshot: state.previous_snapshot.take(),
        previous_timestamp: state.previous_timestamp.take(),
        timestamp,
    };

    state.previous_snapshot = Some(snapshot);
    state.previous_timestamp = Some(timestamp);

    data
}

#[derive(Default)]
struct ReporterState {
    previous_snapshot: Option<InputLatencySnapshot>,
    previous_timestamp: Option<chrono::DateTime<chrono::Local>>,
}

impl Global for ReporterState {}

fn format_report(data: &InputLatencyReportData) -> String {
    let snapshot = &data.snapshot;
    let histogram = &snapshot.latency_histogram;
    let total = histogram.len();

    if total == 0 {
        return "No input latency samples recorded yet.\n\nTry typing or clicking in the window first."
            .to_string();
    }

    let percentiles: &[(&str, f64)] = &[
        ("min  ", 0.0),
        ("p50  ", 0.50),
        ("p75  ", 0.75),
        ("p90  ", 0.90),
        ("p95  ", 0.95),
        ("p99  ", 0.99),
        ("p99.9", 0.999),
        ("max  ", 1.0),
    ];

    let now = data.timestamp;

    let mut report = String::new();
    report.push_str("Input Latency Histogram\n");
    report.push_str("=======================\n");

    let timestamp = now.format("%Y-%m-%d %H:%M:%S %Z");
    report.push_str(&format!("Timestamp: {timestamp}\n"));
    report.push_str(&format!("Samples: {total}\n"));
    if snapshot.mid_draw_events_dropped > 0 {
        report.push_str(&format!(
            "Mid-draw events excluded: {}\n",
            snapshot.mid_draw_events_dropped
        ));
    }

    write_latency_percentiles(&mut report, "Percentiles", histogram, percentiles);
    write_latency_distribution(&mut report, "Distribution", histogram);

    let coalesce = &snapshot.events_per_frame_histogram;
    let coalesce_total = coalesce.len();
    if coalesce_total > 0 {
        report.push('\n');
        report.push_str("Events coalesced per frame:\n");
        for (label, quantile) in percentiles {
            let value = if *quantile == 0.0 {
                coalesce.min()
            } else if *quantile == 1.0 {
                coalesce.max()
            } else {
                coalesce.value_at_quantile(*quantile)
            };
            report.push_str(&format!("  {label}: {value:>6} events\n"));
        }

        report.push('\n');
        report.push_str("Distribution:\n");
        let bar_width = 30usize;
        let max_count = coalesce.max();
        for n in 1..=max_count {
            let count = coalesce
                .iter_recorded()
                .filter(|value| value.value_iterated_to() == n)
                .map(|value| value.count_at_value())
                .sum::<u64>();
            if count == 0 {
                continue;
            }
            let fraction = count as f64 / coalesce_total as f64;
            let bar_len = (fraction * bar_width as f64) as usize;
            let bar = "\u{2588}".repeat(bar_len);
            report.push_str(&format!(
                "  {n:>6} events: {count:>6} ({:>5.1}%) {bar}\n",
                fraction * 100.0,
            ));
        }
    }

    // Delta section: compare against the previous report's snapshot.
    if let (Some(prev_snapshot), Some(prev_timestamp)) =
        (&data.previous_snapshot, &data.previous_timestamp)
    {
        let prev_latency = &prev_snapshot.latency_histogram;
        let prev_total = prev_latency.len();
        let delta_total = total - prev_total;

        report.push('\n');
        report.push_str("Delta Since Last Report\n");
        report.push_str("-----------------------\n");
        let prev_ts = prev_timestamp.format("%Y-%m-%d %H:%M:%S %Z");
        let elapsed_secs = (now - *prev_timestamp).num_seconds().max(0);
        report.push_str(&format!(
            "Previous report: {prev_ts} ({elapsed_secs}s ago)\n"
        ));
        report.push_str(&format!("New samples: {delta_total}\n"));

        if delta_total > 0 {
            let mut delta_histogram = histogram.clone();
            delta_histogram.subtract(prev_latency).ok();

            write_latency_percentiles(
                &mut report,
                "Percentiles (new samples only)",
                &delta_histogram,
                percentiles,
            );
            write_latency_distribution(
                &mut report,
                "Distribution (new samples only)",
                &delta_histogram,
            );
        }
    }

    report
}

fn write_latency_percentiles(
    report: &mut String,
    heading: &str,
    histogram: &Histogram<u64>,
    percentiles: &[(&str, f64)],
) {
    let ns_to_ms = |ns: u64| ns as f64 / 1_000_000.0;

    report.push('\n');
    report.push_str(heading);
    report.push_str(":\n");
    for (label, quantile) in percentiles {
        let value_ns = if *quantile == 0.0 {
            histogram.min()
        } else if *quantile == 1.0 {
            histogram.max()
        } else {
            histogram.value_at_quantile(*quantile)
        };
        let hz = if value_ns > 0 {
            1_000_000_000.0 / value_ns as f64
        } else {
            f64::INFINITY
        };
        report.push_str(&format!(
            "  {label}: {:>8.2}ms  ({:>7.1} Hz)\n",
            ns_to_ms(value_ns),
            hz
        ));
    }
}

fn write_latency_distribution(report: &mut String, heading: &str, histogram: &Histogram<u64>) {
    const BUCKETS: &[(u64, u64, &str, &str)] = &[
        (0, 4_000_000, "0\u{2013}4ms", "(excellent)"),
        (4_000_000, 8_000_000, "4\u{2013}8ms", "(120fps)"),
        (8_000_000, 16_000_000, "8\u{2013}16ms", "(60fps)"),
        (16_000_000, 33_000_000, "16\u{2013}33ms", "(30fps)"),
        (33_000_000, 100_000_000, "33\u{2013}100ms", ""),
        (100_000_000, u64::MAX, "100ms+", "(sluggish)"),
    ];
    let bar_width = 30usize;
    let total = histogram.len() as f64;

    report.push('\n');
    report.push_str(heading);
    report.push_str(":\n");
    for (low, high, range, note) in BUCKETS {
        let count: u64 = histogram
            .iter_recorded()
            .filter(|value| value.value_iterated_to() >= *low && value.value_iterated_to() < *high)
            .map(|value| value.count_at_value())
            .sum();
        let fraction = if total > 0.0 {
            count as f64 / total
        } else {
            0.0
        };
        let bar_len = (fraction * bar_width as f64) as usize;
        let bar = "\u{2588}".repeat(bar_len);
        report.push_str(&format!(
            "  {range:>8}  {note:<11}: {count:>6} ({:>5.1}%) {bar}\n",
            fraction * 100.0,
        ));
    }
}

#[cfg(test)]
mod tests {
    use agentz_protocol::spaces::SpacesSnapshot;
    use gpui::profiler::hang::{HangTrigger, MEASUREMENT_VERSION};
    use gpui::{TestAppContext, VisualTestContext};

    use super::*;
    use crate::machines::MachineId;
    use crate::server_client::ServerClient;
    use crate::shell::Shell;

    fn draws(cx: &mut VisualTestContext) -> u64 {
        cx.update(|window, _| {
            window
                .frame_duration_snapshot()
                .draw_duration_histogram
                .len()
        })
    }

    /// The overlay paints what the window draws anyway, and asks for no frames of its own.
    #[gpui::test]
    fn the_frame_time_overlay_draws_only_when_the_window_does(cx: &mut TestAppContext) {
        cx.update(|cx| {
            crate::init_for_test(cx);
            let client = ServerClient::new_for_test(
                MachineId::Local,
                "This Mac".into(),
                SpacesSnapshot::default(),
                cx,
            );
            crate::machines::init_for_test(vec![client], cx);
            crate::sidebar::init(cx);
        });
        let (shell, cx) = cx.add_window_view(|window, cx| Shell::new(window, cx));
        shell.update_in(cx, |shell, window, cx| {
            window.focus(&gpui::Focusable::focus_handle(shell, cx), cx)
        });
        cx.run_until_parked();
        let mode =
            |cx: &mut VisualTestContext| cx.update(|window, _| window.debug_frame_overlay_mode());
        assert_eq!(mode(cx), gpui::DebugFrameOverlayMode::Hidden);

        let before = draws(cx);
        cx.simulate_keystrokes("ctrl-alt-shift-p");
        cx.run_until_parked();
        assert_eq!(mode(cx), gpui::DebugFrameOverlayMode::Minimal);
        let shown = draws(cx);
        assert!(shown > before, "showing the overlay draws it");

        cx.executor().advance_clock(Duration::from_secs(5));
        cx.run_until_parked();
        assert_eq!(draws(cx), shown, "the overlay asked for more frames");

        cx.simulate_keystrokes("ctrl-alt-shift-p");
        cx.run_until_parked();
        assert_eq!(mode(cx), gpui::DebugFrameOverlayMode::Full);
        cx.simulate_keystrokes("ctrl-alt-shift-o");
        cx.run_until_parked();
        let reset = draws(cx);
        cx.executor().advance_clock(Duration::from_secs(5));
        cx.run_until_parked();
        assert_eq!(
            draws(cx),
            reset,
            "the detailed overlay asked for more frames"
        );

        cx.simulate_keystrokes("ctrl-alt-shift-p");
        cx.run_until_parked();
        assert_eq!(mode(cx), gpui::DebugFrameOverlayMode::Hidden);
    }

    #[test]
    fn the_report_shows_new_samples_since_the_last_one() {
        let snapshot = |latencies_ms: &[u64]| {
            let mut latency_histogram = Histogram::new(3).expect("a valid histogram");
            let mut events_per_frame_histogram = Histogram::new(3).expect("a valid histogram");
            for latency in latencies_ms {
                latency_histogram
                    .record(latency * 1_000_000)
                    .expect("a recordable latency");
                events_per_frame_histogram
                    .record(1)
                    .expect("a recordable count");
            }
            InputLatencySnapshot {
                latency_histogram,
                events_per_frame_histogram,
                mid_draw_events_dropped: 0,
            }
        };
        let timestamp = chrono::Local::now();
        let empty = InputLatencyReportData {
            snapshot: snapshot(&[]),
            previous_snapshot: None,
            previous_timestamp: None,
            timestamp,
        };
        assert!(format_report(&empty).starts_with("No input latency samples recorded yet."));

        let report = format_report(&InputLatencyReportData {
            snapshot: snapshot(&[2, 5, 120]),
            previous_snapshot: Some(snapshot(&[2])),
            previous_timestamp: Some(timestamp - chrono::Duration::seconds(30)),
            timestamp,
        });
        assert!(report.contains("Samples: 3\n"), "{report}");
        assert!(report.contains("Previous report: "), "{report}");
        assert!(report.contains("(30s ago)\nNew samples: 2\n"), "{report}");
        assert!(
            report.contains("Distribution (new samples only):"),
            "{report}"
        );
    }

    #[test]
    fn hangs_are_appended_as_json_lines() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let path = folder.path().join("logs").join("hangs.jsonl");
        let incident = |stall_ms: f64| SerializedHangIncident {
            measurement_version: MEASUREMENT_VERSION,
            phase: "steady",
            trigger: HangTrigger::Threshold,
            start_ms: 10.0,
            active_ms: stall_ms,
            stall_ms,
            dirty_to_present_ms: None,
            sealed_by: "idle",
            busy_fraction: 1.0,
            event_count: 1,
            small_poll_count: 0,
            small_poll_total_ms: 0.0,
            dropped_events: 0,
            journal_discontinuous: false,
            contributors: Vec::new(),
            contributors_elided: 0,
        };
        let logged_at = chrono::Local::now();
        append_hang_incidents(&path, &[], logged_at).expect("nothing to write");
        assert!(!path.exists(), "a poll without hangs made the log");

        append_hang_incidents(&path, &[incident(150.0)], logged_at).expect("written");
        append_hang_incidents(&path, &[incident(300.0), incident(120.0)], logged_at)
            .expect("appended");
        let lines = fs::read_to_string(&path).expect("the log");
        let stalls = lines
            .lines()
            .map(|line| {
                let line: serde_json::Value = serde_json::from_str(line).expect("a JSON line");
                assert_eq!(line["logged_at"], logged_at.to_rfc3339());
                line["incident"]["stall_ms"].as_f64().expect("a stall")
            })
            .collect::<Vec<_>>();
        assert_eq!(stalls, vec![150.0, 300.0, 120.0]);
    }
}
