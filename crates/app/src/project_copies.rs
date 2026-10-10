//! The copies of a project on several machines as New Thread's machine picker lists them
//! (`design/project-copies/decisions.md`): a row per copy with its machine and folder, then how
//! its branch stands against what it tracks, its uncommitted files and lines, and its
//! stashes, as the Workspaces view's git popover writes them. What differs from the first
//! copy is in the warning color. Each machine's server reads its own copy
//! ([`projects::CopyStatus`]) without fetching, so a footer says when each last fetched.

use std::time::SystemTime;

use projects::CopyStatus;
use ui::prelude::*;

use crate::sidebar::format_relative_time;

/// A copy of the project, as the picker shows it.
#[derive(Clone)]
pub(crate) struct CopyRow {
    pub(crate) machine_icon: IconName,
    pub(crate) machine: SharedString,
    /// The copy's folder, as its machine would write it.
    pub(crate) folder: SharedString,
    pub(crate) status: Option<CopyStatus>,
    /// Why the new thread can't move there, such as "offline".
    pub(crate) unusable: Option<SharedString>,
    pub(crate) is_current: bool,
}

/// One kind of thing a copy's line says, compared kind by kind with the first copy's.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Branch,
    Upstream,
    Changes,
    Stash,
}

/// A copy's line: the branch, its upstream with ↑ and ↓, its changes and stashes, each part
/// with its color. A part whose kind says something else in `first` is in the warning color.
fn status_line(status: &CopyStatus, first: Option<&CopyStatus>) -> Vec<(String, Color)> {
    let first_segments = first.map(segments);
    let mut line = Vec::new();
    for (index, (kind, parts)) in segments(status).into_iter().enumerate() {
        let differs = first_segments.as_ref().is_some_and(|first| {
            first
                .iter()
                .find(|(first_kind, _)| *first_kind == kind)
                .map(|(_, first_parts)| text_of(first_parts))
                != Some(text_of(&parts))
        });
        if index > 0 {
            line.push((" · ".to_string(), Color::Muted));
        }
        for (text, color) in parts {
            line.push((text, if differs { Color::Warning } else { color }));
        }
    }
    line
}

fn text_of(parts: &[(String, Color)]) -> String {
    parts.iter().map(|(text, _)| text.as_str()).collect()
}

fn segments(status: &CopyStatus) -> Vec<(Kind, Vec<(String, Color)>)> {
    let branch = match (&status.branch, &status.commit) {
        (Some(branch), _) => branch.clone(),
        (None, Some(commit)) => format!("detached at {commit}"),
        (None, None) => "detached".to_string(),
    };
    let mut upstream = match &status.upstream {
        Some(upstream) => vec![(format!("→ {upstream}"), Color::Muted)],
        None => vec![("no upstream".to_string(), Color::Muted)],
    };
    if status.ahead > 0 {
        upstream.push((format!(" ↑{}", status.ahead), Color::Created));
    }
    if status.behind > 0 {
        upstream.push((format!(" ↓{}", status.behind), Color::Deleted));
    }
    if status.upstream.is_some() && status.ahead == 0 && status.behind == 0 {
        upstream.push((" up to date".to_string(), Color::Muted));
    }
    let changes = if status.changed_files == 0 {
        vec![("no changes".to_string(), Color::Muted)]
    } else {
        let files = if status.changed_files == 1 {
            "file"
        } else {
            "files"
        };
        vec![
            (format!("{} {files}", status.changed_files), Color::Default),
            (format!(" +{}", status.added_lines), Color::Created),
            (format!(" −{}", status.removed_lines), Color::Deleted),
        ]
    };
    let mut segments = vec![
        (Kind::Branch, vec![(branch, Color::Default)]),
        (Kind::Upstream, upstream),
        (Kind::Changes, changes),
    ];
    if status.stashes > 0 {
        let stashes = if status.stashes == 1 {
            "stash"
        } else {
            "stashes"
        };
        segments.push((
            Kind::Stash,
            vec![(format!("{} {stashes}", status.stashes), Color::Warning)],
        ));
    }
    segments
}

/// "As of each machine's last fetch: This Mac 2h ago, Devbox 1 3d ago", for the copies that
/// were read, each machine once.
pub(crate) fn fetched_line(rows: &[CopyRow], now: SystemTime) -> Option<String> {
    let mut machines: Vec<String> = Vec::new();
    let mut seen: Vec<&SharedString> = Vec::new();
    for row in rows {
        let Some(status) = &row.status else {
            continue;
        };
        if seen.contains(&&row.machine) {
            continue;
        }
        seen.push(&row.machine);
        machines.push(match status.fetched_at {
            Some(fetched_at) => match format_relative_time(fetched_at, now).as_str() {
                "now" => format!("{} just now", row.machine),
                ago => format!("{} {ago} ago", row.machine),
            },
            None => format!("{} never", row.machine),
        });
    }
    (!machines.is_empty())
        .then(|| format!("As of each machine's last fetch: {}", machines.join(", ")))
}

/// The copy's row in the menu: its machine and folder, then its line (or why it can't be
/// picked), and a check on the copy the thread is in now.
pub(crate) fn render_copy_row(row: &CopyRow, first: Option<&CopyStatus>) -> AnyElement {
    let is_usable = row.unusable.is_none();
    let detail = match (&row.unusable, &row.status) {
        (Some(reason), _) => h_flex()
            .child(
                Label::new(reason.clone())
                    .size(LabelSize::XSmall)
                    .color(Color::Disabled),
            )
            .into_any_element(),
        (None, Some(status)) => h_flex()
            .min_w_0()
            .overflow_hidden()
            .children(
                status_line(status, first)
                    .into_iter()
                    .map(|(text, color)| Label::new(text).size(LabelSize::XSmall).color(color)),
            )
            .into_any_element(),
        (None, None) => div().into_any_element(),
    };
    let name_color = if is_usable {
        Color::Default
    } else {
        Color::Disabled
    };
    let machine = row.machine.clone();
    h_flex()
        .debug_selector(move || format!("new-thread-copy-{machine}"))
        .w_full()
        .min_w(px(330.))
        .gap_2()
        .py_0p5()
        .child(
            Icon::new(row.machine_icon)
                .size(IconSize::Small)
                .color(Color::Muted),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    h_flex()
                        .min_w_0()
                        .gap_1p5()
                        .child(Label::new(row.machine.clone()).color(name_color))
                        .child(
                            Label::new(row.folder.clone())
                                .size(LabelSize::XSmall)
                                .color(Color::Placeholder)
                                .truncate(),
                        ),
                )
                .child(detail),
        )
        .child(div().flex_none().w(px(14.)).when(row.is_current, |slot| {
            slot.child(
                Icon::new(IconName::Check)
                    .size(IconSize::Small)
                    .color(Color::Accent),
            )
        }))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn copy(branch: &str, upstream: Option<&str>, ahead: u32, behind: u32) -> CopyStatus {
        CopyStatus {
            branch: Some(branch.into()),
            commit: Some("1c2d3e4".into()),
            upstream: upstream.map(str::to_string),
            ahead,
            behind,
            ..CopyStatus::default()
        }
    }

    fn text(line: &[(String, Color)]) -> String {
        line.iter().map(|(text, _)| text.as_str()).collect()
    }

    fn warned(line: &[(String, Color)]) -> String {
        line.iter()
            .filter(|(_, color)| *color == Color::Warning)
            .map(|(text, _)| text.as_str())
            .collect()
    }

    #[test]
    fn a_copy_reads_as_the_workspaces_popover_writes_it() {
        let mac = copy("main", Some("origin/main"), 0, 0);
        assert_eq!(
            text(&status_line(&mac, Some(&mac))),
            "main · → origin/main up to date · no changes"
        );
        assert_eq!(warned(&status_line(&mac, Some(&mac))), "");

        let mut devbox = copy("payments", None, 3, 0);
        devbox.changed_files = 1;
        devbox.added_lines = 8;
        devbox.removed_lines = 2;
        let line = status_line(&devbox, Some(&mac));
        assert_eq!(text(&line), "payments · no upstream ↑3 · 1 file +8 −2");
        // Everything differs from This Mac's copy.
        assert_eq!(warned(&line), "paymentsno upstream ↑31 file +8 −2");

        let mut laptop = copy("main", Some("origin/main"), 0, 12);
        laptop.stashes = 1;
        let line = status_line(&laptop, Some(&mac));
        assert_eq!(
            text(&line),
            "main · → origin/main ↓12 · no changes · 1 stash"
        );
        assert_eq!(warned(&line), "→ origin/main ↓121 stash");
    }

    #[test]
    fn the_footer_says_when_each_machine_fetched() {
        let now = SystemTime::now();
        let row = |machine: &str, fetched: Option<Duration>| CopyRow {
            machine_icon: IconName::Server,
            machine: machine.to_string().into(),
            folder: "~/p".into(),
            status: Some(CopyStatus {
                fetched_at: fetched.map(|ago| now - ago),
                ..CopyStatus::default()
            }),
            unusable: None,
            is_current: false,
        };
        let rows = [
            row("This Mac", Some(Duration::from_secs(2 * 3600))),
            row("Devbox 1", None),
            row("Devbox 1", Some(Duration::from_secs(60))),
        ];
        assert_eq!(
            fetched_line(&rows, now).as_deref(),
            Some("As of each machine's last fetch: This Mac 2h ago, Devbox 1 never")
        );
        assert_eq!(fetched_line(&[], now), None);
    }
}
