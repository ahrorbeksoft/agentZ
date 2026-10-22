//! The copies of a project on several machines as New Thread's machine picker lists them
//! (`design/subthreads-machines/decisions.md`, topics 9 and 10): a line per copy with its
//! machine and, at its end, only what sets it apart from the first copy, in the warning color
//! ("↓2 · 3 files"), or the agent the thread would take there. Hovering a row shows Zed's
//! documentation aside: the folder, how its branch stands against what it tracks, its
//! uncommitted files and lines and its stashes, as the Workspaces view's git popover writes
//! them, and when its machine last fetched. Each machine's server reads its own copy
//! ([`projects::CopyStatus`]) without fetching.

use std::time::SystemTime;

use agentz_protocol::agents::AgentId;
use projects::CopyStatus;
use ui::prelude::*;

use crate::agent_icons::agent_icon;
use crate::sidebar::format_relative_time;

/// A copy of the project, as the picker shows it.
#[derive(Clone)]
pub(crate) struct CopyRow {
    pub(crate) machine_icon: IconName,
    pub(crate) machine: SharedString,
    /// The copy's folder, as its machine would write it.
    pub(crate) folder: SharedString,
    pub(crate) status: Option<CopyStatus>,
    /// Why the new thread can't move there, such as "Offline".
    pub(crate) unusable: Option<SharedString>,
    /// The agent the new thread takes there, by its id and name, when that machine lacks the
    /// thread's own.
    pub(crate) uses_agent: Option<(AgentId, SharedString)>,
    pub(crate) is_current: bool,
}

fn branch_name(status: &CopyStatus) -> String {
    match (&status.branch, &status.commit) {
        (Some(branch), _) => branch.clone(),
        (None, Some(commit)) => format!("detached at {commit}"),
        (None, None) => "detached".to_string(),
    }
}

fn count(count: u32, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// What sets the copy apart from `first`, short: "payments · ↓2 · 3 files". Nothing for a copy
/// that stands as `first` does.
fn marks(status: &CopyStatus, first: &CopyStatus) -> Option<String> {
    let mut marks = Vec::new();
    if status.branch != first.branch || status.commit.is_none() != first.commit.is_none() {
        marks.push(branch_name(status));
    }
    if status.upstream.is_none() && first.upstream.is_some() {
        marks.push("no upstream".to_string());
    }
    if status.ahead > 0 && status.ahead != first.ahead {
        marks.push(format!("↑{}", status.ahead));
    }
    if status.behind > 0 && status.behind != first.behind {
        marks.push(format!("↓{}", status.behind));
    }
    if status.changed_files > 0 && status.changed_files != first.changed_files {
        marks.push(count(status.changed_files, "file", "files"));
    }
    if status.stashes > 0 && status.stashes != first.stashes {
        marks.push(count(status.stashes, "stash", "stashes"));
    }
    (!marks.is_empty()).then(|| marks.join(" · "))
}

/// The aside's lines about git: the branch and its upstream with ↑ and ↓, the changes, and the
/// stashes, each part with its color.
fn git_lines(status: &CopyStatus) -> Vec<(IconName, Vec<(String, Color)>)> {
    let mut branch = vec![(branch_name(status), Color::Default)];
    match &status.upstream {
        Some(upstream) => branch.push((format!(" → {upstream}"), Color::Muted)),
        None => branch.push((" · no upstream".to_string(), Color::Muted)),
    }
    if status.ahead > 0 {
        branch.push((format!(" ↑{}", status.ahead), Color::Created));
    }
    if status.behind > 0 {
        branch.push((format!(" ↓{}", status.behind), Color::Deleted));
    }
    if status.upstream.is_some() && status.ahead == 0 && status.behind == 0 {
        branch.push((" up to date".to_string(), Color::Muted));
    }
    let changes = if status.changed_files == 0 {
        vec![("No changes".to_string(), Color::Muted)]
    } else {
        vec![
            (count(status.changed_files, "file", "files"), Color::Default),
            (format!(" +{}", status.added_lines), Color::Created),
            (format!(" −{}", status.removed_lines), Color::Deleted),
        ]
    };
    let mut lines = vec![(IconName::GitBranch, branch), (IconName::Diff, changes)];
    if status.stashes > 0 {
        lines.push((
            IconName::Archive,
            vec![(count(status.stashes, "stash", "stashes"), Color::Warning)],
        ));
    }
    lines
}

/// "Fetched 9m ago", as of the copy's machine's last fetch.
fn fetched(status: &CopyStatus, now: SystemTime) -> String {
    match status.fetched_at {
        Some(fetched_at) => match format_relative_time(fetched_at, now).as_str() {
            "now" => "Fetched just now".to_string(),
            ago => format!("Fetched {ago} ago"),
        },
        None => "Never fetched".to_string(),
    }
}

/// The copy's row in the menu: its machine, then what sets it apart, the agent it would take,
/// or why it can't be picked, and a check on the copy the thread is in now.
pub(crate) fn render_copy_row(row: &CopyRow, first: Option<&CopyStatus>, cx: &App) -> AnyElement {
    let end = if let Some(reason) = &row.unusable {
        Label::new(reason.clone())
            .size(LabelSize::XSmall)
            .color(Color::Disabled)
            .into_any_element()
    } else if let Some((agent_id, name)) = &row.uses_agent {
        h_flex()
            .gap_1()
            .child(
                agent_icon(agent_id, cx)
                    .map(Icon::from_svg_markup)
                    .unwrap_or_else(|| Icon::new(IconName::Sparkle))
                    .size(IconSize::XSmall)
                    .color(Color::Muted),
            )
            .child(
                Label::new(format!("Uses {name}"))
                    .size(LabelSize::XSmall)
                    .color(Color::Muted),
            )
            .into_any_element()
    } else {
        let marks = row
            .status
            .as_ref()
            .zip(first)
            .and_then(|(status, first)| marks(status, first));
        match marks {
            Some(marks) => Label::new(marks)
                .size(LabelSize::XSmall)
                .color(Color::Warning)
                .into_any_element(),
            None => div().into_any_element(),
        }
    };
    let name_color = if row.unusable.is_none() {
        Color::Default
    } else {
        Color::Disabled
    };
    let machine = row.machine.clone();
    h_flex()
        .debug_selector(move || format!("new-thread-copy-{machine}"))
        .w_full()
        .min_w(px(240.))
        .gap_2()
        .child(
            Icon::new(row.machine_icon)
                .size(IconSize::Small)
                .color(Color::Muted),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(Label::new(row.machine.clone()).color(name_color).truncate()),
        )
        .child(div().flex_none().child(end))
        .child(div().flex_none().w(px(14.)).when(row.is_current, |slot| {
            slot.child(
                Icon::new(IconName::Check)
                    .size(IconSize::Small)
                    .color(Color::Accent),
            )
        }))
        .into_any_element()
}

/// Zed's aside beside a copy's row: its machine and folder, how it stands in git, and when its
/// machine last fetched.
pub(crate) fn render_copy_aside(row: &CopyRow, status: &CopyStatus, now: SystemTime) -> AnyElement {
    v_flex()
        .max_w(px(300.))
        .gap_1()
        .child(
            h_flex()
                .gap_1p5()
                .child(Label::new(row.machine.clone()).size(LabelSize::Small))
                .child(
                    Label::new(row.folder.clone())
                        .size(LabelSize::Small)
                        .color(Color::Placeholder)
                        .truncate(),
                ),
        )
        .children(git_lines(status).into_iter().map(|(icon, parts)| {
            h_flex()
                .gap_1()
                .child(Icon::new(icon).size(IconSize::XSmall).color(Color::Muted))
                .child(
                    h_flex().children(
                        parts.into_iter().map(|(text, color)| {
                            Label::new(text).size(LabelSize::Small).color(color)
                        }),
                    ),
                )
        }))
        .child(
            Label::new(fetched(status, now))
                .size(LabelSize::Small)
                .color(Color::Placeholder),
        )
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

    fn text(parts: &[(String, Color)]) -> String {
        parts.iter().map(|(text, _)| text.as_str()).collect()
    }

    #[test]
    fn a_copy_is_marked_only_with_what_sets_it_apart() {
        let mac = copy("main", Some("origin/main"), 0, 0);
        assert_eq!(marks(&mac, &mac), None);

        let mut agents = copy("main", Some("origin/main"), 0, 2);
        agents.changed_files = 3;
        assert_eq!(marks(&agents, &mac).as_deref(), Some("↓2 · 3 files"));

        let mut devbox = copy("payments", None, 3, 0);
        devbox.stashes = 1;
        assert_eq!(
            marks(&devbox, &mac).as_deref(),
            Some("payments · no upstream · ↑3 · 1 stash")
        );
    }

    #[test]
    fn the_aside_reads_as_the_workspaces_popover_writes_it() {
        let now = SystemTime::now();
        let mut agents = copy("main", Some("origin/main"), 0, 2);
        agents.changed_files = 1;
        agents.added_lines = 8;
        agents.removed_lines = 2;
        agents.fetched_at = Some(now - Duration::from_secs(9 * 60));
        let lines: Vec<String> = git_lines(&agents)
            .iter()
            .map(|(_, parts)| text(parts))
            .collect();
        assert_eq!(lines, ["main → origin/main ↓2", "1 file +8 −2"]);
        assert_eq!(fetched(&agents, now), "Fetched 9m ago");

        let mac = copy("main", Some("origin/main"), 0, 0);
        let lines: Vec<String> = git_lines(&mac)
            .iter()
            .map(|(_, parts)| text(parts))
            .collect();
        assert_eq!(lines, ["main → origin/main up to date", "No changes"]);
        assert_eq!(fetched(&mac, now), "Never fetched");
    }
}
