//! Which repository each project is in, by its primary remote, so the app can show checkouts of
//! one repository (on any machine) as one project. Ported from t3code's
//! `RepositoryIdentityResolver` and `normalizeGitRemoteUrl`. Also the branch checked out in
//! each folder projects and threads work in.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow};
use projects::{GitHead, RepositoryIdentity};
use regex::Regex;

/// Every project is checked again after this, so a changed remote shows up eventually.
const FOUND_TTL: Duration = Duration::from_secs(15 * 60);
/// Short, so a folder that gains a repository or a remote shows up quickly.
const NOT_FOUND_TTL: Duration = Duration::from_secs(60);
const GIT_TIMEOUT: Duration = Duration::from_secs(10);

static REMOTE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\S+)\s+(\S+)\s+\((fetch|push)\)$").expect("valid remote line pattern")
});
static SCP_REMOTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[a-zA-Z0-9._-]+@([^:/\s]+):([^/\s]+(?:/[^/\s]+)+)$")
        .expect("valid scp remote pattern")
});
static URL_REMOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(?:ssh|https?|git)://").expect("valid url pattern"));

/// The identity of the repository `path` is in, or `None` outside git or without a remote. An
/// error means git couldn't tell (it couldn't start or took too long), not that there's no
/// repository.
pub(crate) async fn resolve(path: PathBuf) -> Result<Option<RepositoryIdentity>> {
    if !path.is_dir() {
        return Ok(None);
    }
    let Some(root) = git(&path, &["rev-parse", "--show-toplevel"]).await? else {
        return Ok(None);
    };
    let root = root.trim();
    if root.is_empty() {
        return Ok(None);
    }
    let Some(remotes) = git(Path::new(root), &["remote", "-v"]).await? else {
        return Ok(None);
    };
    Ok(primary_remote(&remote_fetch_urls(&remotes))
        .map(|(remote_name, remote_url)| identity(remote_name, remote_url, PathBuf::from(root))))
}

/// What git printed, or `None` when it answered with a failure, as it does outside a
/// repository.
async fn git(cwd: &Path, args: &[&str]) -> Result<Option<String>> {
    let output = tokio::time::timeout(GIT_TIMEOUT, crate::git::command(cwd, args, &[]).output())
        .await
        .map_err(|_| anyhow!("git {} took over {GIT_TIMEOUT:?}", args.join(" ")))?
        .context("running git")?;
    match output.status.code() {
        Some(0) => Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned())),
        Some(_) => Ok(None),
        None => Err(anyhow!("git {} was killed", args.join(" "))),
    }
}

/// Each remote's fetch URL, by name.
fn remote_fetch_urls(output: &str) -> BTreeMap<String, String> {
    let mut remotes = BTreeMap::new();
    for line in output.lines() {
        let Some(captures) = REMOTE_LINE.captures(line.trim()) else {
            continue;
        };
        if &captures[3] == "fetch" {
            remotes.insert(captures[1].to_string(), captures[2].to_string());
        }
    }
    remotes
}

/// `upstream`, then `origin`, then the first by name.
fn primary_remote(remotes: &BTreeMap<String, String>) -> Option<(String, String)> {
    ["upstream", "origin"]
        .into_iter()
        .find_map(|name| Some((name.to_string(), remotes.get(name)?.clone())))
        .or_else(|| {
            remotes
                .first_key_value()
                .map(|(name, url)| (name.clone(), url.clone()))
        })
}

fn identity(remote_name: String, remote_url: String, root_path: PathBuf) -> RepositoryIdentity {
    let canonical_key = normalize_git_remote_url(&remote_url);
    let repository_path = canonical_key
        .split('/')
        .skip(1)
        .collect::<Vec<_>>()
        .join("/");
    let segments: Vec<&str> = repository_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect();
    RepositoryIdentity {
        display_name: (!repository_path.is_empty()).then(|| repository_path.clone()),
        owner: segments.first().map(|owner| owner.to_string()),
        name: segments.last().map(|name| name.to_string()),
        canonical_key,
        root_path,
        remote_name,
        remote_url,
    }
}

/// A stable comparison key for a remote URL: `host/owner/repo`, lowercased, for both URL and
/// `user@host:path` forms.
pub(crate) fn normalize_git_remote_url(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    let trimmed = match trimmed.len().checked_sub(4) {
        Some(end) if trimmed[end..].eq_ignore_ascii_case(".git") => &trimmed[..end],
        _ => trimmed,
    };
    let normalized = trimmed.to_lowercase();

    if URL_REMOTE.is_match(&normalized) {
        let Ok(url) = url::Url::parse(&normalized) else {
            return normalized;
        };
        let segments: Vec<&str> = url
            .path()
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        if let Some(host) = url.host_str().filter(|host| !host.is_empty())
            && segments.len() > 1
        {
            return azure_devops_repository_key(host, &segments)
                .unwrap_or_else(|| format!("{host}/{}", segments.join("/")));
        }
    }

    if let Some(captures) = SCP_REMOTE.captures(&normalized) {
        let host = &captures[1];
        let path = &captures[2];
        let segments: Vec<&str> = path.split('/').collect();
        return azure_devops_repository_key(host, &segments)
            .unwrap_or_else(|| format!("{host}/{path}"));
    }

    normalized
}

/// Azure DevOps over SSH names a repository `ssh.dev.azure.com/v3/{org}/{project}/{repo}`, and
/// everywhere else `dev.azure.com/{org}/{project}/_git/{repo}`. Both are keyed by the second.
fn azure_devops_repository_key(host: &str, segments: &[&str]) -> Option<String> {
    if host != "ssh.dev.azure.com" && host != "vs-ssh.visualstudio.com" {
        return None;
    }
    let [marker, organization, project, repository] = segments else {
        return None;
    };
    if *marker != "v3" || organization.is_empty() || project.is_empty() || repository.is_empty() {
        return None;
    }
    Some(if host == "ssh.dev.azure.com" {
        format!("dev.azure.com/{organization}/{project}/_git/{repository}")
    } else {
        format!("{organization}.visualstudio.com/{project}/_git/{repository}")
    })
}

/// Reads HEAD from the repository containing `root`, which may be a subfolder of it. Only reads
/// files, so the server can read every folder's often.
pub(crate) fn read_git_head(root: &Path) -> Option<GitHead> {
    let (checkout, dot_git) = root.ancestors().find_map(|directory| {
        let dot_git = directory.join(".git");
        dot_git.exists().then(|| (directory.to_path_buf(), dot_git))
    })?;
    let (git_dir, worktree) = if dot_git.is_dir() {
        (dot_git, None)
    } else {
        // A linked worktree's `.git` is a file pointing at its git directory.
        let contents = std::fs::read_to_string(&dot_git).ok()?;
        let git_dir = PathBuf::from(contents.trim().strip_prefix("gitdir:")?.trim());
        (checkout.join(git_dir), Some(checkout))
    };
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    Some(GitHead {
        branch: branch_from_head(&head)?,
        worktree,
    })
}

fn branch_from_head(head: &str) -> Option<String> {
    let head = head.trim();
    if head.is_empty() {
        return None;
    }
    Some(match head.strip_prefix("ref:") {
        Some(reference) => {
            let reference = reference.trim();
            reference
                .strip_prefix("refs/heads/")
                .unwrap_or(reference)
                .to_string()
        }
        None => head.chars().take(7).collect(),
    })
}

/// When each project's folder was last checked, so the regular sweep only runs git when a
/// result is stale.
#[derive(Default)]
pub(crate) struct RepositoryChecks {
    checks: HashMap<PathBuf, Check>,
}

#[derive(Clone, Copy)]
enum Check {
    Running,
    Done { at: Instant, found: bool },
}

impl RepositoryChecks {
    /// Of `paths`, those whose last result is stale, marked as being checked.
    pub(crate) fn take_due(
        &mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        now: Instant,
    ) -> Vec<PathBuf> {
        let mut due = Vec::new();
        for path in paths {
            let is_due = match self.checks.get(&path) {
                None => true,
                Some(Check::Running) => false,
                Some(Check::Done { at, found }) => {
                    let ttl = if *found { FOUND_TTL } else { NOT_FOUND_TTL };
                    now.saturating_duration_since(*at) >= ttl
                }
            };
            if is_due {
                self.checks.insert(path.clone(), Check::Running);
                due.push(path);
            }
        }
        due
    }

    pub(crate) fn finish(&mut self, path: PathBuf, found: bool, now: Instant) {
        self.checks.insert(path, Check::Done { at: now, found });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches() {
        assert_eq!(
            branch_from_head("ref: refs/heads/main\n").as_deref(),
            Some("main")
        );
        assert_eq!(
            branch_from_head("ref: refs/heads/feature/sidebar").as_deref(),
            Some("feature/sidebar")
        );
        assert_eq!(
            branch_from_head("57bfce2945aa\n").as_deref(),
            Some("57bfce2")
        );
        assert_eq!(branch_from_head(""), None);
    }

    #[test]
    fn reads_this_repository() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(read_git_head(root).is_some());
    }

    #[test]
    fn normalizes_like_t3code() {
        for (remote, key) in [
            (
                "git@github.com:T3Tools/T3Code.git",
                "github.com/t3tools/t3code",
            ),
            (
                "https://github.com/T3Tools/T3Code.git",
                "github.com/t3tools/t3code",
            ),
            (
                "ssh://git@github.com/T3Tools/T3Code",
                "github.com/t3tools/t3code",
            ),
            (
                "git@gitlab.com:T3Tools/platform/T3Code.git",
                "gitlab.com/t3tools/platform/t3code",
            ),
            (
                "https://gitlab.com/T3Tools/platform/T3Code.git",
                "gitlab.com/t3tools/platform/t3code",
            ),
            (
                "https://gitlab.company.com:8443/team/project.git",
                "gitlab.company.com/team/project",
            ),
            (
                "ssh://git@gitlab.company.com:2222/team/project.git",
                "gitlab.company.com/team/project",
            ),
            (
                "gitlab@gitlab.example.com:group/project.git",
                "gitlab.example.com/group/project",
            ),
            (
                "deploy@bitbucket.org:workspace/repo.git",
                "bitbucket.org/workspace/repo",
            ),
            (
                "git@ssh.dev.azure.com:v3/T3Tools/Platform/T3Code",
                "dev.azure.com/t3tools/platform/_git/t3code",
            ),
            (
                "ssh://git@ssh.dev.azure.com:22/v3/T3Tools/Platform/T3Code",
                "dev.azure.com/t3tools/platform/_git/t3code",
            ),
            (
                "T3Tools@vs-ssh.visualstudio.com:v3/T3Tools/Platform/T3Code",
                "t3tools.visualstudio.com/platform/_git/t3code",
            ),
            (
                "https://T3Tools.visualstudio.com/Platform/_git/T3Code",
                "t3tools.visualstudio.com/platform/_git/t3code",
            ),
            (
                "git@ssh.dev.azure.com:v4/T3Tools/Platform/T3Code",
                "ssh.dev.azure.com/v4/t3tools/platform/t3code",
            ),
            (
                "git@ssh.dev.azure.com:v3/T3Tools/T3Code",
                "ssh.dev.azure.com/v3/t3tools/t3code",
            ),
            ("https://github.com/owner/repo/", "github.com/owner/repo"),
            ("/srv/git/repo.git", "/srv/git/repo"),
        ] {
            assert_eq!(normalize_git_remote_url(remote), key, "{remote}");
        }
    }

    #[test]
    fn picks_the_primary_remote() {
        let output = "fork\tgit@github.com:me/repo.git (fetch)\n\
                      fork\tgit@github.com:me/repo.git (push)\n\
                      origin\thttps://github.com/team/repo (fetch)\n\
                      origin\tno_push (push)\n";
        let remotes = remote_fetch_urls(output);
        assert_eq!(remotes.len(), 2);
        assert_eq!(
            primary_remote(&remotes),
            Some(("origin".into(), "https://github.com/team/repo".into()))
        );
        let only_forks = remote_fetch_urls("b\turl-b (fetch)\na\turl-a (fetch)\n");
        assert_eq!(
            primary_remote(&only_forks),
            Some(("a".into(), "url-a".into()))
        );
        assert_eq!(primary_remote(&BTreeMap::new()), None);

        let identity = identity(
            "origin".into(),
            "git@github.com:Team/Repo.git".into(),
            PathBuf::from("/code/repo"),
        );
        assert_eq!(identity.canonical_key, "github.com/team/repo");
        assert_eq!(identity.display_name.as_deref(), Some("team/repo"));
        assert_eq!(identity.owner.as_deref(), Some("team"));
        assert_eq!(identity.name.as_deref(), Some("repo"));
    }

    #[test]
    fn checks_again_once_stale() {
        let mut checks = RepositoryChecks::default();
        let start = Instant::now();
        let found = PathBuf::from("/found");
        let missing = PathBuf::from("/missing");
        let both = || [found.clone(), missing.clone()];
        assert_eq!(checks.take_due(both(), start), both());
        assert!(checks.take_due(both(), start).is_empty(), "still running");
        checks.finish(found.clone(), true, start);
        checks.finish(missing.clone(), false, start);
        assert!(
            checks
                .take_due(both(), start + NOT_FOUND_TTL / 2)
                .is_empty()
        );
        assert_eq!(
            checks.take_due(both(), start + NOT_FOUND_TTL),
            std::slice::from_ref(&missing)
        );
        assert_eq!(checks.take_due(both(), start + FOUND_TTL), [found]);
    }

    #[tokio::test]
    async fn resolves_a_checkout() {
        let folder = tempfile::tempdir().expect("temp dir");
        let repository = folder.path().join("repo");
        let nested = repository.join("crates/app");
        assert_eq!(
            resolve(repository.clone()).await.expect("resolves"),
            None,
            "not there"
        );
        std::fs::create_dir_all(&nested).expect("folders");
        assert_eq!(
            resolve(repository.clone()).await.expect("resolves"),
            None,
            "not a repository"
        );
        crate::git::git(&repository, &["init", "--quiet"], &[])
            .await
            .expect("git init");
        assert_eq!(
            resolve(repository.clone()).await.expect("resolves"),
            None,
            "no remote"
        );
        crate::git::git(
            &repository,
            &["remote", "add", "origin", "git@github.com:Owner/Repo.git"],
            &[],
        )
        .await
        .expect("adds the remote");
        let identity = resolve(nested)
            .await
            .expect("resolves")
            .expect("a repository");
        assert_eq!(identity.canonical_key, "github.com/owner/repo");
        assert_eq!(
            std::fs::canonicalize(&identity.root_path).expect("root"),
            std::fs::canonicalize(&repository).expect("repository")
        );
    }
}
