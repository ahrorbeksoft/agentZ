#![allow(dead_code)]

#[path = "../src/git.rs"]
mod git;
#[path = "../src/workspaces.rs"]
mod workspaces;
#[path = "../src/artifacts.rs"]
mod artifacts;
#[path = "../src/agent_settings.rs"]
mod agent_settings;

use std::{collections::BTreeMap, path::{Path, PathBuf}, sync::Arc, time::Duration};
use futures::FutureExt as _;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use projects::{ProjectId, ThreadId, WorkspaceKind};
use agentz_protocol::workspace::WorkspaceRemoval;

fn start_server(data_dir: &Path) -> agentz_server::ServerHandle {
    agentz_server::start(tokio::runtime::Handle::current(), agentz_server::ServerConfig {
        data_dir: data_dir.to_path_buf(), version: "audit".into(),
        http_client: Arc::new(http_client::BlockedHttpClient),
        shell_environment_ready: futures::future::ready(()).boxed().shared(),
        custom_agents: BTreeMap::new(), agent_control: None,
        hands_pages_to_clients: false, terminal_shell: Some("/bin/sh".into()),
        title_generation_path: None, cloud_path: None, keep_awake: Vec::new(),
        listener: None, handed_over: None,
    }).expect("starts the scratch server")
}

#[tokio::test(flavor = "multi_thread")]
async fn audit_corrupt_state_must_preserve_conversations_and_attachments() {
    let data = tempfile::tempdir().expect("data");
    std::fs::write(data.path().join("state.json"), b"{broken-json").expect("state");
    let transcript = data.path().join("transcripts/42.json");
    let attachment = data.path().join("attachments/42/precious.png");
    for path in [&transcript, &attachment] {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(path, b"precious user data").expect("file");
    }
    let server = start_server(data.path());
    tokio::time::sleep(Duration::from_millis(250)).await;
    server.shut_down();
    server.stopped().await;
    assert!(transcript.exists() && attachment.exists(),
        "corrupt state deleted user data: transcript={}, attachment={}", transcript.exists(), attachment.exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn audit_artifact_filename_must_not_remove_http_sandbox() {
    let data = tempfile::tempdir().expect("data");
    let malicious_name = "report\r\n\r\n<svg onload=alert(1)>\r\npreview.html";
    let mut store = artifacts::ArtifactStore::load(data.path());
    let (id, _) = store.publish(artifacts::Publish {
        id: None, title: "Audit fixture".into(), thread_id: ThreadId(42),
        project_id: ProjectId(1), agent_id: None,
        content: artifacts::Content::Document {
            source: "A file".into(), files: vec![(malicious_name.into(), vec![b'X';512])],
        },
    }).expect("publishes the fixture");
    let key = store.get(&id).expect("artifact").frame_key.clone();
    let server = start_server(data.path());
    let config: serde_json::Value = serde_json::from_slice(
        &std::fs::read(data.path().join("artifacts/server.json")).expect("pages config")
    ).expect("config json");
    let port = config["port"].as_u64().expect("port");
    let encoded: String = malicious_name.as_bytes().iter().map(|b| format!("%{b:02X}")).collect();
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port as u16)).await.expect("connects");
    stream.write_all(format!("GET /a/{}/files/1/{encoded}?key={key} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",id.0).as_bytes()).await.expect("request");
    let mut response = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut response)).await.expect("response deadline").expect("response");
    server.shut_down(); server.stopped().await;
    let response = String::from_utf8_lossy(&response);
    let (head, body) = response.split_once("\r\n\r\n").expect("HTTP response");
    assert!(head.starts_with("HTTP/1.1 200"), "{head}");
    assert!(head.contains("content-security-policy: sandbox"),
        "sandbox header disappeared; actual headers={head:?}, body begins={:?}", &body[..body.len().min(200)]);
}

async fn git_ok(folder: &Path, args: &[&str]) -> String {
    git::git(folder, args, &[
        ("GIT_AUTHOR_NAME", "Audit"), ("GIT_AUTHOR_EMAIL", "audit@example.invalid"),
        ("GIT_COMMITTER_NAME", "Audit"), ("GIT_COMMITTER_EMAIL", "audit@example.invalid"),
    ]).await.expect("fixture git command")
}

async fn repository(root: &Path) -> PathBuf {
    let repo = root.join("project"); std::fs::create_dir(&repo).expect("repo");
    git_ok(&repo, &["init", "-q", "-b", "main"]).await;
    std::fs::write(repo.join("README"), "baseline").expect("readme");
    git_ok(&repo, &["add", "-A"]).await;
    git_ok(&repo, &["commit", "-q", "-m", "Baseline"]).await;
    repo
}

async fn pasture(repo: &Path, data: &Path) -> projects::Workspace {
    workspaces::create(workspaces::NewWorkspace {
        kind: WorkspaceKind::Pasture, repo: repo.to_path_buf(), data_dir: data.to_path_buf(),
        base: Some("main".into()), branch: Some("audit-work".into()), submodules: false,
    }).await.expect("pasture")
}

#[tokio::test]
async fn audit_clone_cleanup_must_not_delete_through_an_external_symlink() {
    let root = tempfile::tempdir().expect("root");
    let repo = repository(root.path()).await;
    let outside = root.path().join("outside"); std::fs::create_dir(&outside).expect("outside");
    let precious = outside.join("precious.txt"); std::fs::write(&precious, "keep").expect("precious");
    std::os::unix::fs::symlink(&outside, repo.join("linked")).expect("symlink");
    std::fs::write(repo.join(".cow.json"), r#"{"post_clone":{"remove":["linked/*"]}}"#).expect("config");
    git_ok(&repo, &["add", "-A"]).await; git_ok(&repo, &["commit", "-q", "-m", "Config"]).await;
    let _workspace = pasture(&repo, &root.path().join("data")).await;
    assert!(precious.exists(), "creating a pasture deleted a file outside the pasture");
}

#[tokio::test]
async fn audit_pasture_removal_must_protect_commits_on_other_branches() {
    let root = tempfile::tempdir().expect("root"); let repo = repository(root.path()).await;
    let workspace = pasture(&repo, &root.path().join("data")).await;
    git_ok(&workspace.path, &["checkout", "-q", "-b", "valuable-work"]).await;
    std::fs::write(workspace.path.join("valuable.txt"), "only copy").expect("work");
    git_ok(&workspace.path, &["add", "-A"]).await;
    git_ok(&workspace.path, &["commit", "-q", "-m", "Valuable work"]).await;
    let unique = git_ok(&workspace.path, &["rev-parse", "HEAD"]).await;
    assert!(!workspaces::has_commit(&repo, unique.trim()).await);
    git_ok(&workspace.path, &["checkout", "-q", "audit-work"]).await;
    let result = workspaces::remove(&repo, &workspace, false).await.expect("removes");
    assert!(matches!(result, WorkspaceRemoval::NeedsConfirmation(_)),
        "removed a pasture with unique commits on a non-HEAD branch: {result:?}");
}

#[test]
fn audit_secret_json_must_keep_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt as _;
    let root = tempfile::tempdir().expect("root"); let file = root.path().join("mcp-servers.json");
    std::fs::write(&file, "{}").expect("file");
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).expect("private");
    agent_settings::write_json(&file, &serde_json::json!({"env":{"API_KEY":"audit-fixture"}})).expect("save");
    let mode = std::fs::metadata(&file).expect("metadata").permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "saving private credentials broadened permissions to {mode:o}");
}

#[test]
fn audit_overlapping_settings_saves_must_not_overwrite_a_newer_save() {
    use std::io::Write as _;
    let root = tempfile::tempdir().expect("root"); let path = root.path().join("settings.json");
    let temp = root.path().join("settings.json.tmp");
    let (opened_tx, opened_rx) = std::sync::mpsc::channel();
    let (go_tx, go_rx) = std::sync::mpsc::channel();
    let older = { let path=path.clone(); let temp=temp.clone(); std::thread::spawn(move || {
        projects::write_durably(&path, &temp, |file| {
            opened_tx.send(()).expect("opened"); go_rx.recv().expect("resume");
            file.write_all(b"older!")
        })
    })};
    opened_rx.recv().expect("opened older");
    projects::write_durably(&path, &temp, |file| file.write_all(b"newest")).expect("newer save");
    go_tx.send(()).expect("resume older"); let _older_result=older.join().expect("join");
    assert_eq!(std::fs::read_to_string(&path).expect("file"), "newest",
        "the older file handle mutated the newer settings after their rename");
}
