//! Checkpoints: a thread's working tree saved as hidden git refs around each turn, so its
//! changes can be shown per turn and for the whole thread. A port of t3code's `CheckpointStore`
//! and its git driver.
//!
//! - Turn 0 is the baseline, taken before the thread's first turn. Each finished turn then
//!   takes the next number. The refs are the record, so nothing else is saved and a restart
//!   loses nothing.
//! - A checkpoint is a commit of the whole tree, made with a private index and stored under
//!   `refs/agentz/checkpoints/<machine>/<thread>/<turn>`. Branches, the user's index and the
//!   working tree are never touched.
//! - Folders outside a git repository get none. Sparse checkouts are skipped too: rebuilding
//!   their index could record the files outside the cone as deleted.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use agent_thread::TurnPoint;
use agentz_protocol::diff::{DiffScope, DiffStatus};
use anyhow::{Context as _, Result};
use projects::ThreadId;

use crate::git::{git, git_limited, is_repository};

pub(crate) const REF_ROOT: &str = "refs/agentz/checkpoints";
/// t3code's limit. Larger patches are cut short.
const MAX_PATCH_BYTES: usize = 10_000_000;
/// Git renames objects and refs into place without flushing them by default, so a crash can
/// leave empty files that break later commands. Checkpoint writes flush first (t3code).
const DURABLE_WRITE: [&str; 4] = [
    "-c",
    "core.fsync=objects,reference",
    "-c",
    "core.fsyncMethod=fsync",
];
const INDEX_CONFIG: [&str; 2] = ["-c", "core.fsmonitor=false"];
const AUTHOR: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "agentZ"),
    ("GIT_AUTHOR_EMAIL", "agentz@users.noreply.github.com"),
    ("GIT_COMMITTER_NAME", "agentZ"),
    ("GIT_COMMITTER_EMAIL", "agentz@users.noreply.github.com"),
];

/// One thread's checkpoints, in the folder its agent works in.
#[derive(Clone, Debug)]
pub(crate) struct Checkpoints {
    cwd: PathBuf,
    prefix: String,
}

/// A diff between two checkpoints, as git prints it.
pub(crate) struct RawDiff {
    pub status: DiffStatus,
    pub turns: u32,
    pub patch: String,
    pub truncated: bool,
}

impl Checkpoints {
    pub(crate) fn new(cwd: PathBuf, machine_id: &str, thread_id: ThreadId) -> Self {
        let machine: String = machine_id
            .chars()
            .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
            .collect();
        Self {
            cwd,
            prefix: format!("{REF_ROOT}/{machine}/{}", thread_id.0),
        }
    }

    fn turn_ref(&self, turn: u32) -> String {
        format!("{}/{turn}", self.prefix)
    }

    /// Takes the baseline before the first turn, and a checkpoint after every turn.
    pub(crate) async fn on_turn(&self, point: TurnPoint) {
        if let Err(error) = self.try_on_turn(point).await {
            log::error!(
                "failed to take a checkpoint in {}: {error:#}",
                self.cwd.display()
            );
        }
    }

    async fn try_on_turn(&self, point: TurnPoint) -> Result<()> {
        if !is_repository(&self.cwd).await {
            return Ok(());
        }
        match (point, self.latest().await?) {
            (TurnPoint::Starting, None) => self.capture(0).await,
            (TurnPoint::Ended, Some(latest)) => self.capture(latest + 1).await,
            // Without a baseline (it failed, or the folder wasn't a repository yet) a turn's
            // checkpoint would have nothing to compare with.
            (TurnPoint::Starting, Some(_)) | (TurnPoint::Ended, None) => Ok(()),
        }
    }

    /// The newest checkpoint's turn, or `None` without a baseline.
    pub(crate) async fn latest(&self) -> Result<Option<u32>> {
        let output = git(
            &self.cwd,
            &[
                "for-each-ref",
                "--format=%(refname)",
                &format!("{}/", self.prefix),
            ],
            &[],
        )
        .await?;
        Ok(output
            .lines()
            .filter_map(|name| name.rsplit('/').next()?.parse::<u32>().ok())
            .max())
    }

    async fn capture(&self, turn: u32) -> Result<()> {
        let cwd = &self.cwd;
        let sparse = git(cwd, &["config", "--bool", "core.sparseCheckout"], &[])
            .await
            .unwrap_or_default();
        if sparse.trim() == "true" {
            return Ok(());
        }
        let common_dir = git(cwd, &["rev-parse", "--git-common-dir"], &[]).await?;
        let common_dir = cwd.join(common_dir.trim());
        let index = common_dir.join(format!("agentz-checkpoint-index-{}", uuid::Uuid::new_v4()));
        let result = self.capture_with_index(turn, &index).await;
        // A killed git can leave the private index's lock behind.
        for path in [index.clone(), index.with_extension("lock")] {
            match tokio::fs::remove_file(&path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => log::error!("failed to remove {}: {error}", path.display()),
            }
        }
        result
    }

    async fn capture_with_index(&self, turn: u32, index: &Path) -> Result<()> {
        let cwd = &self.cwd;
        let index_value = index.to_string_lossy().into_owned();
        let mut env = vec![("GIT_INDEX_FILE", index_value.as_str())];
        env.extend(AUTHOR);
        let has_head = git(
            cwd,
            &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
            &[],
        )
        .await
        .is_ok();
        if has_head && !reuse_index(cwd, index, &env).await {
            remove_if_present(index).await?;
            git(
                cwd,
                &[INDEX_CONFIG[0], INDEX_CONFIG[1], "read-tree", "HEAD"],
                &env,
            )
            .await?;
        }
        let mut add = Vec::from(INDEX_CONFIG);
        add.extend(DURABLE_WRITE);
        add.extend(["add", "-A", "--", "."]);
        git(cwd, &add, &env).await?;
        let mut write_tree = Vec::from(INDEX_CONFIG);
        write_tree.extend(DURABLE_WRITE);
        write_tree.push("write-tree");
        let tree = git(cwd, &write_tree, &env).await?;
        let tree = tree.trim();
        anyhow::ensure!(!tree.is_empty(), "git write-tree printed no tree");
        let turn_ref = self.turn_ref(turn);
        let message = format!("agentZ checkpoint {turn_ref}");
        let mut commit_tree = Vec::from(DURABLE_WRITE);
        commit_tree.extend(["commit-tree", tree, "-m", &message]);
        let commit = git(cwd, &commit_tree, &env).await?;
        let commit = commit.trim();
        anyhow::ensure!(!commit.is_empty(), "git commit-tree printed no commit");
        let mut update_ref = Vec::from(DURABLE_WRITE);
        update_ref.extend(["update-ref", &turn_ref, commit]);
        git(cwd, &update_ref, &[]).await?;
        Ok(())
    }

    /// The patch for the scope, between the checkpoints that bound it.
    pub(crate) async fn diff(&self, scope: DiffScope) -> Result<RawDiff> {
        let empty = |status, turns| RawDiff {
            status,
            turns,
            patch: String::new(),
            truncated: false,
        };
        if !is_repository(&self.cwd).await {
            return Ok(empty(DiffStatus::NotRepository, 0));
        }
        let turns = match self.latest().await? {
            None | Some(0) => return Ok(empty(DiffStatus::NoTurns, 0)),
            Some(latest) => latest,
        };
        let from = match scope {
            DiffScope::LatestTurn => turns - 1,
            DiffScope::All => 0,
        };
        let from = format!("{}^{{commit}}", self.turn_ref(from));
        let to = format!("{}^{{commit}}", self.turn_ref(turns));
        let (patch, truncated) = git_limited(
            &self.cwd,
            &[
                "-c",
                "core.quotePath=false",
                "diff",
                "--patch",
                "--find-renames",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--src-prefix=a/",
                "--dst-prefix=b/",
                &from,
                &to,
            ],
            MAX_PATCH_BYTES,
        )
        .await?;
        Ok(RawDiff {
            status: DiffStatus::Ready,
            turns,
            patch,
            truncated,
        })
    }

    /// Puts the folder back as it was at the scope's first checkpoint, and drops the later ones,
    /// so the restored turns no longer count. t3code's `restoreCheckpoint`: ignored files stay.
    pub(crate) async fn restore(&self, scope: DiffScope) -> Result<()> {
        let turns = match self.latest().await? {
            None | Some(0) => anyhow::bail!("no turn has finished yet"),
            Some(latest) => latest,
        };
        let target = match scope {
            DiffScope::LatestTurn => turns - 1,
            DiffScope::All => 0,
        };
        let commit = git(
            &self.cwd,
            &[
                "rev-parse",
                "--verify",
                &format!("{}^{{commit}}", self.turn_ref(target)),
            ],
            &[],
        )
        .await?;
        let commit = commit.trim();
        let tracked = git(
            &self.cwd,
            &[
                "ls-files",
                "--cached",
                &format!("--with-tree={commit}"),
                "-z",
                "--",
                ".",
            ],
            &[],
        )
        .await?;
        // An empty index and checkpoint have nothing for the pathspec to match.
        if !tracked.is_empty() {
            git(
                &self.cwd,
                &[
                    "restore",
                    "--source",
                    commit,
                    "--worktree",
                    "--staged",
                    "--",
                    ".",
                ],
                &[],
            )
            .await?;
        }
        git(&self.cwd, &["clean", "-fd", "--", "."], &[]).await?;
        let has_head = git(
            &self.cwd,
            &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"],
            &[],
        )
        .await
        .is_ok();
        if has_head {
            git(&self.cwd, &["reset", "--quiet", "--", "."], &[]).await?;
        }
        for turn in target + 1..=turns {
            git(&self.cwd, &["update-ref", "-d", &self.turn_ref(turn)], &[]).await?;
        }
        Ok(())
    }

    /// Removes the thread's checkpoints, as when it's deleted.
    pub(crate) async fn delete(&self) -> Result<()> {
        if !is_repository(&self.cwd).await {
            return Ok(());
        }
        let refs = git(
            &self.cwd,
            &[
                "for-each-ref",
                "--format=%(refname)",
                &format!("{}/", self.prefix),
            ],
            &[],
        )
        .await?;
        for name in refs.lines() {
            git(&self.cwd, &["update-ref", "-d", name], &[]).await?;
        }
        Ok(())
    }
}

/// Starts the private index from a copy of the user's, so `git add` can trust the file stats it
/// already has instead of hashing every file. Resetting it to `HEAD` keeps those stats only
/// for files that match `HEAD` (t3code). Returns false when the copy can't be used.
async fn reuse_index(cwd: &Path, index: &Path, env: &[(&str, &str)]) -> bool {
    let attempt = async {
        let source = git(
            cwd,
            &["rev-parse", "--path-format=absolute", "--git-path", "index"],
            &[],
        )
        .await?;
        let source = PathBuf::from(source.trim());
        let modified = tokio::fs::metadata(&source).await?.modified()?;
        tokio::fs::copy(&source, index).await?;
        git(
            cwd,
            &[
                INDEX_CONFIG[0],
                INDEX_CONFIG[1],
                "read-tree",
                "--reset",
                "HEAD",
            ],
            env,
        )
        .await?;
        // `read-tree` rewrote the index just now. Dating it back below the user's index keeps
        // git's racy-file check: files changed in the same second as that index get hashed.
        let since_epoch = modified.duration_since(SystemTime::UNIX_EPOCH)?;
        let seconds = since_epoch
            .saturating_sub(Duration::from_millis(1))
            .as_secs();
        anyhow::ensure!(seconds > 0, "the index has no usable timestamp");
        let dated = SystemTime::UNIX_EPOCH + Duration::from_secs(seconds);
        std::fs::File::options()
            .write(true)
            .open(index)?
            .set_times(
                std::fs::FileTimes::new()
                    .set_accessed(dated)
                    .set_modified(dated),
            )?;
        // Files marked assume-unchanged (lowercase tags) or skip-worktree (`S`) would keep
        // stale contents, so such an index is rebuilt from scratch.
        let tags = git(cwd, &["ls-files", "-v", "-z"], env).await?;
        let flagged = tags.split('\0').any(|record| {
            record
                .bytes()
                .next()
                .is_some_and(|tag| tag.is_ascii_lowercase() || tag == b'S')
        });
        anyhow::Ok(!flagged)
    };
    attempt.await.unwrap_or(false)
}

async fn remove_if_present(path: &Path) -> Result<()> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("removing {}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn run(cwd: &Path, args: &[&str]) -> String {
        git(cwd, args, &[]).await.expect("git runs")
    }

    async fn repository() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temporary directory");
        run(dir.path(), &["init", "-q", "-b", "main"]).await;
        run(dir.path(), &["config", "user.name", "Test"]).await;
        run(dir.path(), &["config", "user.email", "test@example.com"]).await;
        std::fs::write(dir.path().join("README.md"), "one\ntwo\n").expect("a file");
        run(dir.path(), &["add", "."]).await;
        run(dir.path(), &["commit", "-q", "-m", "first"]).await;
        dir
    }

    fn files(patch: &str) -> Vec<(String, u32, u32)> {
        agentz_protocol::diff::parse_patch(patch)
            .into_iter()
            .map(|file| (file.path, file.additions, file.deletions))
            .collect()
    }

    #[tokio::test]
    async fn turns_are_checkpointed_without_touching_the_branch() {
        if git(Path::new("/"), &["--version"], &[]).await.is_err() {
            return;
        }
        let repository = repository().await;
        let cwd = repository.path();
        let checkpoints = Checkpoints::new(cwd.to_path_buf(), "machine-1", ThreadId(7));
        let head = run(cwd, &["rev-parse", "HEAD"]).await;

        // A change from before the first turn belongs to the baseline.
        std::fs::write(cwd.join("README.md"), "one\ntwo\nthree\n").expect("a change");
        checkpoints.on_turn(TurnPoint::Starting).await;
        std::fs::write(cwd.join("notes.txt"), "new\n").expect("an untracked file");
        checkpoints.on_turn(TurnPoint::Ended).await;
        checkpoints.on_turn(TurnPoint::Starting).await;
        std::fs::remove_file(cwd.join("README.md")).expect("a deletion");
        checkpoints.on_turn(TurnPoint::Ended).await;
        assert_eq!(checkpoints.latest().await.expect("refs"), Some(2));

        let latest = checkpoints
            .diff(DiffScope::LatestTurn)
            .await
            .expect("a diff");
        assert_eq!(latest.status, DiffStatus::Ready);
        assert_eq!(latest.turns, 2);
        assert_eq!(files(&latest.patch), vec![("README.md".into(), 0, 3)]);
        let all = checkpoints.diff(DiffScope::All).await.expect("a diff");
        assert_eq!(
            files(&all.patch),
            vec![("README.md".into(), 0, 3), ("notes.txt".into(), 1, 0)]
        );

        // The branch, the index and the working tree are as they were.
        assert_eq!(run(cwd, &["rev-parse", "HEAD"]).await, head);
        assert_eq!(run(cwd, &["branch", "--list"]).await.trim(), "* main");
        assert_eq!(
            run(cwd, &["status", "--porcelain"]).await,
            " D README.md\n?? notes.txt\n"
        );
        let leftovers: Vec<_> = std::fs::read_dir(cwd.join(".git"))
            .expect("the git directory")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("agentz"))
            .collect();
        assert!(leftovers.is_empty());

        // The user's index was reusable, so unchanged files weren't hashed again.
        let probe = cwd.join(".git").join("probe-index");
        let probe_value = probe.to_string_lossy().into_owned();
        assert!(reuse_index(cwd, &probe, &[("GIT_INDEX_FILE", probe_value.as_str())]).await);
        std::fs::remove_file(&probe).expect("the probe index");

        // Another thread's checkpoints are its own.
        let other = Checkpoints::new(cwd.to_path_buf(), "machine-1", ThreadId(70));
        assert_eq!(other.latest().await.expect("refs"), None);
        assert_eq!(
            other.diff(DiffScope::All).await.expect("a diff").status,
            DiffStatus::NoTurns
        );

        checkpoints.delete().await.expect("deleted");
        assert_eq!(checkpoints.latest().await.expect("refs"), None);
    }

    #[tokio::test]
    async fn restoring_puts_files_back_and_forgets_later_turns() {
        if git(Path::new("/"), &["--version"], &[]).await.is_err() {
            return;
        }
        let repository = repository().await;
        let cwd = repository.path();
        std::fs::write(cwd.join(".gitignore"), "build/\n").expect("a file");
        let checkpoints = Checkpoints::new(cwd.to_path_buf(), "machine-1", ThreadId(3));
        let readme = || std::fs::read_to_string(cwd.join("README.md")).unwrap_or_default();

        checkpoints.on_turn(TurnPoint::Starting).await;
        std::fs::write(cwd.join("README.md"), "one\ntwo\nthree\n").expect("a change");
        std::fs::write(cwd.join("notes.txt"), "new\n").expect("an untracked file");
        checkpoints.on_turn(TurnPoint::Ended).await;
        checkpoints.on_turn(TurnPoint::Starting).await;
        std::fs::remove_file(cwd.join("README.md")).expect("a deletion");
        std::fs::write(cwd.join("other.txt"), "other\n").expect("another file");
        std::fs::create_dir(cwd.join("build")).expect("an ignored folder");
        std::fs::write(cwd.join("build/out"), "out\n").expect("an ignored file");
        checkpoints.on_turn(TurnPoint::Ended).await;

        checkpoints
            .restore(DiffScope::LatestTurn)
            .await
            .expect("restores the latest turn");
        assert_eq!(readme(), "one\ntwo\nthree\n");
        assert!(cwd.join("notes.txt").exists());
        assert!(!cwd.join("other.txt").exists());
        assert!(cwd.join("build/out").exists(), "ignored files stay");
        assert_eq!(checkpoints.latest().await.expect("refs"), Some(1));
        assert_eq!(
            run(cwd, &["status", "--porcelain"]).await,
            " M README.md\n?? .gitignore\n?? notes.txt\n",
            "nothing is staged"
        );

        checkpoints
            .restore(DiffScope::All)
            .await
            .expect("restores everything");
        assert_eq!(readme(), "one\ntwo\n");
        assert!(!cwd.join("notes.txt").exists());
        assert!(cwd.join(".gitignore").exists(), "it was in the baseline");
        assert_eq!(
            checkpoints
                .diff(DiffScope::All)
                .await
                .expect("a diff")
                .status,
            DiffStatus::NoTurns
        );
        assert!(checkpoints.restore(DiffScope::All).await.is_err());
    }

    #[tokio::test]
    async fn repositories_without_commits_and_plain_folders() {
        if git(Path::new("/"), &["--version"], &[]).await.is_err() {
            return;
        }
        let empty = tempfile::tempdir().expect("a temporary directory");
        run(empty.path(), &["init", "-q"]).await;
        let checkpoints = Checkpoints::new(empty.path().to_path_buf(), "m", ThreadId(1));
        checkpoints.on_turn(TurnPoint::Starting).await;
        std::fs::write(empty.path().join("a.txt"), "a\n").expect("a file");
        checkpoints.on_turn(TurnPoint::Ended).await;
        let diff = checkpoints
            .diff(DiffScope::LatestTurn)
            .await
            .expect("a diff");
        assert_eq!(files(&diff.patch), vec![("a.txt".into(), 1, 0)]);

        let plain = tempfile::tempdir().expect("a temporary directory");
        let checkpoints = Checkpoints::new(plain.path().to_path_buf(), "m", ThreadId(1));
        checkpoints.on_turn(TurnPoint::Starting).await;
        checkpoints.on_turn(TurnPoint::Ended).await;
        let diff = checkpoints.diff(DiffScope::All).await.expect("a diff");
        assert_eq!(diff.status, DiffStatus::NotRepository);
    }
}
