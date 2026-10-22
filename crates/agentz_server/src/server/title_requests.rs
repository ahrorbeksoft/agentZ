//! Titles written by a CLI ([`crate::title_generation`]) for every agent's threads, over any
//! title the agent gives: once each, after the thread's first turn, unless the user has named
//! it, and again whenever the user asks.

use agentz_protocol::Response;
use agentz_protocol::thread::{Entry, without_handoff};
use agentz_protocol::title_generation::TitleGeneration;
use anyhow::{Context as _, Result, anyhow};
use projects::ThreadId;

use super::{ClientId, Server};
use crate::title_generation::{self, Speaker};
use crate::transcripts;

impl Server {
    pub(super) fn set_title_generation(&mut self, settings: TitleGeneration) -> Result<Response> {
        title_generation::save(&self.data_dir, &settings)?;
        self.title_generation.settings = settings;
        // A CLI installed since the last look shows up as the user picks one.
        self.find_title_providers();
        Ok(Response::Ok)
    }

    pub(super) fn find_title_providers(&self) {
        let shell_environment_ready = self.shell_environment_ready.clone();
        let search_path = self.title_generation_path.clone();
        self.spawn_then(
            async move {
                shell_environment_ready.await;
                title_generation::find_providers(search_path).await
            },
            |server, providers| server.title_generation.providers = providers,
        );
    }

    /// Whether the agent's own title for the thread should show: not once one was generated.
    pub(super) fn takes_agents_title(&self, thread_id: ThreadId) -> bool {
        !self.generated_titles.contains(&thread_id)
    }

    /// Asks the chosen CLI for a title, from the thread's first message, once its first turn
    /// has ended.
    pub(super) fn title_after_first_turn(&mut self, thread_id: ThreadId) {
        let settings = self.title_generation.settings.clone();
        if !settings.enabled
            || self.generated_titles.contains(&thread_id)
            || self
                .projects
                .thread(thread_id)
                .is_none_or(|thread| thread.has_custom_title)
        {
            return;
        }
        let Some(thread) = self.threads.get(&thread_id) else {
            return;
        };
        // A message sent while the first turn worked may be in already, joined to the turn or
        // sent as it ended.
        let Some(message) = thread
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                Entry::UserMessage(text) => Some(without_handoff(text).trim()),
                _ => None,
            })
            .find(|message| !message.is_empty())
            .map(str::to_string)
        else {
            return;
        };
        self.generated_titles.insert(thread_id);
        let search_path = self.title_generation_path.clone();
        self.spawn_then(
            title_generation::generate(settings, title_generation::prompt(&message), search_path),
            move |server, result| match result {
                Ok(title) => {
                    // Kept as the automatic title, so it never replaces the user's own.
                    server.projects.rename_thread(thread_id, title);
                    server.name_branch_after_title(thread_id);
                }
                Err(error) => {
                    log::warn!(
                        "couldn't generate a title for thread {}: {error:#}",
                        thread_id.0
                    )
                }
            },
        );
    }

    /// Titles the thread again from its conversation so far (t3code's Regenerate title), over
    /// any title it has, and answers once it's set. A thread whose agent isn't running is
    /// read from its transcript.
    pub(super) fn regenerate_title(&mut self, client: ClientId, id: u64, thread_id: ThreadId) {
        let settings = self.title_generation.settings.clone();
        if !settings.enabled {
            return self.respond(client, id, Err(anyhow!("title generation is off")));
        }
        let Some(thread) = self.projects.thread(thread_id) else {
            return self.respond(client, id, Err(anyhow!("no such thread")));
        };
        let previous_title = thread.title.clone();
        let loaded = self
            .threads
            .get(&thread_id)
            .map(|thread| title_messages(thread.entries()));
        let data_dir = self.data_dir.clone();
        let search_path = self.title_generation_path.clone();
        self.spawn_then(
            async move {
                let messages = match loaded {
                    Some(messages) => messages,
                    None => {
                        let read = move || transcripts::load(&data_dir, thread_id);
                        tokio::task::spawn_blocking(read)
                            .await
                            .context("reading the transcript")??
                            .map(|transcript| title_messages(&transcript.entries))
                            .unwrap_or_default()
                    }
                };
                let contents = title_generation::thread_context(&messages);
                if contents.is_empty() {
                    return Err(anyhow!("the thread has no messages to title it by"));
                }
                let prompt = title_generation::regeneration_prompt(&previous_title, &contents);
                title_generation::generate(settings, prompt, search_path).await
            },
            move |server, result| {
                let result = result.map(|title| {
                    server.projects.rename_thread(thread_id, title);
                    // Asked for, the new title shows even over a name the user gave.
                    if server
                        .projects
                        .thread(thread_id)
                        .is_some_and(|thread| thread.has_custom_title)
                    {
                        server.projects.set_custom_title(thread_id, String::new());
                    }
                    Response::Ok
                });
                if let Err(error) = &result {
                    log::warn!(
                        "couldn't regenerate the title of thread {}: {error:#}",
                        thread_id.0
                    );
                }
                server.respond(client, id, result);
            },
        );
    }
}

/// The user's messages and the agent's replies, in order, for [`title_generation::thread_context`].
fn title_messages(entries: &[Entry]) -> Vec<(Speaker, String)> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            Entry::UserMessage(text) => Some((Speaker::User, without_handoff(text).to_string())),
            Entry::AgentMessage(text) => Some((Speaker::Agent, text.clone())),
            _ => None,
        })
        .collect()
}
