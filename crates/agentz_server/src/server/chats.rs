//! Chats: threads outside every project (t3code's threads with no project), listed in the
//! sidebar's Chats shelf. Each works in a folder of its own in the data directory, named as
//! t3code names them (the date, the first words of its first message and its id), made when
//! its first message is sent. Until then its agent works in the folder those are made in.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use agentz_protocol::PromptPart;
use anyhow::{Context as _, Result};
use projects::ThreadId;
use util::ResultExt as _;

use super::Server;

/// How many names a chat's folder tries before giving up: the id makes the first one unique,
/// so the rest only cover folders left behind.
const MAX_FOLDER_ATTEMPTS: u32 = 100;

impl Server {
    /// Before a chat's first message: makes its folder, and moves its agent there. Its draft's
    /// session was opened where chats' folders are made, and never had a message.
    pub(super) fn give_chat_its_folder(
        &mut self,
        thread_id: ThreadId,
        prompt: &[PromptPart],
    ) -> Result<()> {
        let Some(thread) = self.projects.thread(thread_id) else {
            return Ok(());
        };
        if !thread.is_chat() || thread.task.is_some() || thread.workspace.is_some() {
            return Ok(());
        }
        let root = self
            .projects
            .chats_folder()
            .context("chats have no folder")?
            .to_path_buf();
        let text: String = prompt
            .iter()
            .filter_map(|part| match part {
                PromptPart::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        let date = chrono::Local::now().format("%Y-%m-%d").to_string();
        let folder = claim_folder(&root, &date, &text, thread_id)?;
        self.projects.set_thread_workspace(thread_id, Some(folder));
        self.stop_agent(thread_id);
        self.tool_sessions
            .retain(|_, session_thread| *session_thread != thread_id);
        self.projects.forget_thread_session(thread_id);
        Ok(())
    }

    /// Removes a deleted chat's folder, with what its agent left there, unless another thread
    /// still works in it (one continuing it with another agent).
    pub(super) fn remove_chat_folder(&self, folder: PathBuf) {
        let in_chats_folder = self
            .projects
            .chats_folder()
            .is_some_and(|root| folder.parent() == Some(root));
        if !in_chats_folder || !self.projects.threads_in_folder(&folder).is_empty() {
            return;
        }
        self.runtime.spawn_blocking(move || {
            std::fs::remove_dir_all(&folder)
                .with_context(|| format!("removing {}", folder.display()))
                .log_err();
        });
    }
}

/// t3code's `folderWords`: the first five words of the message, lowercase letters and digits
/// only, so the name stays one path segment, cut to 48 characters.
fn folder_words(text: &str) -> String {
    let words: Vec<String> = text
        .to_lowercase()
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .take(5)
        .map(str::to_string)
        .collect();
    let joined = words.join("-");
    let cut: String = joined.chars().take(48).collect();
    cut.trim_end_matches('-').to_string()
}

/// Makes the chat's folder in `root`, claimed by creating it: a name that's taken gets a
/// number after the id.
fn claim_folder(root: &Path, date: &str, text: &str, thread_id: ThreadId) -> Result<PathBuf> {
    std::fs::create_dir_all(root).with_context(|| format!("creating {}", root.display()))?;
    let words = folder_words(text);
    let base: Vec<&str> = [date, words.as_str()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    let base = format!("{}-{}", base.join("-"), thread_id.0);
    for attempt in 1..=MAX_FOLDER_ATTEMPTS {
        let name = match attempt {
            1 => base.clone(),
            attempt => format!("{base}-{attempt}"),
        };
        let folder = root.join(name);
        match std::fs::create_dir(&folder) {
            Ok(()) => return Ok(folder),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("creating {}", folder.display()));
            }
        }
    }
    anyhow::bail!(
        "every name for the chat's folder in {} is taken",
        root.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chat_folder_is_named_after_its_first_words() {
        assert_eq!(
            folder_words("How do I pin a Future? It's !Unpin, and…"),
            "how-do-i-pin-a"
        );
        assert_eq!(folder_words("¿Qué?"), "qu");
        assert_eq!(folder_words("…"), "");
        let long = "a".repeat(60);
        assert_eq!(folder_words(&long).len(), 48);

        let root = tempfile::tempdir().expect("temp dir");
        let first = claim_folder(root.path(), "2026-05-01", "Hello there", ThreadId(7))
            .expect("first folder");
        assert_eq!(first, root.path().join("2026-05-01-hello-there-7"));
        let again = claim_folder(root.path(), "2026-05-01", "Hello there", ThreadId(7))
            .expect("second folder");
        assert_eq!(again, root.path().join("2026-05-01-hello-there-7-2"));
        let wordless =
            claim_folder(root.path(), "2026-05-01", "…", ThreadId(8)).expect("wordless folder");
        assert_eq!(wordless, root.path().join("2026-05-01-8"));
    }
}
