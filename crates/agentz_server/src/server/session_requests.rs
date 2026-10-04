//! Importing the conversations agents keep (ACP's `session/list`) as threads, as Zed's thread
//! import does.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use agent_thread::SessionListing;
use agentz_protocol::Response;
use agentz_protocol::agents::{AgentId, AgentSession, AgentSessions};
use anyhow::Result;
use projects::ImportedSession;

use super::{ClientId, Server, thread_title_from_prompt};

/// What an imported thread is called when its agent gave the session no title.
const UNTITLED_SESSION_TITLE: &str = "Imported thread";

impl Server {
    pub(super) fn list_agent_sessions(&mut self, client: ClientId, id: u64, agent_id: AgentId) {
        let command = self.agent_command(&agent_id, true);
        self.spawn_then(
            async move {
                let listing = agent_thread::list_sessions(command).await?;
                // Projects' folders are canonical, and agents may report `/tmp` for
                // `/private/tmp`.
                Ok(match listing {
                    SessionListing::Listed(sessions) => {
                        let sessions = sessions
                            .into_iter()
                            .map(|session| (canonical(&session.cwd), session))
                            .collect::<Vec<_>>();
                        Listing::Listed(sessions)
                    }
                    SessionListing::Unsupported => Listing::Unsupported,
                    SessionListing::LoggedOut => Listing::LoggedOut,
                })
            },
            move |server, listing: Result<Listing>| {
                let sessions = listing.map(|listing| server.agent_sessions(&agent_id, listing));
                server.respond(client, id, sessions.map(Response::AgentSessions));
            },
        );
    }

    fn agent_sessions(&self, agent_id: &AgentId, listing: Listing) -> AgentSessions {
        let sessions = match listing {
            Listing::Listed(sessions) => sessions,
            Listing::Unsupported => return AgentSessions::Unsupported,
            Listing::LoggedOut => return AgentSessions::LoggedOut,
        };
        AgentSessions::Listed(
            sessions
                .into_iter()
                .map(|(folder, session)| {
                    let session_id = session.session_id.0.to_string();
                    let (project_id, workspace) = match self.projects.folder_owner(&folder) {
                        Some((project_id, workspace)) => (Some(project_id), workspace),
                        None => (None, None),
                    };
                    AgentSession {
                        project_id,
                        workspace,
                        thread_id: self.projects.thread_for_session(&agent_id.0, &session_id),
                        updated_at: session.updated_at.as_deref().and_then(parse_timestamp),
                        title: session.title.filter(|title| !title.trim().is_empty()),
                        cwd: session.cwd,
                        session_id,
                    }
                })
                .collect(),
        )
    }

    pub(super) fn import_agent_sessions(
        &mut self,
        agent_id: AgentId,
        sessions: Vec<AgentSession>,
        archived: bool,
    ) -> Result<Response> {
        let mut imported = Vec::new();
        for session in sessions {
            if self
                .projects
                .thread_for_session(&agent_id.0, &session.session_id)
                .is_some()
            {
                continue;
            }
            let Some((project_id, workspace)) =
                self.projects.folder_owner(&canonical(&session.cwd))
            else {
                continue;
            };
            let title = session
                .title
                .as_deref()
                .map(thread_title_from_prompt)
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| UNTITLED_SESSION_TITLE.to_string());
            if let Some(thread_id) = self.projects.add_imported_thread(ImportedSession {
                project_id,
                workspace,
                agent_id: agent_id.0.to_string(),
                session_id: session.session_id,
                title,
                updated_at: session.updated_at,
                archived,
            }) {
                imported.push(thread_id);
            }
        }
        Ok(Response::ThreadsImported(imported))
    }
}

/// An agent's listing, with each session's folder made canonical.
enum Listing {
    Listed(Vec<(PathBuf, agent_client_protocol::schema::v1::SessionInfo)>),
    Unsupported,
    LoggedOut,
}

fn canonical(folder: &Path) -> PathBuf {
    std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf())
}

/// ACP's `updatedAt` is ISO 8601.
fn parse_timestamp(timestamp: &str) -> Option<SystemTime> {
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .ok()
        .map(SystemTime::from)
}
