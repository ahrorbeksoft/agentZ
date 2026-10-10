//! What the pages server asks of the server actor ([`pages::PageAsk`]): this machine's
//! artifacts straight from its store, another machine's relayed through the app
//! ([`Event::RelayArtifact`]), and Send to thread, which queues the page's text and the user's
//! note as their message in the thread that published it.

use agentz_protocol::artifacts::{
    Artifact, ArtifactId, ArtifactListing, ArtifactReply, ArtifactRequest,
};
use agentz_protocol::{ConnectionId, Event, PromptPart, Request, ServerMessage};
use base64::Engine as _;
use futures::channel::oneshot;
use util::ResultExt as _;

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
        match request {
            ArtifactRequest::List => Ok(ArtifactReply::List(
                self.artifacts
                    .artifacts()
                    .iter()
                    .map(|artifact| self.listing(artifact))
                    .collect(),
            )),
            ArtifactRequest::Listing(id) => {
                let artifact = self
                    .artifacts
                    .get(&id)
                    .ok_or_else(|| "no such artifact".to_string())?;
                Ok(ArtifactReply::Listing(self.listing(artifact)))
            }
            ArtifactRequest::Read { id, version } => {
                let artifact = self
                    .artifacts
                    .get(&id)
                    .ok_or_else(|| "no such artifact".to_string())?;
                let version = version.unwrap_or_else(|| artifact.latest());
                let listing = self.listing(artifact);
                let source = self
                    .artifacts
                    .read_source(&id, version)
                    .map_err(|error| format!("{error:#}"))?;
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
                    .map_err(|error| format!("{error:#}"))?;
                Ok(ArtifactReply::File(
                    base64::engine::general_purpose::STANDARD.encode(bytes),
                ))
            }
            ArtifactRequest::Send {
                id,
                version,
                text,
                note,
            } => {
                self.artifact_send(&id, version, &text, &note)?;
                Ok(ArtifactReply::Sent)
            }
            ArtifactRequest::Unknown(_) => Err("the app doesn't know this request".to_string()),
        }
    }

    /// An artifact as its pages show it: where it came from and its agent's icon.
    fn listing(&self, artifact: &Artifact) -> ArtifactListing {
        let thread = artifact
            .thread_id
            .and_then(|thread_id| self.projects.thread(thread_id));
        let place = match thread {
            Some(thread) if thread.is_chat() => format!("Chat · {}", thread.title),
            Some(thread) => match self.projects.project(thread.project_id) {
                Some(project) => format!("{} › {}", project.name(), thread.title),
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
            has_thread: thread.is_some(),
            agent_name: artifact
                .agent_id
                .as_ref()
                .map(|agent_id| self.agent_name(agent_id).to_string()),
            agent_icon,
        }
    }

    /// Send to thread (topic 11): the page's text and the user's note, as their message, with
    /// a chip naming the page and version. Queued while the agent works, as the composer's
    /// messages are.
    fn artifact_send(
        &mut self,
        id: &ArtifactId,
        version: u32,
        text: &str,
        note: &str,
    ) -> Result<(), String> {
        let artifact = self
            .artifacts
            .get(id)
            .ok_or_else(|| "no such artifact".to_string())?;
        let thread_id = artifact
            .thread_id
            .filter(|thread_id| self.projects.thread(*thread_id).is_some())
            .ok_or_else(|| "the thread that published it was deleted".to_string())?;
        let title = format!("{} · v{version}", artifact.title);
        // The chip naming the page carries what it gave; the note follows, as typed after it.
        let mut prompt = Vec::new();
        if !text.trim().is_empty() {
            prompt.push(PromptPart::Conversation {
                uri: format!("agentz://artifact/{}/{version}", id.0),
                title,
                text: text.trim().to_string(),
            });
        }
        if !note.trim().is_empty() {
            let note = note.trim();
            prompt.push(PromptPart::Text(if prompt.is_empty() {
                note.to_string()
            } else {
                format!(" {note}")
            }));
        }
        if prompt.is_empty() {
            return Err("the page gave nothing, and there's no note".to_string());
        }
        self.queue_request(Request::QueueMessage {
            connection: ConnectionId::Thread(thread_id),
            prompt,
        })
        .map_err(|error| format!("{error:#}"))?;
        self.artifacts.note_sent(id);
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

    /// The other machines the app says it reaches, for the gallery.
    pub(crate) fn answer_machine_names(&self, answer: oneshot::Sender<Vec<String>>) {
        answer.send(self.relays.machine_names()).ok();
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
}
