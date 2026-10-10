//! Messages to agents with what the user mentioned in them (Zed's mentions): files and folders
//! of the thread's machine, other threads' conversations and pasted images, and the files @ can
//! mention.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_thread::{Attachments, MessagePart};
use agentz_protocol::agents::AgentId;
use agentz_protocol::attachments::AttachmentId;
use agentz_protocol::thread::{ConnectionStatus, mentioned_thread};
use agentz_protocol::{ConnectionId, FileEntry, FileListing, PromptPart, Response};
use anyhow::{Context as _, Result, anyhow};
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
    /// Sent into the turn the agent is working on, as a steered queued message is
    /// ([`agentz_protocol::Request::SteerQueuedMessage`]).
    steer: bool,
    deadline: Instant,
}

impl Server {
    /// Sends a message to a thread's agent ([`agentz_protocol::Request::Prompt`]), or into the
    /// turn it's working on.
    pub(super) fn prompt(
        &mut self,
        connection: ConnectionId,
        prompt: Vec<PromptPart>,
        steer: bool,
    ) -> Result<Response> {
        if let ConnectionId::Thread(thread_id) = connection
            && self
                .projects
                .thread(thread_id)
                .is_some_and(|thread| thread.task.is_some())
        {
            return Err(anyhow!(
                "a subthread only takes its task; message its parent instead"
            ));
        }
        if prompt
            .iter()
            .all(|part| matches!(part, PromptPart::Text(_)))
        {
            let text = prompt
                .into_iter()
                .map(|part| match part {
                    PromptPart::Text(text) => text,
                    _ => String::new(),
                })
                .collect();
            self.update_thread(connection, |thread| {
                if steer {
                    thread.steer(text)
                } else {
                    thread.send(text)
                }
            })?;
        } else {
            // Starts the thread's agent first, as plain text does.
            self.update_thread(connection, |_| ())?;
            self.queue_prompt(connection, prompt, steer)?;
        }
        Ok(Response::Ok)
    }

    /// Sends a message once the threads it mentions have their conversations, starting their
    /// agents if they aren't running.
    pub(super) fn queue_prompt(
        &mut self,
        connection: ConnectionId,
        prompt: Vec<PromptPart>,
        steer: bool,
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
            steer,
            deadline,
        });
        self.wake_at(deadline);
        self.send_waiting_prompts();
        Ok(())
    }

    /// Whether a message to the thread waits for the threads it mentions, or for its files and
    /// images to be read.
    pub(super) fn has_waiting_prompt(&self, thread_id: ThreadId) -> bool {
        let connection = ConnectionId::Thread(thread_id);
        self.pending_prompts
            .iter()
            .any(|pending| pending.connection == connection)
            || self.reading_prompts.contains(&connection)
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
                self.send_prompt(pending.connection, pending.prompt, pending.steer);
            }
        }
    }

    /// Takes the mentioned threads' conversations now, reads the mentioned files and the
    /// images off the server's task, then sends.
    fn send_prompt(&mut self, connection: ConnectionId, prompt: Vec<PromptPart>, steer: bool) {
        let attachments = match connection {
            ConnectionId::Thread(thread_id) => {
                Some(Attachments::for_thread(&self.data_dir, thread_id))
            }
            ConnectionId::LoginSession(_) => None,
        };
        let parts: Vec<UnreadPart> = prompt
            .into_iter()
            .map(|part| match part {
                PromptPart::Text(text) => UnreadPart::Read(MessagePart::Text(text)),
                PromptPart::Thread(thread_id) => UnreadPart::Read(self.mentioned_thread(thread_id)),
                PromptPart::Image(id) => UnreadPart::Image(id),
                PromptPart::Path(path) => UnreadPart::Path(path),
            })
            .collect();
        self.reading_prompts.push(connection);
        self.spawn_then(
            async move {
                let mut resolved = Vec::with_capacity(parts.len());
                for part in parts {
                    match part {
                        UnreadPart::Read(part) => resolved.push(part),
                        UnreadPart::Path(path) => resolved.push(mentioned_path(path).await),
                        UnreadPart::Image(id) => {
                            match attached_image(attachments.clone(), id).await {
                                Ok(part) => resolved.push(part),
                                Err(error) => log::error!("failed to send an image: {error:#}"),
                            }
                        }
                    }
                }
                resolved
            },
            move |server, parts| {
                if let Some(index) = server
                    .reading_prompts
                    .iter()
                    .position(|reading| *reading == connection)
                {
                    server.reading_prompts.remove(index);
                }
                server
                    .update_thread(connection, |thread| {
                        if steer {
                            thread.steer_message(parts)
                        } else {
                            thread.send_after_turn(parts)
                        }
                    })
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
                tokio::task::spawn_blocking(move || list_files(root, |_| true))
                    .await
                    .context("listing the files")?
            },
            move |server, listing| server.respond(client, id, listing.map(Response::Files)),
        );
    }
}

/// A part of a message, before what it names is read.
enum UnreadPart {
    Read(MessagePart),
    Path(PathBuf),
    Image(AttachmentId),
}

/// An image kept for the thread, with its bytes.
async fn attached_image(attachments: Option<Attachments>, id: AttachmentId) -> Result<MessagePart> {
    let attachments = attachments.context("only threads take images")?;
    let read = {
        let id = id.clone();
        tokio::task::spawn_blocking(move || attachments.read_base64(&id))
    };
    let data = read.await.context("reading the image")??;
    Ok(MessagePart::Image { id, data })
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

/// The folder's files and folders that `keep` keeps, as git would show them: gitignored ones
/// and `.git` left out.
pub(super) fn list_files(root: PathBuf, keep: impl Fn(&FileEntry) -> bool) -> Result<FileListing> {
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
        let entry = FileEntry {
            path,
            is_dir: entry.file_type().is_some_and(|kind| kind.is_dir()),
        };
        if !keep(&entry) {
            continue;
        }
        entries.push(entry);
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
        let listing = list_files(root.path().to_path_buf(), |_| true).expect("a listing");
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
