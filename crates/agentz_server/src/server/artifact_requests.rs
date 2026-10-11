//! What the pages server asks of the server actor ([`pages::PageAsk`]): this machine's
//! artifacts straight from its store, another machine's relayed through the app
//! ([`Event::RelayArtifact`]), a design's picks as the user makes them, and Submit, which can
//! tell the thread that published the design with a short message of the user's.

use agentz_protocol::artifacts::{
    Artifact, ArtifactId, ArtifactListing, ArtifactReply, ArtifactRequest, DesignSettings,
};
use agentz_protocol::{ConnectionId, Event, PeerMachine, PromptPart, Request, ServerMessage};
use base64::Engine as _;
use futures::channel::oneshot;
use projects::ProjectId;
use util::ResultExt as _;

use crate::artifacts::{design_text, submit_summary};
use crate::pages::{self, PageAsk, PageEvent};

use super::Server;

impl Server {
    /// The artifacts snapshot session subscribers get.
    pub(super) fn artifacts_snapshot(&self) -> agentz_protocol::artifacts::Artifacts {
        agentz_protocol::artifacts::Artifacts {
            artifacts: self.artifacts.artifacts().to_vec(),
            pages: self.pages.as_ref().map(pages::Pages::pages),
        }
    }

    /// What the pages server asked, answered now or relayed to the machine it names.
    pub(super) fn answer_page_ask(&mut self, ask: PageAsk) {
        match ask.machine {
            None => {
                let reply = self.artifact_request(ask.request);
                ask.answer.send(reply).ok();
            }
            Some(machine) => self.relay_artifact(machine, ask.request, ask.answer),
        }
    }

    /// This machine's answer to a pages request: the app's `Request::Artifact` lands here too.
    pub(super) fn artifact_request(
        &mut self,
        request: ArtifactRequest,
    ) -> Result<ArtifactReply, String> {
        let error = |error: anyhow::Error| format!("{error:#}");
        match request {
            ArtifactRequest::List => Ok(ArtifactReply::List(
                self.artifacts
                    .artifacts()
                    .iter()
                    .map(|artifact| self.listing(artifact))
                    .collect(),
            )),
            ArtifactRequest::Listing(id) => {
                let artifact = self.artifact(&id)?;
                Ok(ArtifactReply::Listing(self.listing(artifact)))
            }
            ArtifactRequest::Read { id, version } => {
                let artifact = self.artifact(&id)?;
                let version = version.unwrap_or_else(|| artifact.latest());
                let listing = self.listing(artifact);
                let source = self.artifacts.read_source(&id, version).map_err(error)?;
                Ok(ArtifactReply::Page {
                    listing,
                    version,
                    source,
                })
            }
            ArtifactRequest::File { id, version, name } => {
                let bytes = self
                    .artifacts
                    .read_file(&id, version, &name)
                    .map_err(error)?;
                Ok(ArtifactReply::File(
                    base64::engine::general_purpose::STANDARD.encode(bytes),
                ))
            }
            ArtifactRequest::Design { id, version } => {
                let artifact = self.artifact(&id)?;
                let version = version.unwrap_or_else(|| artifact.latest());
                let listing = self.listing(artifact);
                let design = self.artifacts.read_design(&id, version).map_err(error)?;
                let picks = self.artifacts.read_picks(&id, version);
                Ok(ArtifactReply::Design {
                    listing,
                    version,
                    design,
                    picks,
                })
            }
            ArtifactRequest::SetPicks {
                id,
                version,
                topic,
                picks,
            } => {
                self.artifacts
                    .set_picks(&id, version, topic as usize, picks)
                    .map_err(error)?;
                self.artifacts_changed(Some(id));
                Ok(ArtifactReply::Saved)
            }
            ArtifactRequest::Submit {
                id,
                version,
                send,
                note,
            } => {
                self.submit_design(&id, version, send, &note)?;
                Ok(ArtifactReply::Saved)
            }
            ArtifactRequest::Text { id, version } => {
                let artifact = self.artifact(&id)?;
                let version = version.unwrap_or_else(|| artifact.latest());
                let listing = self.listing(artifact);
                let text = match artifact.kind {
                    agentz_protocol::artifacts::ArtifactKind::Document => {
                        self.artifacts.read_source(&id, version).map_err(error)?
                    }
                    agentz_protocol::artifacts::ArtifactKind::Design => {
                        let design = self.artifacts.read_design(&id, version).map_err(error)?;
                        let picks = self.artifacts.read_picks(&id, version);
                        design_text(&artifact.title, version, &listing.place, &design, &picks)
                    }
                };
                Ok(ArtifactReply::Text {
                    listing,
                    version,
                    text,
                })
            }
            ArtifactRequest::Search(query) => {
                Ok(ArtifactReply::Found(self.artifacts.search(&query)))
            }
            ArtifactRequest::Unknown(_) => Err("the app doesn't know this request".to_string()),
        }
    }

    fn artifact(&self, id: &ArtifactId) -> Result<&Artifact, String> {
        self.artifacts
            .get(id)
            .ok_or_else(|| "no such artifact".to_string())
    }

    /// An artifact as its pages show it: where it came from and its agent's icon.
    pub(super) fn listing(&self, artifact: &Artifact) -> ArtifactListing {
        let thread = artifact
            .thread_id
            .and_then(|thread_id| self.projects.thread(thread_id));
        let project = artifact
            .project_id
            .filter(|project_id| *project_id != ProjectId::CHATS)
            .and_then(|project_id| self.projects.project(project_id))
            .map(|project| project.name().to_string());
        let place = match thread {
            Some(thread) if thread.is_chat() => format!("Chat · {}", thread.title),
            Some(thread) => match &project {
                Some(project) => format!("{project} › {}", thread.title),
                None => thread.title.clone(),
            },
            None => "from a deleted thread".to_string(),
        };
        let agent_icon = artifact
            .agent_id
            .as_ref()
            .and_then(|agent_id| self.registry.agent(agent_id))
            .and_then(|agent| agent.icon())
            .map(|icon| icon.svg.to_string());
        ArtifactListing {
            artifact: artifact.clone(),
            place,
            project,
            chat: artifact.is_from_chat(),
            has_thread: thread.is_some(),
            thread_title: thread.map(|thread| thread.title.clone()),
            archived: thread.is_some_and(|thread| thread.archived_at.is_some()),
            agent_name: artifact
                .agent_id
                .as_ref()
                .map(|agent_id| self.agent_name(agent_id).to_string()),
            agent_icon,
        }
    }

    /// Submit (topic 16 A): marks the design's version decided, and with `send`, puts a short
    /// message of the user's in its thread (topic 17 B): what they picked and their note,
    /// behind a chip whose reference has the agent read the rest with `agentz_artifact_read`.
    /// Queued while the agent works, as the composer's messages are.
    fn submit_design(
        &mut self,
        id: &ArtifactId,
        version: u32,
        send: bool,
        note: &str,
    ) -> Result<(), String> {
        let artifact = self.artifact(id)?;
        let thread_id = artifact
            .thread_id
            .filter(|thread_id| self.projects.thread(*thread_id).is_some());
        if send && thread_id.is_none() {
            return Err("the thread that published it was deleted".to_string());
        }
        let title = artifact.title.clone();
        let place = self.listing(artifact).place;
        let design = self
            .artifacts
            .read_design(id, version)
            .map_err(|error| format!("{error:#}"))?;
        let Some(thread_id) = thread_id.filter(|_| send) else {
            self.artifacts
                .submit(id, version, false)
                .map_err(|error| format!("{error:#}"))?;
            self.artifacts_changed(Some(id.clone()));
            return Ok(());
        };
        if version != artifact.latest() {
            return Err("only the latest version can be submitted".to_string());
        }
        let picks = self.artifacts.read_picks(id, version);
        let comments = picks.comments();
        let size = match design.topics.len() {
            1 => format!(
                "1 topic, {} variant{}",
                design.topics[0].variants.len(),
                if design.topics[0].variants.len() == 1 {
                    ""
                } else {
                    "s"
                }
            ),
            topics => format!("{topics} topics"),
        };
        let reference = format!(
            "{title}: a design ({size}, v{version}), from {place}, submitted just now with \
             {comments} comment{}. Read the picks and comments with agentz_artifact_read \
             {{\"artifactId\": \"{}\", \"version\": {version}}}.",
            if comments == 1 { "" } else { "s" },
            id.0
        );
        let mut message = submit_summary(&design, &picks);
        if !note.trim().is_empty() {
            message.push(' ');
            message.push_str(note.trim());
        }
        let prompt = vec![
            PromptPart::Conversation {
                uri: format!("agentz://artifact/{}/{version}", id.0),
                title: format!("{title} · v{version}"),
                text: reference,
            },
            PromptPart::Text(format!(" {message}")),
        ];
        self.queue_request(Request::QueueMessage {
            connection: ConnectionId::Thread(thread_id),
            prompt,
        })
        .map_err(|error| format!("{error:#}"))?;
        self.artifacts
            .submit(id, version, true)
            .map_err(|error| format!("{error:#}"))?;
        self.artifacts_changed(Some(id.clone()));
        Ok(())
    }

    /// The pages server hears of a change, and session subscribers get the new snapshot.
    pub(super) fn artifacts_changed(&self, artifact: Option<ArtifactId>) {
        if let (Some(pages), Some(id)) = (&self.pages, artifact) {
            pages.broadcast(PageEvent::Artifact { machine: None, id });
        }
    }

    /// A page's link to its thread was clicked: the clients show it.
    pub(crate) fn tell_clients(&self, event: Event) {
        if let Event::ShowThread { .. } = event {
            self.broadcast(event);
        }
    }

    /// The other machines the app says it reaches, and what it calls this one, for the
    /// artifacts page.
    pub(crate) fn answer_machines(
        &self,
        answer: oneshot::Sender<(Option<String>, Vec<PeerMachine>)>,
    ) {
        answer.send(self.relays.machines()).ok();
    }

    /// The app's answer to an [`Event::RelayArtifact`].
    pub(super) fn artifact_relayed(
        &mut self,
        relay_id: u64,
        result: Result<ArtifactReply, String>,
    ) {
        self.relays.finish_artifact(relay_id, result);
    }

    fn relay_artifact(
        &mut self,
        machine: String,
        request: ArtifactRequest,
        answer: oneshot::Sender<Result<ArtifactReply, String>>,
    ) {
        // The app's name for this very machine answers locally.
        if self.relays.is_this_machine(&machine) {
            let reply = self.artifact_request(request);
            answer.send(reply).ok();
            return;
        }
        if !self.relays.machine_online(&machine) {
            answer
                .send(Err(format!("{machine} isn't online in agentZ.")))
                .ok();
            return;
        }
        let Some(client) = self.relays.latest_client() else {
            answer
                .send(Err("agentZ isn't open to reach other machines.".to_string()))
                .ok();
            return;
        };
        let relay_id = self.relays.start_artifact_relay(answer);
        self.send(
            client,
            ServerMessage::Event(Event::RelayArtifact {
                relay_id,
                machine,
                request,
            }),
        );
    }

    /// Deletes an artifact (`Request::DeleteArtifact`).
    pub(super) fn delete_artifact(&mut self, id: ArtifactId) -> anyhow::Result<()> {
        self.artifacts.delete(&id)?;
        if let Some(pages) = &self.pages {
            pages.broadcast(PageEvent::Deleted {
                machine: None,
                id: id.clone(),
            });
        }
        Ok(())
    }

    /// The app's theme, for the pages this server serves (`Request::SetPageTheme`).
    pub(super) fn set_page_theme(&mut self, theme: agentz_protocol::artifacts::PageTheme) {
        crate::artifacts::save_theme(&self.data_dir, &theme).log_err();
        if let Some(pages) = &self.pages {
            pages.set_theme(theme);
        }
    }

    /// The app's design defaults (`Request::SetDesignDefaults`), for agents' tool lists.
    pub(super) fn set_design_defaults(&mut self, settings: DesignSettings) -> anyhow::Result<()> {
        self.artifacts.set_design_settings(settings)
    }
}
