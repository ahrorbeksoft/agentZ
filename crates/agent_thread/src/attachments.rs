//! Where a thread's attached images and uploaded files are kept: `attachments/<thread id>/` in
//! the server's data directory. Images are named by their [`AttachmentId`]; files keep their
//! names, in a folder named by their hash, since the agent reads them by path.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use agentz_protocol::attachments::{AttachmentId, MAX_ATTACHMENT_SIZE, content_hash};
use anyhow::{Context as _, Result, anyhow};
use base64::Engine as _;
use projects::ThreadId;

#[derive(Clone, Debug)]
pub struct Attachments {
    directory: PathBuf,
}

impl Attachments {
    pub fn for_thread(data_dir: &Path, thread_id: ThreadId) -> Self {
        Self {
            directory: data_dir.join("attachments").join(thread_id.0.to_string()),
        }
    }

    /// The threads that have attachments kept, by their folders in `data_dir`.
    pub fn threads_with_attachments(data_dir: &Path) -> Vec<ThreadId> {
        let Ok(entries) = std::fs::read_dir(data_dir.join("attachments")) else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| {
                let id = entry.ok()?.file_name().to_str()?.parse().ok()?;
                Some(ThreadId(id))
            })
            .collect()
    }

    /// Keeps an image, once for the same bytes.
    pub fn add(&self, mime_type: &str, bytes: &[u8]) -> Result<AttachmentId> {
        anyhow::ensure!(
            bytes.len() <= MAX_ATTACHMENT_SIZE,
            "the image is over {} MB",
            MAX_ATTACHMENT_SIZE / 1024 / 1024
        );
        let id = AttachmentId::for_image(bytes, mime_type)
            .ok_or_else(|| anyhow!("agentZ doesn't keep images of type {mime_type}"))?;
        let path = self.path(&id);
        if !path.exists() {
            write_new(&path, bytes)?;
        }
        Ok(id)
    }

    pub fn add_base64(&self, mime_type: &str, data: &str) -> Result<AttachmentId> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .context("decoding the image")?;
        self.add(mime_type, &bytes)
    }

    /// Keeps a file under its own name, and gives where.
    pub fn add_file(&self, name: &str, bytes: &[u8]) -> Result<PathBuf> {
        anyhow::ensure!(
            bytes.len() <= MAX_ATTACHMENT_SIZE,
            "{name} is over {} MB",
            MAX_ATTACHMENT_SIZE / 1024 / 1024
        );
        // Only its last component, so a name can't reach outside the folder.
        let name = Path::new(name)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| anyhow!("not a file name: {name}"))?;
        let hash = content_hash(bytes);
        let path = self.directory.join("files").join(&hash[..16]).join(name);
        if !path.exists() {
            write_new(&path, bytes)?;
        }
        Ok(path)
    }

    pub fn path(&self, id: &AttachmentId) -> PathBuf {
        self.directory.join(id.as_str())
    }

    /// The PNG thumbnail of the image, once made and kept.
    pub fn thumbnail(&self, id: &AttachmentId) -> Option<Vec<u8>> {
        std::fs::read(self.thumbnail_path(id)).ok()
    }

    pub fn keep_thumbnail(&self, id: &AttachmentId, png: &[u8]) -> Result<()> {
        write_new(&self.thumbnail_path(id), png)
    }

    fn thumbnail_path(&self, id: &AttachmentId) -> PathBuf {
        self.directory
            .join("thumbnails")
            .join(format!("{}.png", id.as_str()))
    }

    pub fn read(&self, id: &AttachmentId) -> Result<Vec<u8>> {
        let path = self.path(id);
        std::fs::read(&path).with_context(|| format!("reading {}", path.display()))
    }

    pub fn read_base64(&self, id: &AttachmentId) -> Result<String> {
        Ok(base64::engine::general_purpose::STANDARD.encode(self.read(id)?))
    }

    /// Forgets every attachment, with the thread.
    pub fn remove_all(&self) -> Result<()> {
        match std::fs::remove_dir_all(&self.directory) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("removing {}", self.directory.display()))
            }
        }
    }
}

/// Writes beside the path first, so a reader never finds half a file.
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    static NEXT_WRITE: AtomicU64 = AtomicU64::new(0);
    let directory = path.parent().context("no folder")?;
    std::fs::create_dir_all(directory)
        .with_context(|| format!("creating {}", directory.display()))?;
    // Two writes of the same file may run at once, as when an image is pasted twice.
    let temporary = directory.join(format!(
        ".{}.{}.{}.tmp",
        path.file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default(),
        std::process::id(),
        NEXT_WRITE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&temporary, bytes)
        .with_context(|| format!("writing {}", temporary.display()))?;
    std::fs::rename(&temporary, path).with_context(|| format!("writing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_are_kept_once_by_their_hash() {
        let data_dir = tempfile::tempdir().expect("a temporary folder");
        let attachments = Attachments::for_thread(data_dir.path(), ThreadId(7));
        let id = attachments
            .add("image/png", b"not really a png")
            .expect("kept");
        assert_eq!(
            attachments.add("image/png", b"not really a png").ok(),
            Some(id.clone())
        );
        assert_eq!(
            attachments.read(&id).expect("read back"),
            b"not really a png"
        );
        assert!(attachments.add("text/plain", b"text").is_err());
        assert_eq!(
            Attachments::threads_with_attachments(data_dir.path()),
            vec![ThreadId(7)]
        );

        let file = attachments
            .add_file("../../notes.txt", b"notes")
            .expect("kept");
        assert!(file.starts_with(data_dir.path().join("attachments").join("7").join("files")));
        assert_eq!(
            file.file_name().and_then(|name| name.to_str()),
            Some("notes.txt")
        );

        attachments.remove_all().expect("removed");
        assert!(attachments.read(&id).is_err());
        assert!(Attachments::threads_with_attachments(data_dir.path()).is_empty());
    }
}
