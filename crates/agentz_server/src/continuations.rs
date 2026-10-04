//! The conversations threads continue with another agent ("Continue with another agent"), kept
//! in `handoffs/<thread id>.json` until each goes with its thread's first message, so a restart
//! of the server doesn't lose one. Until then the thread is a draft, removed once the user leaves
//! it ([`crate::server`]'s `sweep_unsent_continuations`).

use std::path::{Path, PathBuf};

use agentz_protocol::thread::PendingHandoff;
use anyhow::{Context as _, Result};
use projects::ThreadId;

fn path(data_dir: &Path, thread_id: ThreadId) -> PathBuf {
    data_dir
        .join("handoffs")
        .join(format!("{}.json", thread_id.0))
}

pub(crate) fn save(data_dir: &Path, thread_id: ThreadId, handoff: &PendingHandoff) -> Result<()> {
    let path = path(data_dir, thread_id);
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)
            .with_context(|| format!("creating {}", directory.display()))?;
    }
    let json = serde_json::to_vec(handoff).context("encoding the handoff")?;
    std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))
}

pub(crate) fn load(data_dir: &Path, thread_id: ThreadId) -> Result<Option<PendingHandoff>> {
    let path = path(data_dir, thread_id);
    match std::fs::read(&path) {
        Ok(json) => serde_json::from_slice(&json)
            .map(Some)
            .with_context(|| format!("reading {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
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

/// The threads whose conversation waits for their first message.
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
