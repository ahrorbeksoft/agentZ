//! The conversations waiting to go with a thread's next message
//! ([`ThreadState::pending_handoffs`]): what its new agent or account hasn't seen, the
//! conversation a fork starts with or a new session couldn't load, and forks merged back. They're kept in `handoffs/<thread id>.json` until the agent takes a message with
//! them, so a restart of the server doesn't lose one.
//!
//! [`ThreadState::pending_handoffs`]: agentz_protocol::thread::ThreadState::pending_handoffs

use std::io::Write as _;
use std::path::{Path, PathBuf};

use agentz_protocol::thread::PendingHandoff;
use anyhow::{Context as _, Result};
use projects::ThreadId;

pub(crate) fn path(data_dir: &Path, thread_id: ThreadId) -> PathBuf {
    data_dir
        .join("handoffs")
        .join(format!("{}.json", thread_id.0))
}

/// Keeps the thread's waiting handoffs, removing the file once none wait.
pub(crate) fn save(
    data_dir: &Path,
    thread_id: ThreadId,
    handoffs: &[PendingHandoff],
) -> Result<()> {
    if handoffs.is_empty() {
        return remove(data_dir, thread_id);
    }
    let path = path(data_dir, thread_id);
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)
            .with_context(|| format!("creating {}", directory.display()))?;
    }
    let json = serde_json::to_vec(handoffs).context("encoding the handoffs")?;
    projects::write_durably(&path, &path.with_extension("json.tmp"), |file| {
        file.write_all(&json)
    })
}

pub(crate) fn load(data_dir: &Path, thread_id: ThreadId) -> Result<Vec<PendingHandoff>> {
    let path = path(data_dir, thread_id);
    match std::fs::read(&path) {
        Ok(json) => {
            serde_json::from_slice(&json).with_context(|| format!("reading {}", path.display()))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

pub(crate) fn remove(data_dir: &Path, thread_id: ThreadId) -> Result<()> {
    let path = path(data_dir, thread_id);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("removing {}", path.display())),
    }
}

/// The threads with a conversation waiting for their next message.
pub(crate) fn list(data_dir: &Path) -> Vec<ThreadId> {
    let Ok(entries) = std::fs::read_dir(data_dir.join("handoffs")) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let name = entry.ok()?.file_name();
            let id = name.to_str()?.strip_suffix(".json")?.parse().ok()?;
            Some(ThreadId(id))
        })
        .collect()
}
