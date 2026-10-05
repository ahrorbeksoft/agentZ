//! Messages to agents with what the user mentioned in them (Zed's mentions): files and folders
//! of the thread's machine, other threads' conversations and pasted images, and the files @ can
//! mention.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_thread::MessagePart;
use agentz_protocol::agents::AgentId;
use agentz_protocol::thread::{ConnectionStatus, mentioned_thread};
use agentz_protocol::{ConnectionId, FileEntry, FileListing, PromptPart, Response};
use anyhow::{Context as _, Result};
use projects::ThreadId;
use util::ResultExt as _;

use super::{ClientId, Server};

/// How long a message waits for the threads it mentions to load their conversations.
const MENTIONED_THREADS_TIMEOUT: Duration = Duration::from_secs(30);
/// A mentioned file bigger than this goes as a link rather than its contents.
const MENTIONED_FILE_LIMIT: u64 = 1024 * 1024;

/// A message waiting for the threads it mentions.
pub(super) struct PendingPrompt {
    connection: ConnectionId,
    prompt: Vec<PromptPart>,
    deadline: Instant,
}

impl Server {
    /// Sends a message once the threads it mentions have their conversations, starting their
    /// agents if they aren't running.
    pub(super) fn queue_prompt(
        &mut self,
        connection: ConnectionId,
        prompt: Vec<PromptPart>,
    ) -> Result<()> {
        for part in &prompt {
            if let PromptPart::Thread(thread_id) = part {
                let is_agent_thread = self
                    .projects
                    .thread(*thread_id)
                    .is_some_and(|thread| thread.terminal.is_none());
                anyhow::ensure!(is_agent_thread, "the mentioned thread is gone");
                if !self.threads.contains_key(thread_id) {
                    let thread = self.start_thread(*thread_id)?;
                    self.threads.insert(*thread_id, thread);
                }
            }
        }
        let deadline = Instant::now() + MENTIONED_THREADS_TIMEOUT;
        self.pending_prompts.push(PendingPrompt {
            connection,
            prompt,
            deadline,
        });
        self.wake_at(deadline);
        self.send_waiting_prompts();
        Ok(())
    }

    /// Sends the waiting messages whose mentioned threads have loaded, or that waited long
    /// enough.
    pub(super) fn send_waiting_prompts(&mut self) {
        let now = Instant::now();
        for pending in std::mem::take(&mut self.pending_prompts) {
            if let ConnectionId::Thread(thread_id) = pending.connection
                && self.projects.thread(thread_id).is_none()
            {
                continue;
            }
            let is_loading = pending.prompt.iter().any(|part| match part {
                PromptPart::Thread(thread_id) => self
                    .threads
                    .get(thread_id)
                    .is_some_and(|thread| *thread.status() == ConnectionStatus::Connecting),
                _ => false,
            });
            if is_loading && now < pending.deadline {
                self.pending_prompts.push(pending);
            } else {
                self.send_prompt(pending.connection, pending.prompt);
            }
        }
    }

    /// Takes the mentioned threads' conversations now, reads the mentioned files off the
    /// server's task, then sends.
    fn send_prompt(&mut self, connection: ConnectionId, prompt: Vec<PromptPart>) {
        let parts: Vec<Result<MessagePart, PathBuf>> = prompt
            .into_iter()
            .map(|part| match part {
                PromptPart::Text(text) => Ok(MessagePart::Text(text)),
                PromptPart::Thread(thread_id) => Ok(self.mentioned_thread(thread_id)),
                PromptPart::Image { mime_type, data } => Ok(MessagePart::Image { mime_type, data }),
                PromptPart::Path(path) => Err(path),
            })
            .collect();
        self.spawn_then(
            async move {
                let mut resolved = Vec::with_capacity(parts.len());
                for part in parts {
                    resolved.push(match part {
                        Ok(part) => part,
                        Err(path) => mentioned_path(path).await,
                    });
                }
                resolved
            },
            move |server, parts| {
                server
                    .update_thread(connection, |thread| thread.send_message(parts))
                    .log_err();
            },
        );
    }

    fn mentioned_thread(&self, thread_id: ThreadId) -> MessagePart {
        let title = self
            .projects
            .thread(thread_id)
            .map(|thread| thread.title.clone())
            .unwrap_or_default();
        let text = match self.threads.get(&thread_id) {
            Some(thread) => {
                let agent_name = self
                    .projects
                    .thread(thread_id)
                    .and_then(|thread| thread.agent_id.clone())
                    .map(|agent_id| self.agent_name(&AgentId::new(agent_id)).to_string())
                    .unwrap_or_else(|| thread.agent_name().to_string());
                mentioned_thread(thread, &agent_name, &title)
            }
            None => format!("(The conversation of \"{title}\" couldn't be loaded.)"),
        };
        MessagePart::Thread {
            uri: format!("agentz://thread/{}", thread_id.0),
            title,
            text,
        }
    }

    /// The files and folders @ can mention, in the folder the thread works in.
    pub(super) fn list_files(&mut self, client: ClientId, id: u64, thread_id: ThreadId) {
        let Some(root) = self.projects.thread_folder(thread_id) else {
            return self.respond(client, id, Err(anyhow::anyhow!("no such thread")));
        };
        self.spawn_then(
            async move {
                tokio::task::spawn_blocking(move || list_files(root))
                    .await
                    .context("listing the files")?
            },
            move |server, listing| server.respond(client, id, listing.map(Response::Files)),
        );
    }
}

/// A mentioned path: a folder, or a file with its contents when they're text and not too big.
async fn mentioned_path(path: PathBuf) -> MessagePart {
    let Ok(metadata) = tokio::fs::metadata(&path).await else {
        return MessagePart::File {
            path,
            contents: None,
        };
    };
    if metadata.is_dir() {
        return MessagePart::Folder(path);
    }
    let contents = if metadata.len() <= MENTIONED_FILE_LIMIT {
        tokio::fs::read(&path)
            .await
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok())
    } else {
        None
    };
    MessagePart::File { path, contents }
}

/// The folder's files and folders, as git would show them: gitignored ones and `.git` left
/// out.
fn list_files(root: PathBuf) -> Result<FileListing> {
    let mut entries = Vec::new();
    let walker = ignore::WalkBuilder::new(&root)
        .hidden(false)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build();
    for entry in walker {
        let Ok(entry) = entry else {
            continue;
        };
        let Some(path) = relative_path(&root, entry.path()) else {
            continue;
        };
        entries.push(FileEntry {
            path,
            is_dir: entry.file_type().is_some_and(|kind| kind.is_dir()),
        });
        if entries.len() >= FileListing::LIMIT {
            break;
        }
    }
    Ok(FileListing { root, entries })
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }
    let names: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(names.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_are_listed_without_ignored_ones() {
        let root = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(root.path().join("src/cart")).expect("folders");
        std::fs::create_dir_all(root.path().join(".git")).expect("folders");
        std::fs::write(root.path().join("src/cart/total.ts"), "").expect("a file");
        std::fs::write(root.path().join("debug.log"), "").expect("a file");
        std::fs::write(root.path().join(".gitignore"), "*.log\n").expect("a file");
        let listing = list_files(root.path().to_path_buf()).expect("a listing");
        let mut paths: Vec<(&str, bool)> = listing
            .entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry.is_dir))
            .collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![
                (".gitignore", false),
                ("src", true),
                ("src/cart", true),
                ("src/cart/total.ts", false),
            ]
        );
    }
}
