//! Threads' conversations ([`agent_thread::Transcript`]), kept in
//! `transcripts/<thread id>.json` so a thread opens with all of its conversation, whatever its
//! agent replays.

use std::path::{Path, PathBuf};

use agent_thread::Transcript;
use anyhow::{Context as _, Result};
use projects::ThreadId;

fn path(data_dir: &Path, thread_id: ThreadId) -> PathBuf {
    data_dir
        .join("transcripts")
        .join(format!("{}.json", thread_id.0))
}

pub(crate) fn save(data_dir: &Path, thread_id: ThreadId, transcript: &Transcript) -> Result<()> {
    let path = path(data_dir, thread_id);
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)
            .with_context(|| format!("creating {}", directory.display()))?;
    }
    let json = serde_json::to_vec(transcript).context("encoding the transcript")?;
    // Written beside it first, so a crash doesn't leave half a conversation.
    let temporary = path.with_extension("json.tmp");
    std::fs::write(&temporary, json).with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, &path).with_context(|| format!("writing {}", path.display()))
}

pub(crate) fn load(data_dir: &Path, thread_id: ThreadId) -> Result<Option<Transcript>> {
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

/// The threads that have a transcript kept.
pub(crate) fn list(data_dir: &Path) -> Vec<ThreadId> {
    let Ok(entries) = std::fs::read_dir(data_dir.join("transcripts")) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcripts_are_kept_until_removed() {
        let data_dir = tempfile::tempdir().expect("temp dir");
        let thread_id = ThreadId(4);
        assert!(
            load(data_dir.path(), thread_id)
                .expect("readable")
                .is_none()
        );
        let transcript = Transcript {
            entries: vec![agent_thread::Entry::UserMessage("hello".into())],
            ..Transcript::default()
        };
        save(data_dir.path(), thread_id, &transcript).expect("saved");
        assert_eq!(
            load(data_dir.path(), thread_id).expect("readable"),
            Some(transcript)
        );
        assert_eq!(list(data_dir.path()), [thread_id]);
        remove(data_dir.path(), thread_id).expect("removed");
        assert!(list(data_dir.path()).is_empty());
    }
}
