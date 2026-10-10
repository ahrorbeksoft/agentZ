//! Measures what agentZ keeps on its machine, for Settings › Storage (design/storage), as `du`
//! counts it: the room files take on disk, hard links once, symbolic links not followed.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use agentz_protocol::agents::AgentId;
use agentz_protocol::storage::{
    AgentStorage, CheckoutStorage, NodeStorage, Storage, ThreadStorage,
};
use projects::{ProjectId, ThreadId, Workspace, WorkspaceKind};
use util::ResultExt as _;

use crate::git::git;
use crate::workspaces;

/// Reads mustn't take git's index lock from under the user's own commands.
const NO_LOCKS: [(&str, &str); 1] = [("GIT_OPTIONAL_LOCKS", "0")];

/// What to measure, from the server's state.
pub(crate) struct StoragePlan {
    pub data_dir: PathBuf,
    /// The data folder as the page shows it.
    pub data_folder: String,
    /// Each thread the thread lists show, with the files and folders it and its subthreads
    /// keep.
    pub threads: Vec<(ThreadId, Vec<PathBuf>)>,
    pub checkouts: Vec<ProjectCheckout>,
    pub agents: Vec<(AgentId, Vec<PathBuf>)>,
    pub registry_cache: Vec<PathBuf>,
    pub node_dir: PathBuf,
    pub server_log: PathBuf,
    /// The last measurement, whose checkouts and agents keep their sizes rather than being
    /// walked again: they're the slow part, and change little between full measurements.
    pub previous: Option<Storage>,
}

/// A worktree or pasture a project keeps.
pub(crate) struct ProjectCheckout {
    pub project_id: ProjectId,
    pub project_folder: PathBuf,
    pub workspace: Workspace,
}

pub(crate) async fn measure(plan: StoragePlan) -> Storage {
    let StoragePlan {
        data_dir,
        data_folder,
        threads,
        checkouts: project_checkouts,
        agents,
        registry_cache,
        node_dir,
        server_log,
        previous,
    } = plan;
    let known: Vec<PathBuf> = project_checkouts
        .iter()
        .map(|checkout| checkout.workspace.path.clone())
        .collect();
    let left = blocking(move || left_checkouts(&data_dir, &known)).await;

    let mut checkouts = Vec::new();
    for checkout in project_checkouts {
        let path = checkout.workspace.path;
        // A folder deleted outside agentZ takes no room.
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            continue;
        }
        let repository = folder_name(&checkout.project_folder);
        let mut storage = CheckoutStorage {
            path,
            kind: checkout.workspace.kind,
            project_id: Some(checkout.project_id),
            repository,
            branch: checkout.workspace.branch,
            bytes: 0,
            changed_files: 0,
            has_own_commits: false,
        };
        read_checkout(&mut storage, Some(&checkout.project_folder)).await;
        checkouts.push(storage);
    }
    for (path, kind, repository) in left {
        let mut storage = CheckoutStorage {
            path,
            kind,
            project_id: None,
            repository,
            branch: None,
            bytes: 0,
            changed_files: 0,
            has_own_commits: false,
        };
        read_checkout(&mut storage, None).await;
        checkouts.push(storage);
    }

    let (checkout_sizes, agent_sizes): (HashMap<PathBuf, u64>, HashMap<AgentId, u64>) =
        match &previous {
            Some(previous) => (
                previous
                    .checkouts
                    .iter()
                    .map(|checkout| (checkout.path.clone(), checkout.bytes))
                    .collect(),
                previous
                    .agents
                    .iter()
                    .map(|agent| (agent.agent_id.clone(), agent.bytes))
                    .collect(),
            ),
            None => Default::default(),
        };
    blocking(move || {
        let threads = threads
            .into_iter()
            .map(|(thread_id, paths)| ThreadStorage {
                thread_id,
                bytes: paths.iter().map(|path| disk_usage(path)).sum(),
            })
            .filter(|thread| thread.bytes > 0)
            .collect();
        for checkout in &mut checkouts {
            checkout.bytes = match checkout_sizes.get(&checkout.path) {
                Some(bytes) => *bytes,
                None => disk_usage(&checkout.path),
            };
        }
        let agents = agents
            .into_iter()
            .map(|(agent_id, folders)| {
                let bytes = match agent_sizes.get(&agent_id) {
                    Some(bytes) => *bytes,
                    None => folders.iter().map(|folder| disk_usage(folder)).sum(),
                };
                AgentStorage { agent_id, bytes }
            })
            .collect();
        Storage {
            data_folder,
            measured: true,
            threads,
            checkouts,
            agents,
            registry_cache: registry_cache.iter().map(|path| disk_usage(path)).sum(),
            node: downloaded_node(&node_dir),
            server_log: std::fs::metadata(&server_log)
                .map(|metadata| metadata.len())
                .unwrap_or_default(),
        }
    })
    .await
}

async fn blocking<T: Default + Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    tokio::task::spawn_blocking(work)
        .await
        .log_err()
        .unwrap_or_default()
}

/// Its branch now, and whether deleting it would lose work: changed or new files, and for a
/// pasture, commits only it has.
async fn read_checkout(checkout: &mut CheckoutStorage, project_folder: Option<&Path>) {
    let path = checkout.path.clone();
    if let Ok(Some(branch)) = workspaces::current_branch(&path).await {
        checkout.branch = Some(branch);
    }
    checkout.changed_files = git(&path, &["status", "--porcelain"], &NO_LOCKS)
        .await
        .map(|status| status.lines().count() as u32)
        .unwrap_or_default();
    checkout.has_own_commits = match (checkout.kind, project_folder) {
        // Its branch is the project's, and stays when it's deleted.
        (WorkspaceKind::Worktree, _) => false,
        (WorkspaceKind::Pasture, Some(project_folder)) => {
            match workspaces::head_commit(&path).await {
                Some(head) => !workspaces::has_commit(project_folder, &head).await,
                None => false,
            }
        }
        (WorkspaceKind::Pasture, None) => workspaces::has_unpushed_commits(&path).await,
    };
}

/// Worktrees and pastures in the data folder no project keeps: a Workspaces thread's, or a
/// removed project's. Each is `<kind>/<repository>/<branch>`.
fn left_checkouts(data_dir: &Path, known: &[PathBuf]) -> Vec<(PathBuf, WorkspaceKind, String)> {
    let mut left = Vec::new();
    for kind in [WorkspaceKind::Worktree, WorkspaceKind::Pasture] {
        let Ok(repositories) = std::fs::read_dir(data_dir.join(workspaces::folder_name(kind)))
        else {
            continue;
        };
        for repository in repositories.flatten() {
            if !repository.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let Ok(checkouts) = std::fs::read_dir(repository.path()) else {
                continue;
            };
            let name = repository.file_name().to_string_lossy().into_owned();
            for checkout in checkouts.flatten() {
                if !checkout.file_type().is_ok_and(|kind| kind.is_dir()) {
                    continue;
                }
                // Projects keep their workspaces' paths resolved.
                let path =
                    std::fs::canonicalize(checkout.path()).unwrap_or_else(|_| checkout.path());
                if !known.contains(&path) {
                    left.push((path, kind, name.clone()));
                }
            }
        }
    }
    left
}

/// Node.js downloaded into `<dir>/<version>/`.
fn downloaded_node(dir: &Path) -> Option<NodeStorage> {
    let version = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .find(|name| name.starts_with('v'))?;
    Some(NodeStorage {
        version,
        bytes: disk_usage(dir),
        in_use: false,
    })
}

fn folder_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// The room `path` takes on disk with everything in it. What can't be read counts as nothing.
pub(crate) fn disk_usage(path: &Path) -> u64 {
    let mut linked = HashSet::new();
    let mut total = 0;
    let mut pending = vec![path.to_path_buf()];
    while let Some(path) = pending.pop() {
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        total += allocated(&metadata, &mut linked);
        if metadata.is_dir()
            && let Ok(entries) = std::fs::read_dir(&path)
        {
            pending.extend(entries.flatten().map(|entry| entry.path()));
        }
    }
    total
}

#[cfg(unix)]
fn allocated(metadata: &std::fs::Metadata, linked: &mut HashSet<(u64, u64)>) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    if !metadata.is_dir()
        && metadata.nlink() > 1
        && !linked.insert((metadata.dev(), metadata.ino()))
    {
        return 0;
    }
    // In 512-byte units whatever the file system's block size.
    metadata.blocks() * 512
}

#[cfg(not(unix))]
fn allocated(metadata: &std::fs::Metadata, _linked: &mut HashSet<(u64, u64)>) -> u64 {
    metadata.len()
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;

    const IDENTITY: [(&str, &str); 4] = [
        ("GIT_AUTHOR_NAME", "t"),
        ("GIT_AUTHOR_EMAIL", "t@example.com"),
        ("GIT_COMMITTER_NAME", "t"),
        ("GIT_COMMITTER_EMAIL", "t@example.com"),
    ];

    fn write(path: &Path, bytes: usize) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("folder");
        }
        std::fs::write(path, vec![b'x'; bytes]).expect("write");
    }

    async fn commit_all(folder: &Path, message: &str) {
        git(folder, &["add", "-A"], &IDENTITY).await.expect("add");
        git(folder, &["commit", "-q", "-m", message], &IDENTITY)
            .await
            .expect("commit");
    }

    #[cfg(unix)]
    #[test]
    fn hard_links_count_once_and_symbolic_links_are_not_followed() {
        let dir = tempfile::tempdir().expect("temporary folder");
        let root = dir.path().join("measured");
        write(&root.join("a.bin"), 64 * 1024);
        let one_file = disk_usage(&root.join("a.bin"));
        assert!(one_file >= 64 * 1024, "{one_file}");
        std::fs::hard_link(root.join("a.bin"), root.join("b.bin")).expect("hard link");
        write(&dir.path().join("outside.bin"), 256 * 1024);
        std::os::unix::fs::symlink(dir.path().join("outside.bin"), root.join("c.bin"))
            .expect("symbolic link");
        std::os::unix::fs::symlink(dir.path(), root.join("loop")).expect("symbolic link");
        let total = disk_usage(&root);
        assert!(total >= one_file && total < one_file + 64 * 1024, "{total}");
        assert_eq!(disk_usage(&dir.path().join("missing")), 0);
    }

    #[tokio::test]
    async fn threads_checkouts_and_caches_are_measured() {
        let dir = tempfile::tempdir().expect("temporary folder");
        let data_dir = std::fs::canonicalize(dir.path()).expect("resolved");
        write(&data_dir.join("transcripts/1.json"), 10_000);
        write(&data_dir.join("attachments/1/image"), 20_000);
        write(&data_dir.join("attachments/2/image"), 30_000);
        write(&data_dir.join("chats/3/notes.md"), 40_000);
        write(&data_dir.join("registry/registry.json"), 1_000);
        write(
            &data_dir.join("registry/npx/codex/node_modules/a.js"),
            50_000,
        );
        write(
            &data_dir.join("node/v24.11.0/node-v24.11.0-linux-x64/bin/node"),
            60_000,
        );
        write(&data_dir.join("logs/server.log"), 1_234);

        // The project, a pasture of it with a commit of its own, and a pasture left behind.
        let project = data_dir.join("storefront");
        std::fs::create_dir_all(&project).expect("folder");
        git(&project, &["init", "-q", "-b", "main"], &IDENTITY)
            .await
            .expect("init");
        write(&project.join("README.md"), 10);
        commit_all(&project, "Start").await;
        let pasture = data_dir.join("pastures/storefront/checkout-flow");
        let left = data_dir.join("pastures/storefront/old-search");
        for copy in [&pasture, &left] {
            let project_arg = project.to_string_lossy();
            let copy_arg = copy.to_string_lossy();
            git(&data_dir, &["clone", "-q", &project_arg, &copy_arg], &[])
                .await
                .expect("clone");
        }
        write(&pasture.join("checkout.rs"), 100);
        commit_all(&pasture, "Add checkout").await;
        write(&pasture.join("draft.rs"), 100);

        let plan = StoragePlan {
            data_dir: data_dir.clone(),
            data_folder: "~/.agentz".into(),
            threads: vec![
                (
                    ThreadId(1),
                    vec![
                        data_dir.join("transcripts/1.json"),
                        data_dir.join("handoffs/1.json"),
                        data_dir.join("attachments/1"),
                        // A subthread's.
                        data_dir.join("attachments/2"),
                    ],
                ),
                (ThreadId(3), vec![data_dir.join("chats/3")]),
                (ThreadId(4), vec![data_dir.join("transcripts/4.json")]),
            ],
            checkouts: vec![ProjectCheckout {
                project_id: ProjectId(1),
                project_folder: project.clone(),
                workspace: Workspace {
                    kind: WorkspaceKind::Pasture,
                    path: pasture.clone(),
                    branch: Some("checkout-flow".into()),
                    base: None,
                    created_at: SystemTime::now(),
                },
            }],
            agents: vec![(
                AgentId::new("codex"),
                vec![
                    data_dir.join("registry/codex"),
                    data_dir.join("registry/npx/codex"),
                ],
            )],
            registry_cache: vec![
                data_dir.join("registry/registry.json"),
                data_dir.join("registry/icons"),
            ],
            node_dir: data_dir.join("node"),
            server_log: data_dir.join("logs/server.log"),
            previous: None,
        };
        let storage = measure(plan).await;

        assert!(storage.measured);
        assert_eq!(storage.data_folder, "~/.agentz");
        let threads: Vec<ThreadId> = storage.threads.iter().map(|t| t.thread_id).collect();
        // Nothing was kept for thread 4.
        assert_eq!(threads, [ThreadId(1), ThreadId(3)]);
        assert!(storage.threads[0].bytes >= 60_000, "{:?}", storage.threads);
        assert!(storage.threads[1].bytes >= 40_000, "{:?}", storage.threads);

        assert_eq!(storage.checkouts.len(), 2, "{:?}", storage.checkouts);
        let kept = &storage.checkouts[0];
        assert_eq!(kept.path, pasture);
        assert_eq!(kept.project_id, Some(ProjectId(1)));
        assert_eq!(kept.repository, "storefront");
        assert_eq!(kept.branch.as_deref(), Some("main"));
        assert_eq!((kept.changed_files, kept.has_own_commits), (1, true));
        assert!(kept.has_changes() && kept.bytes > 0);
        let left_behind = &storage.checkouts[1];
        assert_eq!(left_behind.path, left);
        assert_eq!(left_behind.project_id, None);
        assert_eq!(left_behind.kind, WorkspaceKind::Pasture);
        assert_eq!(left_behind.repository, "storefront");
        // origin has every commit it has.
        assert!(!left_behind.has_changes(), "{left_behind:?}");

        assert_eq!(storage.agents.len(), 1);
        assert!(storage.agents[0].bytes >= 50_000);
        assert!(storage.registry_cache >= 1_000 && storage.registry_cache < 50_000);
        let node = storage.node.as_ref().expect("downloaded Node.js");
        assert_eq!(node.version, "v24.11.0");
        assert!(node.bytes >= 60_000 && !node.in_use);
        assert_eq!(storage.server_log, 1_234);

        // A light measurement keeps the checkouts' and agents' sizes.
        let previous = Storage {
            checkouts: vec![CheckoutStorage {
                bytes: 7,
                ..kept.clone()
            }],
            agents: vec![AgentStorage {
                agent_id: AgentId::new("codex"),
                bytes: 9,
            }],
            ..Storage::default()
        };
        let light = measure(StoragePlan {
            data_dir: data_dir.clone(),
            data_folder: "~/.agentz".into(),
            threads: Vec::new(),
            checkouts: Vec::new(),
            agents: vec![(AgentId::new("codex"), Vec::new())],
            registry_cache: Vec::new(),
            node_dir: data_dir.join("node"),
            server_log: data_dir.join("logs/server.log"),
            previous: Some(previous),
        })
        .await;
        // The project no longer keeps it, so it's left behind.
        let kept_again = light
            .checkouts
            .iter()
            .find(|checkout| checkout.path == pasture)
            .expect("still there");
        assert_eq!((kept_again.bytes, kept_again.project_id), (7, None));
        assert_eq!(light.agents[0].bytes, 9);
    }
}
