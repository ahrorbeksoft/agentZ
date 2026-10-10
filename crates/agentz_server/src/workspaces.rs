//! Worktrees and pastures, made and removed by the server on its machine.
//!
//! - **Worktrees** are t3code's: `git worktree add -b <branch> <folder> <base>`, then
//!   submodules, under `<data dir>/worktrees/<repo>/<branch>`.
//! - **Pastures** port cow's `create` (MIT, `references/cow/src/commands/create.rs`): a
//!   copy-on-write clone of the whole folder under `<data dir>/pastures/<repo>/<branch>`, its
//!   git fixed up and runtime files removed, undone if any step fails. cow's `sync`, `extract
//!   --branch` and `remove` move work between a pasture and its project.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use agentz_protocol::workspace::{
    Checkout, GitBranch, PastureSupport, ProjectGit, RepositoryCheckouts, WorkspaceRemoval,
    remote_branch,
};
use anyhow::{Context as _, Result, anyhow};
use projects::{Workspace, WorkspaceKind};
use serde::Deserialize;

use crate::git::{git, is_repository};

/// Never cloned into pastures (cow): build output the pasture rebuilds itself, whose clone would
/// go stale as soon as the project rebuilt.
#[cfg(target_os = "macos")]
const BUILD_ARTIFACT_DIRS: &[&str] = &["target", ".build", "DerivedData", ".turbo"];
/// Runtime files a cloned project would otherwise appear to have open (cow).
const RUNTIME_FILE_EXTENSIONS: &[&str] = &["pid", "sock", "socket"];
const BRANCH_PREFIX: &str = "agentz/";
/// A fetch that hangs (a remote that's down, a login it waits for) gives up after this.
const FETCH_TIMEOUT: Duration = Duration::from_secs(120);
/// t3code's limit for a branch name made from a title.
const MAX_TITLE_BRANCH_CHARS: usize = 64;
/// Paths given to one git command at a time, well within the system's argument limit.
const PATHS_PER_COMMAND: usize = 200;

pub(crate) struct NewWorkspace {
    pub kind: WorkspaceKind,
    /// The project's folder.
    pub repo: PathBuf,
    pub data_dir: PathBuf,
    pub base: Option<String>,
    pub branch: Option<String>,
    /// Fills its submodules too; otherwise [`init_submodules`] does, as a step of its own.
    pub submodules: bool,
}

/// Makes a worktree or pasture of the project on a new branch.
pub(crate) async fn create(new: NewWorkspace) -> Result<Workspace> {
    let repo = &new.repo;
    anyhow::ensure!(
        is_repository(repo).await,
        "{} isn't a git repository",
        repo.display()
    );
    let base = match new.base.filter(|base| !base.trim().is_empty()) {
        Some(base) => base,
        None => current_branch(repo).await?.unwrap_or_else(|| "HEAD".into()),
    };
    git(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base}^{{commit}}"),
        ],
        &[],
    )
    .await
    .with_context(|| format!("there's no branch or commit `{base}`"))?;
    let branch = match new.branch.filter(|branch| !branch.trim().is_empty()) {
        Some(branch) => branch.trim().to_string(),
        None => temporary_branch(),
    };
    git(repo, &["check-ref-format", "--branch", &branch], &[])
        .await
        .with_context(|| format!("`{branch}` isn't a valid branch name"))?;
    // git's own failure would say less (t3code checks first, too).
    anyhow::ensure!(
        git(
            repo,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{branch}")
            ],
            &[]
        )
        .await
        .is_err(),
        "the branch `{branch}` already exists; choose another name"
    );
    let folder = folder_name(new.kind);
    let repo_name = repo
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into());
    let path = new
        .data_dir
        .join(folder)
        .join(repo_name)
        .join(branch.replace('/', "-"));
    anyhow::ensure!(
        !path.exists(),
        "{} already exists; remove it or choose another branch name",
        path.display()
    );
    let parent = path
        .parent()
        .context("the workspace folder has no parent")?;
    tokio::fs::create_dir_all(parent)
        .await
        .with_context(|| format!("creating {}", parent.display()))?;
    // Agents report their folder resolved (`/private/tmp`, not `/tmp`), as projects are stored.
    let path = match path.file_name() {
        Some(name) => tokio::fs::canonicalize(parent).await?.join(name),
        None => path,
    };
    match new.kind {
        WorkspaceKind::Worktree => create_worktree(repo, &path, &branch, &base).await?,
        WorkspaceKind::Pasture => {
            if let Err(error) = create_pasture(repo, &path, &branch, &base).await {
                remove_folder(&path).await.ok();
                return Err(error);
            }
        }
    }
    // Best effort: a submodule that can't be fetched mustn't cost the thread its workspace
    // (t3code).
    if new.submodules
        && let Err(error) = init_submodules(&path).await
    {
        log::warn!("submodules in {} are empty: {error:#}", path.display());
    }
    Ok(Workspace {
        kind: new.kind,
        path,
        branch: Some(branch),
        base: Some(base),
        created_at: SystemTime::now(),
    })
}

async fn create_worktree(repo: &Path, path: &Path, branch: &str, base: &str) -> Result<()> {
    let path_arg = path.to_string_lossy();
    // Without `--no-track`, a branch from `origin/main` would push to origin's `main`.
    git(
        repo,
        &[
            "worktree",
            "add",
            "--no-track",
            "-b",
            branch,
            &path_arg,
            base,
        ],
        &[],
    )
    .await?;
    Ok(())
}

/// `git worktree add` leaves submodules empty, and a pasture's switch may leave them behind.
pub(crate) async fn init_submodules(path: &Path) -> Result<()> {
    if !has_submodules(path) {
        return Ok(());
    }
    git(path, &["submodule", "update", "--init", "--recursive"], &[]).await?;
    Ok(())
}

pub(crate) fn has_submodules(folder: &Path) -> bool {
    folder.join(".gitmodules").exists()
}

/// `git fetch origin`, for the branch list's Fetch. The branches origin deleted go too.
pub(crate) async fn fetch_origin(folder: &Path) -> Result<()> {
    fetch(folder, &["fetch", "--prune", "--no-tags", "origin"]).await
}

/// Brings `origin/<branch>` up to date before a workspace starts from it.
pub(crate) async fn fetch_base(folder: &Path, branch: &str) -> Result<()> {
    let refspec = format!("+refs/heads/{branch}:refs/remotes/origin/{branch}");
    fetch(folder, &["fetch", "--no-tags", "origin", &refspec]).await
}

async fn fetch(folder: &Path, args: &[&str]) -> Result<()> {
    // Nobody can answer a login prompt for the server.
    let fetching = git(folder, args, &[("GIT_TERMINAL_PROMPT", "0")]);
    tokio::time::timeout(FETCH_TIMEOUT, fetching)
        .await
        .map_err(|_| anyhow!("fetching from origin took too long"))??;
    Ok(())
}

async fn create_pasture(repo: &Path, path: &Path, branch: &str, base: &str) -> Result<()> {
    // A linked worktree's `.git` is a file pointing into its main repository, which a clone
    // would share (cow refuses them too).
    anyhow::ensure!(
        repo.join(".git").is_dir(),
        "{} is a git worktree or not a repository's top folder; pastures are made from a \
         repository's own checkout",
        repo.display()
    );
    let (source, destination) = (repo.to_path_buf(), path.to_path_buf());
    tokio::task::spawn_blocking(move || clone_folder(&source, &destination))
        .await
        .context("cloning the project")??;

    // The source's linked worktrees would be listed as this clone's own, with paths into the
    // source (cow).
    let inherited_worktrees = path.join(".git").join("worktrees");
    if inherited_worktrees.exists() {
        remove_folder(&inherited_worktrees).await?;
    }
    // The clone has the source's remote-tracking refs, so `git checkout origin/main` could
    // otherwise create a local branch shadowing them (cow).
    git(path, &["config", "--local", "checkout.guess", "false"], &[]).await?;
    switch_copy(path, branch, base).await?;

    remove_runtime_files(path).await?;
    let config = repo.join(".cow.json");
    if config.exists() {
        run_cow_config(path, &config).await?;
    }
    Ok(())
}

/// Puts a pasture's copy on a new branch from `base`. git carries the copy's uncommitted
/// changes over when they fit the base; the ones to files the base changes are left out, as
/// are untracked files where the base has files, so the switch goes through.
async fn switch_copy(path: &Path, branch: &str, base: &str) -> Result<()> {
    let switch = ["checkout", "-q", "--no-track", "-b", branch, base];
    let Err(error) = git(path, &switch, &[]).await else {
        return Ok(());
    };
    let differing = git(
        path,
        &["diff", "--name-only", "--no-renames", "-z", "HEAD", base],
        &[],
    )
    .await
    .map_err(|_| error)?;
    let in_head: HashSet<String> = git(path, &["ls-tree", "-r", "-z", "--name-only", "HEAD"], &[])
        .await?
        .split('\0')
        .map(str::to_string)
        .collect();
    let (restored, added): (Vec<&str>, Vec<&str>) = differing
        .split('\0')
        .filter(|file| !file.is_empty())
        .partition(|file| in_head.contains(*file));
    for files in restored.chunks(PATHS_PER_COMMAND) {
        let mut args = vec!["checkout", "-q", "HEAD", "--"];
        args.extend(files);
        git(path, &args, &[]).await?;
    }
    for files in added.chunks(PATHS_PER_COMMAND) {
        let mut args = vec!["rm", "-q", "--cached", "--ignore-unmatch", "--"];
        args.extend(files);
        git(path, &args, &[]).await?;
        for file in files {
            let file = path.join(file);
            if file.is_file() || file.is_symlink() {
                tokio::fs::remove_file(&file)
                    .await
                    .with_context(|| format!("removing {}", file.display()))?;
            }
        }
    }
    git(path, &switch, &[]).await?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn clone_folder(source: &Path, destination: &Path) -> Result<()> {
    let has_artifacts = BUILD_ARTIFACT_DIRS
        .iter()
        .any(|name| source.join(name).is_dir());
    if !has_artifacts {
        return clonefile(source, destination);
    }
    // cow's selective clone, without its symlinked dependency folders: everything at the top
    // but the build output, each entry in one `clonefile`.
    std::fs::create_dir(destination)
        .with_context(|| format!("creating {}", destination.display()))?;
    for entry in
        std::fs::read_dir(source).with_context(|| format!("reading {}", source.display()))?
    {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        if file_type.is_dir() && BUILD_ARTIFACT_DIRS.iter().any(|skipped| name == *skipped) {
            continue;
        }
        let target = destination.join(&name);
        if file_type.is_symlink() {
            // `clonefile` would follow it.
            std::os::unix::fs::symlink(std::fs::read_link(entry.path())?, &target)?;
        } else {
            clonefile(&entry.path(), &target)?;
        }
    }
    Ok(())
}

/// APFS's `clonefile(2)`: the whole tree in one call, sharing every block until it changes.
#[cfg(target_os = "macos")]
fn clonefile(source: &Path, destination: &Path) -> Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt as _;

    let source_c = CString::new(source.as_os_str().as_bytes()).context("source path")?;
    let destination_c =
        CString::new(destination.as_os_str().as_bytes()).context("destination path")?;
    // SAFETY: both are valid NUL-terminated paths that outlive the call.
    let result = unsafe { libc::clonefile(source_c.as_ptr(), destination_c.as_ptr(), 0) };
    if result != 0 {
        return Err(anyhow!(
            "cloning {} to {} failed: {}",
            source.display(),
            destination.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

/// A reflink copy on btrfs or xfs, or else a full copy (cow on Linux).
#[cfg(not(target_os = "macos"))]
#[allow(clippy::disallowed_methods, reason = "runs on a blocking thread")]
fn clone_folder(source: &Path, destination: &Path) -> Result<()> {
    use std::process::Command;

    let status = Command::new("cp")
        .arg("--reflink=always")
        .arg("-R")
        .arg(source)
        .arg(destination)
        .stderr(std::process::Stdio::null())
        .status();
    if status.is_ok_and(|status| status.success()) {
        return Ok(());
    }
    log::warn!("{} has no reflinks; copying it in full", source.display());
    if destination.exists() {
        std::fs::remove_dir_all(destination)?;
    }
    let status = Command::new("cp")
        .arg("-R")
        .arg(source)
        .arg(destination)
        .status()
        .context("running cp")?;
    anyhow::ensure!(status.success(), "copying {} failed", source.display());
    Ok(())
}

async fn remove_runtime_files(path: &Path) -> Result<()> {
    let mut entries = tokio::fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();
        let is_runtime_file = entry_path.extension().is_some_and(|extension| {
            RUNTIME_FILE_EXTENSIONS
                .iter()
                .any(|known| extension == *known)
        });
        if is_runtime_file && !entry.file_type().await?.is_dir() {
            tokio::fs::remove_file(&entry_path)
                .await
                .with_context(|| format!("removing {}", entry_path.display()))?;
        }
    }
    Ok(())
}

/// A repository's `.cow.json` `post_clone`: `remove` patterns, then `run` commands.
async fn run_cow_config(path: &Path, config: &Path) -> Result<()> {
    #[derive(Deserialize)]
    struct CowConfig {
        post_clone: Option<PostClone>,
    }
    #[derive(Deserialize)]
    struct PostClone {
        #[serde(default)]
        remove: Vec<String>,
        #[serde(default)]
        run: Vec<String>,
    }
    let text = tokio::fs::read_to_string(config)
        .await
        .with_context(|| format!("reading {}", config.display()))?;
    let config: CowConfig = serde_json::from_str(&text).context("reading .cow.json")?;
    let Some(post_clone) = config.post_clone else {
        return Ok(());
    };
    for pattern in &post_clone.remove {
        for matched in expand_pattern(path, pattern)? {
            if tokio::fs::symlink_metadata(&matched).await?.is_dir() {
                remove_folder(&matched).await?;
            } else {
                tokio::fs::remove_file(&matched)
                    .await
                    .with_context(|| format!("removing {}", matched.display()))?;
            }
        }
    }
    for command in &post_clone.run {
        let status = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(path)
            .stdin(std::process::Stdio::null())
            .status()
            .await
            .with_context(|| format!("running `{command}`"))?;
        anyhow::ensure!(status.success(), "`{command}` failed after cloning");
    }
    Ok(())
}

/// The paths under `root` matching a glob such as `apps/*/dist`, one component at a time so
/// only matching folders are read.
fn expand_pattern(root: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let mut matches = vec![root.to_path_buf()];
    for component in pattern.split('/').filter(|component| !component.is_empty()) {
        anyhow::ensure!(
            component != ".." && component != "**",
            "unsupported .cow.json pattern `{pattern}`"
        );
        let has_wildcard = component.contains(['*', '?', '[', '{']);
        let matcher = globset::Glob::new(component)
            .with_context(|| format!("reading the pattern `{pattern}`"))?
            .compile_matcher();
        let mut next = Vec::new();
        for folder in matches {
            if !has_wildcard {
                let candidate = folder.join(component);
                if candidate.symlink_metadata().is_ok() {
                    next.push(candidate);
                }
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&folder) else {
                continue;
            };
            for entry in entries.flatten() {
                if matcher.is_match(entry.file_name()) {
                    next.push(entry.path());
                }
            }
        }
        matches = next;
    }
    matches.retain(|path| path != root);
    Ok(matches)
}

/// Deletes the workspace's folder, keeping its branch (herdr). Without `force`, a workspace with
/// work that would be lost is left alone and the reason returned.
pub(crate) async fn remove(
    repo: &Path,
    workspace: &Workspace,
    force: bool,
) -> Result<WorkspaceRemoval> {
    let path = &workspace.path;
    match workspace.kind {
        WorkspaceKind::Worktree => {
            if !path.exists() {
                git(repo, &["worktree", "prune"], &[]).await?;
                return Ok(WorkspaceRemoval::Removed);
            }
            let path_arg = path.to_string_lossy();
            let mut args = vec!["worktree", "remove"];
            if force {
                args.push("--force");
            }
            args.push(&path_arg);
            match git(repo, &args, &[]).await {
                Ok(_) => Ok(WorkspaceRemoval::Removed),
                Err(error) if !force => Ok(WorkspaceRemoval::NeedsConfirmation(format!(
                    "{error:#}. Removing it anyway loses its changed and untracked files."
                ))),
                Err(error) => Err(error),
            }
        }
        WorkspaceKind::Pasture => {
            if !force && path.exists() {
                let mut losses = Vec::new();
                let status = git(path, &["status", "--porcelain"], &[]).await?;
                let changed = status.lines().count();
                if changed > 0 {
                    losses.push(plural(changed, "uncommitted change"));
                }
                if let Some(head) = head_commit(path).await
                    && !has_commit(repo, &head).await
                {
                    losses.push("commits the project doesn't have".to_string());
                }
                if !losses.is_empty() {
                    return Ok(WorkspaceRemoval::NeedsConfirmation(format!(
                        "This pasture has {}. Removing it loses them; bring its branch to the \
                         project first to keep its commits.",
                        losses.join(" and ")
                    )));
                }
            }
            if path.exists() {
                remove_folder(path).await?;
            }
            Ok(WorkspaceRemoval::Removed)
        }
    }
}

/// cow's `sync`: rebases (or merges) the pasture onto a branch fetched from the project. A
/// failed rebase is undone, and the conflicting files named.
pub(crate) async fn sync(repo: &Path, pasture: &Path, branch: &str, merge: bool) -> Result<String> {
    // Untracked files, like the `.env` every pasture has, don't get in a rebase's way.
    let status = git(
        pasture,
        &["status", "--porcelain", "--untracked-files=no"],
        &[],
    )
    .await?;
    anyhow::ensure!(
        status.trim().is_empty(),
        "the pasture has uncommitted changes; commit or stash them before syncing"
    );
    let repo_arg = repo.to_string_lossy();
    git(pasture, &["fetch", "--no-tags", &repo_arg, branch], &[])
        .await
        .with_context(|| format!("fetching `{branch}` from the project"))?;
    if merge {
        if let Err(error) = git(pasture, &["merge", "--no-edit", "FETCH_HEAD"], &[]).await {
            let conflicts = conflicted_files(pasture).await;
            git(pasture, &["merge", "--abort"], &[]).await.ok();
            return Err(conflict_error(error, "merge", &conflicts));
        }
        return Ok(format!("Merged the project's `{branch}` into the pasture."));
    }
    if let Err(error) = git(pasture, &["rebase", "FETCH_HEAD"], &[]).await {
        let conflicts = conflicted_files(pasture).await;
        git(pasture, &["rebase", "--abort"], &[]).await.ok();
        return Err(conflict_error(error, "rebase", &conflicts));
    }
    Ok(format!(
        "Rebased the pasture onto the project's `{branch}`."
    ))
}

/// cow's `extract --branch`: creates (or fast-forwards) a branch in the project at the
/// pasture's `HEAD`, ready to review and push from there.
pub(crate) async fn bring_back(repo: &Path, pasture: &Path, branch: &str) -> Result<String> {
    git(repo, &["check-ref-format", "--branch", branch], &[])
        .await
        .with_context(|| format!("`{branch}` isn't a valid branch name"))?;
    let pasture_arg = pasture.to_string_lossy();
    git(
        repo,
        &[
            "fetch",
            "--no-tags",
            &pasture_arg,
            &format!("HEAD:refs/heads/{branch}"),
        ],
        &[],
    )
    .await
    .with_context(|| format!("bringing the pasture's work to the project's `{branch}`"))?;
    Ok(format!(
        "The project's branch `{branch}` now has the pasture's commits."
    ))
}

/// The repository's main checkout, also from inside one of its worktrees.
pub(crate) async fn main_checkout(folder: &Path) -> Result<PathBuf> {
    let common_dir = git(
        folder,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        &[],
    )
    .await?;
    let common_dir = PathBuf::from(common_dir.trim());
    if common_dir.file_name().is_some_and(|name| name == ".git")
        && let Some(root) = common_dir.parent()
    {
        return Ok(root.to_path_buf());
    }
    // A bare repository's or a separate git dir's checkout: the folder's own top.
    let top_level = git(folder, &["rev-parse", "--show-toplevel"], &[]).await?;
    Ok(PathBuf::from(top_level.trim()))
}

/// Makes a worktree or pasture of the repository `folder` is in, from what `folder` has
/// checked out unless `base` says otherwise. Returns the repository's main checkout with it.
pub(crate) async fn create_from(
    folder: &Path,
    kind: WorkspaceKind,
    base: Option<String>,
    branch: Option<String>,
    data_dir: PathBuf,
) -> Result<(PathBuf, Workspace)> {
    let (repo, base) = repository_and_base(folder, base).await?;
    let workspace = create(NewWorkspace {
        kind,
        repo: repo.clone(),
        data_dir,
        base: Some(base),
        branch,
        submodules: true,
    })
    .await?;
    Ok((repo, workspace))
}

/// The main checkout of the repository `folder` is in, and what a new branch starts from:
/// `base`, or what `folder` has checked out (its commit when it's detached).
pub(crate) async fn repository_and_base(
    folder: &Path,
    base: Option<String>,
) -> Result<(PathBuf, String)> {
    anyhow::ensure!(
        is_repository(folder).await,
        "{} isn't in a git repository",
        folder.display()
    );
    let repo = main_checkout(folder).await?;
    let base = match base.filter(|base| !base.trim().is_empty()) {
        Some(base) => base,
        None => match current_branch(folder).await? {
            Some(branch) => branch,
            None => git(folder, &["rev-parse", "HEAD"], &[])
                .await?
                .trim()
                .to_string(),
        },
    };
    Ok((repo, base))
}

/// Renames a workspace's branch `from` to `to`, unless `to` is taken in it or (for a pasture's
/// own repository) in the project: then it keeps `from`. Returns whether it was renamed.
pub(crate) async fn rename_branch(
    repo: &Path,
    workspace: &Path,
    from: &str,
    to: &str,
) -> Result<bool> {
    if from == to
        || git(workspace, &["check-ref-format", "--branch", to], &[])
            .await
            .is_err()
    {
        return Ok(false);
    }
    let reference = format!("refs/heads/{to}");
    for folder in [workspace, repo] {
        let taken = git(
            folder,
            &["rev-parse", "--verify", "--quiet", &reference],
            &[],
        )
        .await
        .is_ok();
        if taken {
            return Ok(false);
        }
    }
    git(workspace, &["branch", "-m", from, to], &[]).await?;
    Ok(true)
}

/// A branch named after a thread's title (t3code's `sanitizeBranchFragment`), under the
/// server's prefix: `agentz/fix-checkout-rounding`.
pub(crate) fn branch_for_title(title: &str) -> String {
    let is_edge = |character: char| "./_-".contains(character);
    let unquoted: String = title
        .trim()
        .to_lowercase()
        .chars()
        .filter(|character| !"'\"`".contains(*character))
        .collect();
    let unquoted =
        unquoted.trim_matches(|character: char| is_edge(character) || character.is_whitespace());
    let mut fragment = String::new();
    for character in unquoted.chars() {
        let character = if character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || "/_-".contains(character)
        {
            character
        } else {
            '-'
        };
        // Runs of dashes or slashes become one.
        if "-/".contains(character) && fragment.ends_with(character) {
            continue;
        }
        fragment.push(character);
    }
    let fragment: String = fragment
        .trim_matches(is_edge)
        .chars()
        .take(MAX_TITLE_BRANCH_CHARS)
        .collect();
    let fragment = fragment.trim_end_matches(is_edge);
    let fragment = if fragment.is_empty() {
        "update"
    } else {
        fragment
    };
    format!("{BRANCH_PREFIX}{fragment}")
}

/// The name the server gives a new workspace's branch until it can name it after its thread.
pub(crate) fn temporary_branch() -> String {
    format!("{BRANCH_PREFIX}{}", short_id())
}

/// The repository `folder` is in, with what `folder` has checked out, and its checkouts:
/// git's worktrees, then the pastures recorded on a project at its main checkout.
pub(crate) async fn repository_checkouts(
    folder: &Path,
    data_dir: &Path,
    project_workspaces: &[(PathBuf, Vec<Workspace>)],
) -> Result<RepositoryCheckouts> {
    anyhow::ensure!(
        is_repository(folder).await,
        "{} isn't in a git repository",
        folder.display()
    );
    let root = main_checkout(folder).await?;
    let mut git = project_git(&root, data_dir).await;
    git.branch = current_branch(folder).await.ok().flatten();
    let mut checkouts = worktrees(&root).await?;
    let pastures = project_workspaces
        .iter()
        .filter(|(path, _)| *path == root)
        .flat_map(|(_, workspaces)| workspaces)
        .filter(|workspace| workspace.kind == WorkspaceKind::Pasture && workspace.path.exists());
    for pasture in pastures {
        let branch = current_branch(&pasture.path).await.ok().flatten();
        checkouts.push(Checkout {
            path: pasture.path.clone(),
            branch: branch.or_else(|| pasture.branch.clone()),
            kind: Some(WorkspaceKind::Pasture),
        });
    }
    Ok(RepositoryCheckouts {
        git,
        checkouts,
        data_dir: String::new(),
    })
}

/// The repository's checkouts as `git worktree list` has them, the main one first. Bare and
/// prunable (deleted) ones are left out.
pub(crate) async fn worktrees(repo: &Path) -> Result<Vec<Checkout>> {
    let output = git(repo, &["worktree", "list", "--porcelain"], &[]).await?;
    Ok(parse_worktree_list(&output))
}

/// herdr's `parse_worktree_list_porcelain`.
fn parse_worktree_list(output: &str) -> Vec<Checkout> {
    let mut checkouts = Vec::new();
    for (index, entry) in output.split("\n\n").enumerate() {
        let mut path = None;
        let mut branch = None;
        let mut is_skipped = false;
        for line in entry.lines() {
            if let Some(value) = line.strip_prefix("worktree ") {
                path = Some(PathBuf::from(value));
            } else if let Some(value) = line.strip_prefix("branch ") {
                branch = Some(
                    value
                        .strip_prefix("refs/heads/")
                        .unwrap_or(value)
                        .to_string(),
                );
            } else if line == "bare" || line.starts_with("prunable") {
                is_skipped = true;
            }
        }
        if let Some(path) = path.filter(|_| !is_skipped) {
            let kind = (index > 0).then_some(WorkspaceKind::Worktree);
            checkouts.push(Checkout { path, branch, kind });
        }
    }
    checkouts
}

/// The project's branches and how a pasture of it would be made.
pub(crate) async fn project_git(repo: &Path, data_dir: &Path) -> ProjectGit {
    if !is_repository(repo).await {
        return ProjectGit::default();
    }
    let branch = current_branch(repo).await.ok().flatten();
    let local = local_branches(repo).await.unwrap_or_default();
    let remote = origin_branches(repo).await.unwrap_or_default();
    let checked_out: HashSet<String> = worktrees(repo)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter_map(|checkout| checkout.branch)
        .collect();
    let has_origin = git(repo, &["remote", "get-url", "origin"], &[])
        .await
        .is_ok();
    let default_branch = match origin_default_branch(repo).await {
        Some(branch) => Some(branch),
        None => ["main", "master"]
            .into_iter()
            .find(|name| local.iter().any(|branch| branch == name))
            .map(str::to_string),
    };
    let branches = local
        .into_iter()
        .map(|name| GitBranch {
            is_checked_out: checked_out.contains(&name),
            name,
            is_remote: false,
        })
        .chain(remote.into_iter().map(|name| GitBranch {
            name,
            is_remote: true,
            is_checked_out: false,
        }))
        .collect();
    let pastures = if repo.join(".git").is_dir() {
        pasture_support(repo, data_dir).await
    } else {
        PastureSupport::Unsupported(
            "Pastures are made from a repository's own checkout, not a worktree.".into(),
        )
    };
    ProjectGit {
        is_repository: true,
        branch,
        default_branch,
        has_origin,
        branches,
        pastures,
    }
}

pub(crate) async fn local_branches(repo: &Path) -> Result<Vec<String>> {
    branches_under(repo, "refs/heads").await
}

/// origin's branches (`origin/main`), most recently committed first, without `origin/HEAD`.
async fn origin_branches(repo: &Path) -> Result<Vec<String>> {
    Ok(branches_under(repo, "refs/remotes/origin")
        .await?
        .into_iter()
        .filter(|name| name != "origin/HEAD" && name != "origin")
        .collect())
}

async fn branches_under(repo: &Path, prefix: &str) -> Result<Vec<String>> {
    let output = git(
        repo,
        &[
            "for-each-ref",
            "--sort=-committerdate",
            "--format=%(refname:short)",
            prefix,
        ],
        &[],
    )
    .await?;
    Ok(output.lines().map(str::to_string).collect())
}

/// The branch `origin/HEAD` points at, as a clone sets it: `main`.
async fn origin_default_branch(repo: &Path) -> Option<String> {
    let output = git(
        repo,
        &[
            "symbolic-ref",
            "--quiet",
            "--short",
            "refs/remotes/origin/HEAD",
        ],
        &[],
    )
    .await
    .ok()?;
    remote_branch(output.trim()).map(str::to_string)
}

pub(crate) async fn current_branch(folder: &Path) -> Result<Option<String>> {
    let output = git(folder, &["branch", "--show-current"], &[]).await?;
    let branch = output.trim();
    Ok((!branch.is_empty()).then(|| branch.to_string()))
}

/// A clone is only copy-on-write within one volume, so the data directory must share the
/// project's.
#[cfg(target_os = "macos")]
async fn pasture_support(repo: &Path, data_dir: &Path) -> PastureSupport {
    let (repo, data_dir) = (repo.to_path_buf(), data_dir.to_path_buf());
    let check = tokio::task::spawn_blocking(move || -> Result<PastureSupport> {
        std::fs::create_dir_all(&data_dir)?;
        use std::os::unix::fs::MetadataExt as _;

        let file_system = file_system_name(&repo)?;
        if file_system != "apfs" {
            return Ok(PastureSupport::Unsupported(format!(
                "Pastures need APFS; this project is on {file_system}."
            )));
        }
        if std::fs::metadata(&repo)?.dev() != std::fs::metadata(&data_dir)?.dev() {
            return Ok(PastureSupport::Unsupported(
                "Pastures need the project on the same volume as agentZ's data.".into(),
            ));
        }
        Ok(PastureSupport::CopyOnWrite)
    })
    .await;
    match check {
        Ok(Ok(support)) => support,
        Ok(Err(error)) => PastureSupport::Unsupported(format!("{error:#}")),
        Err(error) => PastureSupport::Unsupported(error.to_string()),
    }
}

/// The file system's type, from `statfs(2)`.
#[cfg(target_os = "macos")]
fn file_system_name(path: &Path) -> Result<String> {
    use std::ffi::{CStr, CString};
    use std::os::unix::ffi::OsStrExt as _;

    let path_c = CString::new(path.as_os_str().as_bytes()).context("path")?;
    // SAFETY: `statfs` fills the zeroed struct, which is plain data, for a valid path.
    let mut stats: libc::statfs = unsafe { std::mem::zeroed() };
    let result = unsafe { libc::statfs(path_c.as_ptr(), &mut stats) };
    if result != 0 {
        return Err(anyhow!(
            "reading {}'s file system: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    // SAFETY: `f_fstypename` is NUL-terminated by `statfs`.
    let name = unsafe { CStr::from_ptr(stats.f_fstypename.as_ptr()) };
    Ok(name.to_string_lossy().into_owned())
}

/// Reflinks only work within one file system; elsewhere cow copies in full.
#[cfg(not(target_os = "macos"))]
#[allow(
    clippy::disallowed_methods,
    reason = "the probe's copy runs on a blocking thread"
)]
async fn pasture_support(repo: &Path, data_dir: &Path) -> PastureSupport {
    use std::os::unix::fs::MetadataExt as _;

    let (repo, data_dir) = (repo.to_path_buf(), data_dir.to_path_buf());
    let check = tokio::task::spawn_blocking(move || -> Result<PastureSupport> {
        std::fs::create_dir_all(&data_dir)?;
        if std::fs::metadata(&repo)?.dev() != std::fs::metadata(&data_dir)?.dev() {
            return Ok(PastureSupport::FullCopy);
        }
        let probe = data_dir.join(format!(".reflink-probe-{}", short_id()));
        std::fs::write(&probe, b"agentZ")?;
        let copy = probe.with_extension("copy");
        let reflinked = std::process::Command::new("cp")
            .arg("--reflink=always")
            .arg(&probe)
            .arg(&copy)
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        std::fs::remove_file(&probe).ok();
        std::fs::remove_file(&copy).ok();
        Ok(if reflinked {
            PastureSupport::CopyOnWrite
        } else {
            PastureSupport::FullCopy
        })
    })
    .await;
    match check {
        Ok(Ok(support)) => support,
        Ok(Err(error)) => PastureSupport::Unsupported(format!("{error:#}")),
        Err(error) => PastureSupport::Unsupported(error.to_string()),
    }
}

pub(crate) async fn head_commit(folder: &Path) -> Option<String> {
    let output = git(folder, &["rev-parse", "--verify", "--quiet", "HEAD"], &[])
        .await
        .ok()?;
    Some(output.trim().to_string())
}

pub(crate) async fn has_commit(repo: &Path, commit: &str) -> bool {
    git(
        repo,
        &["cat-file", "-e", &format!("{commit}^{{commit}}")],
        &[],
    )
    .await
    .is_ok()
}

/// For a pasture no project keeps, whose project can't say which commits are its own: those no
/// remote has.
pub(crate) async fn has_unpushed_commits(folder: &Path) -> bool {
    git(
        folder,
        &["rev-list", "--count", "HEAD", "--not", "--remotes"],
        &[],
    )
    .await
    .ok()
    .and_then(|count| count.trim().parse::<u32>().ok())
    .is_some_and(|count| count > 0)
}

/// The data folder's folder for the kind, which keeps one folder per repository.
pub(crate) fn folder_name(kind: WorkspaceKind) -> &'static str {
    match kind {
        WorkspaceKind::Worktree => "worktrees",
        WorkspaceKind::Pasture => "pastures",
    }
}

async fn conflicted_files(folder: &Path) -> Vec<String> {
    git(folder, &["diff", "--name-only", "--diff-filter=U"], &[])
        .await
        .map(|output| output.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

fn conflict_error(error: anyhow::Error, action: &str, conflicts: &[String]) -> anyhow::Error {
    if conflicts.is_empty() {
        return error.context(format!("the {action} failed and was undone"));
    }
    anyhow!(
        "the {action} had conflicts in {} and was undone",
        conflicts.join(", ")
    )
}

async fn remove_folder(path: &Path) -> Result<()> {
    tokio::fs::remove_dir_all(path)
        .await
        .with_context(|| format!("removing {}", path.display()))
}

fn short_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..8].to_string()
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn run(cwd: &Path, args: &[&str]) -> String {
        let env = [
            ("GIT_AUTHOR_NAME", "test"),
            ("GIT_AUTHOR_EMAIL", "test@example.com"),
            ("GIT_COMMITTER_NAME", "test"),
            ("GIT_COMMITTER_EMAIL", "test@example.com"),
        ];
        git(cwd, args, &env).await.expect("git runs")
    }

    async fn repository(root: &Path) -> PathBuf {
        let repo = root.join("demo");
        std::fs::create_dir_all(&repo).expect("create");
        run(&repo, &["init", "-q", "-b", "main"]).await;
        std::fs::write(repo.join("README.md"), "# Demo\n").expect("write");
        std::fs::write(repo.join(".gitignore"), ".env\nserver.pid\ntarget/\n").expect("write");
        run(&repo, &["add", "-A"]).await;
        run(&repo, &["commit", "-q", "-m", "init"]).await;
        repo
    }

    #[test]
    fn worktree_lists_are_read_as_herdr_reads_them() {
        let output = "\
worktree /repo/main
HEAD abc
branch refs/heads/main

worktree /repo/issue
HEAD def
branch refs/heads/agentz/issue

worktree /repo/detached
HEAD fed
detached

worktree /repo/gone
HEAD 123
branch refs/heads/gone
prunable gitdir file points to non-existent location
";
        assert_eq!(
            parse_worktree_list(output),
            vec![
                Checkout {
                    path: PathBuf::from("/repo/main"),
                    branch: Some("main".into()),
                    kind: None,
                },
                Checkout {
                    path: PathBuf::from("/repo/issue"),
                    branch: Some("agentz/issue".into()),
                    kind: Some(WorkspaceKind::Worktree),
                },
                Checkout {
                    path: PathBuf::from("/repo/detached"),
                    branch: None,
                    kind: Some(WorkspaceKind::Worktree),
                },
            ]
        );
    }

    #[tokio::test]
    async fn worktrees_are_made_and_removed() {
        let root = tempfile::tempdir().expect("temp dir");
        let repo = repository(root.path()).await;
        let data_dir = root.path().join("data");
        let workspace = create(NewWorkspace {
            kind: WorkspaceKind::Worktree,
            repo: repo.clone(),
            data_dir: data_dir.clone(),
            base: None,
            branch: Some("feature/login".into()),
            submodules: true,
        })
        .await
        .expect("creates");
        assert_eq!(
            workspace.path,
            data_dir
                .canonicalize()
                .expect("resolves")
                .join("worktrees/demo/feature-login")
        );
        assert_eq!(workspace.base.as_deref(), Some("main"));
        assert!(workspace.path.join("README.md").exists());
        assert_eq!(
            current_branch(&workspace.path)
                .await
                .expect("branch")
                .as_deref(),
            Some("feature/login")
        );

        let taken = create(NewWorkspace {
            kind: WorkspaceKind::Worktree,
            repo: repo.clone(),
            data_dir: data_dir.clone(),
            base: None,
            branch: Some("feature/login".into()),
            submodules: true,
        })
        .await;
        assert!(taken.is_err(), "the branch exists");

        std::fs::write(workspace.path.join("notes.txt"), "draft\n").expect("write");
        let removal = remove(&repo, &workspace, false).await.expect("removes");
        assert!(matches!(removal, WorkspaceRemoval::NeedsConfirmation(_)));
        assert!(workspace.path.exists());
        let removal = remove(&repo, &workspace, true).await.expect("removes");
        assert_eq!(removal, WorkspaceRemoval::Removed);
        assert!(!workspace.path.exists());
        assert!(
            local_branches(&repo)
                .await
                .expect("branches")
                .contains(&"feature/login".into()),
            "branches are kept"
        );
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn pastures_clone_everything_and_bring_work_back() {
        let root = tempfile::tempdir().expect("temp dir");
        let repo = repository(root.path()).await;
        let data_dir = root.path().join("data");
        std::fs::write(repo.join(".env"), "SECRET=1\n").expect("write");
        std::fs::write(repo.join("server.pid"), "123\n").expect("write");
        std::fs::create_dir_all(repo.join("target/debug")).expect("create");
        std::fs::write(repo.join("target/debug/app"), "binary").expect("write");
        std::fs::create_dir_all(repo.join("dist")).expect("create");
        std::fs::write(
            repo.join(".cow.json"),
            r#"{"post_clone": {"remove": ["dist"], "run": ["touch ready"]}}"#,
        )
        .expect("write");
        run(&repo, &["worktree", "add", "-q", "-b", "other", "../other"]).await;

        assert_eq!(
            project_git(&repo, &data_dir).await.pastures,
            PastureSupport::CopyOnWrite
        );
        let pasture = create(NewWorkspace {
            kind: WorkspaceKind::Pasture,
            repo: repo.clone(),
            data_dir: data_dir.clone(),
            base: Some("main".into()),
            branch: None,
            submodules: true,
        })
        .await
        .expect("creates");
        let path = &pasture.path;
        let branch = pasture.branch.clone().expect("branch");
        assert!(branch.starts_with("agentz/"));
        assert_eq!(
            current_branch(path).await.expect("branch"),
            Some(branch.clone())
        );
        assert!(path.join(".env").exists(), "untracked files come along");
        assert!(
            !path.join("server.pid").exists(),
            "runtime files are removed"
        );
        assert!(!path.join("target").exists(), "build output is skipped");
        assert!(!path.join("dist").exists(), ".cow.json removes");
        assert!(path.join("ready").exists(), ".cow.json runs");
        assert!(!path.join(".git/worktrees").exists());
        let worktrees = run(path, &["worktree", "list"]).await;
        assert_eq!(worktrees.lines().count(), 1, "{worktrees}");

        // The project moves on; the pasture commits.
        std::fs::write(repo.join("CHANGELOG.md"), "# Changes\n").expect("write");
        run(&repo, &["add", "CHANGELOG.md"]).await;
        run(&repo, &["commit", "-q", "-m", "changelog"]).await;
        std::fs::write(path.join("feature.txt"), "new\n").expect("write");
        run(path, &["add", "feature.txt"]).await;
        run(path, &["commit", "-q", "-m", "feature"]).await;

        let removal = remove(&repo, &pasture, false).await.expect("checks");
        assert!(matches!(removal, WorkspaceRemoval::NeedsConfirmation(_)));

        sync(&repo, path, "main", false).await.expect("syncs");
        assert!(path.join("CHANGELOG.md").exists());
        bring_back(&repo, path, &branch).await.expect("brings back");
        let log = run(&repo, &["log", "--format=%s", &branch]).await;
        assert_eq!(
            log.lines().collect::<Vec<_>>(),
            ["feature", "changelog", "init"]
        );

        // Untracked files left from the clone.
        std::fs::remove_file(path.join("ready")).expect("remove");
        std::fs::remove_file(path.join(".cow.json")).expect("remove");
        let removal = remove(&repo, &pasture, false).await.expect("removes");
        assert_eq!(removal, WorkspaceRemoval::Removed);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn sync_undoes_conflicts() {
        let root = tempfile::tempdir().expect("temp dir");
        let repo = repository(root.path()).await;
        // A second clone stands in for a pasture, which works the same on any file system.
        let pasture = root.path().join("pasture");
        run(
            root.path(),
            &[
                "clone",
                "-q",
                &repo.to_string_lossy(),
                &pasture.to_string_lossy(),
            ],
        )
        .await;
        std::fs::write(repo.join("README.md"), "# Project\n").expect("write");
        run(&repo, &["commit", "-q", "-am", "project"]).await;
        std::fs::write(pasture.join("README.md"), "# Pasture\n").expect("write");
        run(&pasture, &["commit", "-q", "-am", "pasture"]).await;

        let error = sync(&repo, &pasture, "main", false)
            .await
            .expect_err("conflicts");
        assert!(format!("{error:#}").contains("README.md"), "{error:#}");
        assert_eq!(
            std::fs::read_to_string(pasture.join("README.md")).expect("read"),
            "# Pasture\n"
        );
        assert!(run(&pasture, &["status", "--porcelain"]).await.is_empty());
    }

    #[test]
    fn branches_are_named_after_titles_as_t3code_names_them() {
        assert_eq!(
            branch_for_title("Fix the login redirect!"),
            "agentz/fix-the-login-redirect"
        );
        assert_eq!(
            branch_for_title("  \"Quoted\" title / API  "),
            "agentz/quoted-title-/-api"
        );
        assert_eq!(branch_for_title("Émoji 🚀 time"), "agentz/moji-time");
        assert_eq!(branch_for_title("…"), "agentz/update");
        let long = branch_for_title(&"word ".repeat(30));
        assert_eq!(long.len(), "agentz/".len() + 64);
        let cut_at_a_dash = branch_for_title(&format!("{}-tail", "a".repeat(63)));
        assert_eq!(cut_at_a_dash, format!("agentz/{}", "a".repeat(63)));
    }

    #[tokio::test]
    async fn origin_branches_follow_local_ones_and_are_fetched() {
        let root = tempfile::tempdir().expect("temp dir");
        let upstream = repository(root.path()).await;
        let origin = root.path().join("origin.git");
        let project = root.path().join("project");
        run(
            root.path(),
            &[
                "clone",
                "-q",
                "--bare",
                &upstream.to_string_lossy(),
                &origin.to_string_lossy(),
            ],
        )
        .await;
        run(
            root.path(),
            &[
                "clone",
                "-q",
                &origin.to_string_lossy(),
                &project.to_string_lossy(),
            ],
        )
        .await;
        run(&project, &["branch", "-q", "local-only"]).await;
        let data_dir = root.path().join("data");

        let listed = project_git(&project, &data_dir).await;
        assert!(listed.has_origin);
        assert_eq!(listed.default_branch.as_deref(), Some("main"));
        let branches: Vec<(&str, Option<&str>)> = listed
            .branches
            .iter()
            .map(|branch| (branch.name.as_str(), listed.mark(branch)))
            .collect();
        assert_eq!(
            branches
                .iter()
                .filter(|(_, mark)| *mark == Some("remote"))
                .count(),
            1
        );
        assert!(branches.contains(&("main", Some("current"))));
        assert!(branches.contains(&("local-only", None)));
        assert_eq!(branches.last(), Some(&("origin/main", Some("remote"))));

        // A teammate pushes a branch and a commit to main.
        run(&upstream, &["checkout", "-q", "-b", "teammate"]).await;
        run(
            &upstream,
            &["commit", "-q", "--allow-empty", "-m", "theirs"],
        )
        .await;
        run(&upstream, &["checkout", "-q", "main"]).await;
        std::fs::write(upstream.join("NEWS.md"), "news\n").expect("write");
        run(&upstream, &["add", "NEWS.md"]).await;
        run(&upstream, &["commit", "-q", "-m", "news"]).await;
        let origin_path = origin.to_string_lossy();
        run(&upstream, &["push", "-q", &origin_path, "teammate", "main"]).await;

        fetch_base(&project, "main").await.expect("fetches main");
        let listed = project_git(&project, &data_dir).await;
        assert!(
            !listed
                .branches
                .iter()
                .any(|branch| branch.name == "origin/teammate"),
            "only main was fetched"
        );
        let workspace = create(NewWorkspace {
            kind: WorkspaceKind::Worktree,
            repo: project.clone(),
            data_dir: data_dir.clone(),
            base: Some("origin/main".into()),
            branch: None,
            submodules: true,
        })
        .await
        .expect("creates");
        assert!(workspace.path.join("NEWS.md").exists());
        let upstream_branch = git(
            &workspace.path,
            &["rev-parse", "--abbrev-ref", "@{upstream}"],
            &[],
        )
        .await;
        assert!(upstream_branch.is_err(), "it doesn't track origin's main");

        fetch_origin(&project).await.expect("fetches");
        let listed = project_git(&project, &data_dir).await;
        assert!(
            listed
                .branches
                .iter()
                .any(|branch| branch.name == "origin/teammate" && branch.is_remote)
        );
        assert!(listed.branches.iter().any(|branch| branch.name
            == workspace.branch.clone().expect("a branch")
            && listed.mark(branch) == Some("worktree")));
    }

    #[tokio::test]
    async fn branches_are_renamed_unless_the_name_is_taken() {
        let root = tempfile::tempdir().expect("temp dir");
        let repo = repository(root.path()).await;
        let data_dir = root.path().join("data");
        let new = || NewWorkspace {
            kind: WorkspaceKind::Worktree,
            repo: repo.clone(),
            data_dir: data_dir.clone(),
            base: None,
            branch: None,
            submodules: true,
        };
        let first = create(new()).await.expect("creates");
        let second = create(new()).await.expect("creates");
        let temporary = second.branch.clone().expect("a branch");
        assert!(temporary.starts_with("agentz/"));
        let renamed = rename_branch(
            &repo,
            &first.path,
            &first.branch.clone().expect("a branch"),
            "agentz/fix-login",
        )
        .await
        .expect("renames");
        assert!(renamed);
        assert_eq!(
            current_branch(&first.path)
                .await
                .expect("branch")
                .as_deref(),
            Some("agentz/fix-login")
        );
        let renamed = rename_branch(&repo, &second.path, &temporary, "agentz/fix-login")
            .await
            .expect("checks");
        assert!(!renamed, "the name is taken");
        assert_eq!(
            current_branch(&second.path).await.expect("branch"),
            Some(temporary)
        );
    }

    #[tokio::test]
    async fn copies_switch_to_their_base_leaving_out_what_does_not_fit() {
        let root = tempfile::tempdir().expect("temp dir");
        let repo = repository(root.path()).await;
        run(&repo, &["checkout", "-q", "-b", "other"]).await;
        std::fs::write(repo.join("README.md"), "# Other\n").expect("write");
        std::fs::write(repo.join("added.txt"), "theirs\n").expect("write");
        run(&repo, &["add", "-A"]).await;
        run(&repo, &["commit", "-q", "-m", "other"]).await;
        run(&repo, &["checkout", "-q", "main"]).await;
        // A copy of a project with changes that don't fit `other`, and one that does.
        std::fs::write(repo.join("README.md"), "# Mine\n").expect("write");
        std::fs::write(repo.join("added.txt"), "mine\n").expect("write");
        std::fs::write(repo.join(".gitignore"), ".env\n").expect("write");

        switch_copy(&repo, "agentz/copy", "other")
            .await
            .expect("switches");
        assert_eq!(
            current_branch(&repo).await.expect("branch").as_deref(),
            Some("agentz/copy")
        );
        assert_eq!(
            std::fs::read_to_string(repo.join("README.md")).expect("read"),
            "# Other\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.join("added.txt")).expect("read"),
            "theirs\n"
        );
        assert_eq!(
            std::fs::read_to_string(repo.join(".gitignore")).expect("read"),
            ".env\n",
            "a change that fits is kept"
        );
    }

    #[test]
    fn patterns_expand_one_folder_at_a_time() {
        let root = tempfile::tempdir().expect("temp dir");
        for folder in ["apps/web/dist", "apps/api/dist", "apps/api/src"] {
            std::fs::create_dir_all(root.path().join(folder)).expect("create");
        }
        let mut matches = expand_pattern(root.path(), "apps/*/dist").expect("expands");
        matches.sort();
        assert_eq!(
            matches,
            vec![
                root.path().join("apps/api/dist"),
                root.path().join("apps/web/dist")
            ]
        );
        assert!(expand_pattern(root.path(), "../outside").is_err());
    }
}
