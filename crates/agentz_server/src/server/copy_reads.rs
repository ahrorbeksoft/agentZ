//! How the projects the app combines with copies on other machines stand in git, read with
//! the branches and sent with the projects ([`projects::CopyStatus`]), so New Thread's machine
//! picker can say which copy is behind or has work of its own. Nothing is fetched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use projects::{CopyStatus, ProjectId};

use super::Server;
use crate::spaces::{parse_ahead_behind, sum_numstat};

/// Reads every few seconds mustn't take git's index lock from under the user's own commands.
const NO_LOCKS: [(&str, &str); 1] = [("GIT_OPTIONAL_LOCKS", "0")];

impl Server {
    pub(super) fn refresh_copy_statuses(&mut self) {
        if self.reading_copy_statuses {
            return;
        }
        let combined = self.relays.combined_projects();
        let folders: Vec<(ProjectId, PathBuf)> = self
            .projects
            .projects()
            .iter()
            .filter(|project| combined.contains(&project.id))
            .map(|project| (project.id, project.path.clone()))
            .collect();
        if folders.is_empty() {
            self.projects.set_copy_statuses(BTreeMap::new());
            return;
        }
        self.reading_copy_statuses = true;
        let read = async move {
            let mut statuses = BTreeMap::new();
            for (project, folder) in folders {
                if let Some(status) = copy_status(&folder).await {
                    statuses.insert(project, status);
                }
            }
            statuses
        };
        self.spawn_then(read, |server, statuses| {
            server.reading_copy_statuses = false;
            server.projects.set_copy_statuses(statuses);
        });
    }
}

/// The folder's branch, commit, upstream with ahead and behind, uncommitted changes and
/// stashes, or `None` outside git.
pub(crate) async fn copy_status(folder: &Path) -> Option<CopyStatus> {
    let status = crate::git::git(folder, &["status", "--porcelain=v2", "--branch"], &NO_LOCKS)
        .await
        .ok()?;
    let mut copy = parse_status(&status);
    // A branch that tracks nothing is ahead by the commits no remote has.
    if copy.upstream.is_none() && copy.commit.is_some() {
        copy.ahead = crate::git::git(
            folder,
            &["rev-list", "--count", "HEAD", "--not", "--remotes"],
            &NO_LOCKS,
        )
        .await
        .ok()
        .and_then(|count| count.trim().parse().ok())
        .unwrap_or_default();
    }
    (copy.added_lines, copy.removed_lines) =
        crate::git::git(folder, &["diff", "HEAD", "--numstat"], &NO_LOCKS)
            .await
            .map(|numstat| sum_numstat(&numstat))
            .unwrap_or_default();
    copy.stashes = crate::git::git(folder, &["stash", "list"], &NO_LOCKS)
        .await
        .map(|stashes| stashes.lines().count() as u32)
        .unwrap_or_default();
    copy.fetched_at = crate::git::git(folder, &["rev-parse", "--git-path", "FETCH_HEAD"], &[])
        .await
        .ok()
        .and_then(|path| {
            std::fs::metadata(folder.join(path.trim()))
                .and_then(|metadata| metadata.modified())
                .ok()
        });
    Some(copy)
}

/// `git status --porcelain=v2 --branch`: its `# branch.` headers, and an entry per changed or
/// new file.
fn parse_status(status: &str) -> CopyStatus {
    let mut copy = CopyStatus::default();
    for line in status.lines() {
        let Some(header) = line.strip_prefix("# branch.") else {
            if !line.starts_with('#') && !line.is_empty() {
                copy.changed_files += 1;
            }
            continue;
        };
        let (key, value) = header.split_once(' ').unwrap_or((header, ""));
        match key {
            "oid" if value != "(initial)" => {
                copy.commit = Some(value.chars().take(7).collect());
            }
            "head" if value != "(detached)" => copy.branch = Some(value.to_string()),
            "upstream" => copy.upstream = Some(value.to_string()),
            "ab" => {
                let counts = value.replace(['+', '-'], "");
                (copy.ahead, copy.behind) = parse_ahead_behind(&counts).unwrap_or_default();
            }
            _ => {}
        }
    }
    copy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_headers_and_entries_are_read() {
        let status = "# branch.oid 77ab12cdef0123456789\n# branch.head payments\n\
                      # branch.upstream origin/payments\n# branch.ab +3 -1\n\
                      1 .M N... 100644 100644 100644 aaa bbb src/main.rs\n\
                      ? notes.md\n";
        assert_eq!(
            parse_status(status),
            CopyStatus {
                branch: Some("payments".into()),
                commit: Some("77ab12c".into()),
                upstream: Some("origin/payments".into()),
                ahead: 3,
                behind: 1,
                changed_files: 2,
                ..CopyStatus::default()
            }
        );
        let detached = parse_status("# branch.oid (initial)\n# branch.head (detached)\n");
        assert_eq!(detached, CopyStatus::default());
    }

    #[tokio::test]
    async fn a_checkout_reports_its_changes_and_stashes() {
        let dir = tempfile::tempdir().expect("temporary folder");
        let folder = dir.path();
        let identity = [
            ("GIT_AUTHOR_NAME", "t"),
            ("GIT_AUTHOR_EMAIL", "t@example.com"),
            ("GIT_COMMITTER_NAME", "t"),
            ("GIT_COMMITTER_EMAIL", "t@example.com"),
        ];
        crate::git::git(folder, &["init", "-q", "-b", "main"], &identity)
            .await
            .expect("init");
        std::fs::write(folder.join("a.txt"), "one\n").expect("write");
        crate::git::git(folder, &["add", "-A"], &identity)
            .await
            .expect("add");
        crate::git::git(folder, &["commit", "-q", "-m", "Start"], &identity)
            .await
            .expect("commit");
        std::fs::write(folder.join("a.txt"), "two\n").expect("write");
        crate::git::git(folder, &["stash", "-q"], &identity)
            .await
            .expect("stash");
        std::fs::write(folder.join("a.txt"), "one\nthree\n").expect("write");

        let copy = copy_status(folder).await.expect("a repository");
        assert_eq!(copy.branch.as_deref(), Some("main"));
        assert_eq!(copy.upstream, None);
        // No remote has the commit.
        assert_eq!(copy.ahead, 1);
        assert_eq!(copy.commit.as_ref().map(String::len), Some(7));
        assert_eq!(
            (copy.changed_files, copy.added_lines, copy.removed_lines),
            (1, 1, 0)
        );
        assert_eq!(copy.stashes, 1);
        assert_eq!(copy.fetched_at, None);
        assert_eq!(copy_status(&folder.join("missing")).await, None);
    }
}
