//! Titles written by a CLI ([`crate::title_generation`]) for threads whose agent gives them
//! none: once each, after the thread's first turn, unless the user has named it.

use agentz_protocol::Response;
use agentz_protocol::thread::{Entry, without_handoff};
use agentz_protocol::title_generation::TitleGeneration;
use anyhow::Result;
use projects::ThreadId;

use super::Server;
use crate::title_generation;

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

    pub(super) fn agent_titled_thread(&mut self, thread_id: ThreadId) {
        self.agent_titled_threads.insert(thread_id);
    }

    /// Asks the chosen CLI for a title once the thread's first turn has ended.
    pub(super) fn title_after_first_turn(&mut self, thread_id: ThreadId) {
        let settings = self.title_generation.settings.clone();
        if !settings.enabled
            || self.agent_titled_threads.contains(&thread_id)
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
        let mut messages = thread.entries().iter().filter_map(|entry| match entry {
            Entry::UserMessage(text) => Some(text),
            _ => None,
        });
        let (Some(message), None) = (messages.next(), messages.next()) else {
            return;
        };
        let message = without_handoff(message).trim().to_string();
        if message.is_empty() {
            return;
        }
        self.generated_titles.insert(thread_id);
        let search_path = self.title_generation_path.clone();
        self.spawn_then(
            title_generation::generate(settings, message, search_path),
            move |server, result| match result {
                Ok(title) => {
                    if server.agent_titled_threads.contains(&thread_id) {
                        return;
                    }
                    // Kept as the automatic title, so it never replaces the user's own.
                    server.projects.rename_thread(thread_id, title);
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
}
