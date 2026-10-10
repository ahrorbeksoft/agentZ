//! The artifact tools (design/artifacts): `agentz_artifact_publish` publishes a page the agent
//! wrote (a version of it each time), and `agentz_artifact_list` and `agentz_artifact_read`
//! find them again. Each publish is served to the user's browser by the pages server, whose
//! link the answer carries.

use agentz_protocol::artifacts::{ArtifactId, ArtifactKind};
use futures::FutureExt as _;
use serde_json::{Value, json};

use super::{Arguments, Caller, Continuation, Outcome, Server, Step, failure, invalid};
use crate::artifacts::Publish;

const MAX_PATH_CHARS: usize = 4_096;
const MAX_FILES: usize = 20;
/// The most of a page's source a read answers with.
const MAX_READ_CHARS: usize = 50_000;

impl Server {
    pub(super) fn artifact_publish(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let Some(thread_id) = caller.thread_id else {
            return Err(failure(
                "capability_denied",
                "Only a thread's agent can publish an artifact.",
            ));
        };
        let title = arguments
            .string("title", 256)?
            .ok_or_else(|| invalid("title is required."))?
            .to_string();
        let file = arguments
            .string("file", MAX_PATH_CHARS)?
            .ok_or_else(|| invalid("file is required."))?;
        let file = crate::directories::expand_home(std::path::Path::new(file));
        if !file.is_absolute() {
            return Err(invalid("file must be an absolute path or start with ~."));
        }
        let kind = match file.extension().and_then(|extension| extension.to_str()) {
            Some(extension) if extension.eq_ignore_ascii_case("md") => ArtifactKind::Document,
            _ => ArtifactKind::Page,
        };
        let mut attachments = Vec::new();
        if let Some(paths) = arguments.array("files")? {
            if paths.len() > MAX_FILES {
                return Err(invalid(format!("at most {MAX_FILES} files.")));
            }
            for path in paths {
                let Some(path) = path.as_str() else {
                    return Err(invalid("files holds paths."));
                };
                let path = crate::directories::expand_home(std::path::Path::new(path));
                if !path.is_absolute() {
                    return Err(invalid(
                        "each file must be an absolute path or start with ~.",
                    ));
                }
                attachments.push(path);
            }
        }
        let update = arguments
            .string("artifactId", 64)?
            .map(|id| ArtifactId(id.to_string()));
        let read = async move {
            let source = tokio::fs::read_to_string(&file).await.map_err(|error| {
                failure(
                    "invalid_request",
                    format!("{} couldn't be read: {error}", file.display()),
                )
            })?;
            let mut files = Vec::with_capacity(attachments.len());
            for path in attachments {
                let bytes = tokio::fs::read(&path).await.map_err(|error| {
                    failure(
                        "invalid_request",
                        format!("{} couldn't be read: {error}", path.display()),
                    )
                })?;
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                files.push((name, bytes));
            }
            Ok::<_, super::Failure>((source, files))
        };
        Ok(Step::Then(
            async move {
                let read = read.await;
                Box::new(move |server: &mut Server| {
                    let (source, files) = read?;
                    let project_id = server
                        .projects
                        .thread(thread_id)
                        .map(|thread| thread.project_id)
                        .ok_or_else(|| failure("thread_not_found", "The thread is gone."))?;
                    let agent_id = server
                        .projects
                        .thread(thread_id)
                        .and_then(|thread| thread.agent_id.clone())
                        .map(agentz_protocol::agents::AgentId::new);
                    let (id, version) = server
                        .artifacts
                        .publish(Publish {
                            id: update,
                            title,
                            kind,
                            source,
                            files,
                            thread_id,
                            project_id,
                            agent_id,
                        })
                        .map_err(|error| failure("invalid_request", format!("{error:#}")))?;
                    server.artifacts_changed(Some(id.clone()));
                    let url = server
                        .pages
                        .as_ref()
                        .map(|pages| pages.pages().link(None, &id, None));
                    let artifact = server.artifacts.get(&id);
                    Ok(Step::Done(json!({
                        "artifactId": id.0,
                        "version": version,
                        "versions": artifact.map(|artifact| artifact.latest()),
                        "url": url,
                    })))
                }) as Continuation
            }
            .boxed(),
        ))
    }

    pub(super) fn artifact_list(&self, caller: Caller, arguments: &Arguments) -> Outcome {
        let everywhere = arguments.bool("all")?.unwrap_or(false);
        let mut artifacts: Vec<Value> = self
            .artifacts
            .artifacts()
            .iter()
            .filter(|artifact| {
                everywhere
                    || artifact.project_id == caller.project_id
                    || (caller.is_chat() && artifact.is_from_chat())
            })
            .map(|artifact| self.artifact_json(artifact))
            .collect();
        if artifacts.is_empty() && !everywhere {
            artifacts = self
                .artifacts
                .artifacts()
                .iter()
                .map(|artifact| self.artifact_json(artifact))
                .collect();
            return Ok(Step::Done(json!({
                "artifacts": artifacts,
                "note": "None from this project; these are every project's.",
            })));
        }
        Ok(Step::Done(json!({ "artifacts": artifacts })))
    }

    fn artifact_json(&self, artifact: &agentz_protocol::artifacts::Artifact) -> Value {
        let url = self
            .pages
            .as_ref()
            .map(|pages| pages.pages().link(None, &artifact.id, None));
        let thread = artifact
            .thread_id
            .and_then(|thread_id| self.projects.thread(thread_id));
        json!({
            "artifactId": artifact.id.0,
            "title": artifact.title,
            "kind": match artifact.kind {
                ArtifactKind::Page => "page",
                ArtifactKind::Document => "document",
            },
            "versions": artifact.latest(),
            "publishedAt": artifact.published_at(),
            "thread": thread.map(|thread| thread.title.clone()),
            "files": artifact.versions.last().map(|version| version.files.len()).unwrap_or_default(),
            "url": url,
        })
    }

    pub(super) fn artifact_read(&self, arguments: &Arguments) -> Outcome {
        let id = arguments
            .string("artifactId", 64)?
            .ok_or_else(|| invalid("artifactId is required, from agentz_artifact_list."))?;
        let id = ArtifactId(id.to_string());
        let Some(artifact) = self.artifacts.get(&id) else {
            return Err(failure("invalid_request", "There's no such artifact."));
        };
        let version = arguments
            .number("version")?
            .map(|version| version as u32)
            .unwrap_or_else(|| artifact.latest());
        let source = self
            .artifacts
            .read_source(&id, version)
            .map_err(|error| invalid(format!("{error:#}")))?;
        let (source, truncated) = super::truncate(&source, MAX_READ_CHARS);
        Ok(Step::Done(json!({
            "artifactId": id.0,
            "version": version,
            "versions": artifact.latest(),
            "title": artifact.title,
            "kind": match artifact.kind {
                ArtifactKind::Page => "page",
                ArtifactKind::Document => "document",
            },
            "files": artifact
                .version(version)
                .map(|version| version.files.iter().map(|file| {
                    json!({"name": file.name, "bytes": file.bytes})
                }).collect::<Vec<_>>())
                .unwrap_or_default(),
            "source": source,
            "truncated": truncated,
        })))
    }
}

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "agentz_artifact_publish",
            "title": "Publish an artifact",
            "description": "Publish a page for the user to open in their browser, as Claude Code's artifacts: one self-contained .html or .md file (a Markdown file shows as a styled document page), 16 MiB at most. Write the file first, then publish it by its path. Use it for what suits a page rather than a chat reply: designs to pick from with comments, reports, tables to read, benchmarks, checklists or timelines you'll republish as you go, small tools, screenshot galleries. Each publish is a new version of the page; pass artifactId from an earlier publish to add one, otherwise a new artifact is made. The answer's url opens it. On the page: use the app's theme through its CSS variables (--background, --text, --muted, --border, --accent, --surface, --hover, --font-sans, --font-mono) so it sits in the app; scripts may use only a few well-known CDNs, and images must be embedded. To let the user send picks or notes back to this thread, define window.agentzPicks = () => text; the page's Send to thread button delivers it as the user's message. files attaches downloads (a CSV, a .docx) to the version.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": {"type": "string", "maxLength": 256, "description": "What the page is, as its card and header say it."},
                    "file": {"type": "string", "maxLength": 4096, "description": "The page's file, an absolute path or one starting with ~; a .md file becomes a document page."},
                    "artifactId": {"type": "string", "maxLength": 64, "description": "Publish a new version of this artifact instead of starting one."},
                    "files": {"type": "array", "maxItems": 20, "items": {"type": "string", "maxLength": 4096}, "description": "Files to offer as downloads from the page, by path."},
                    "clientRequestId": {"type": "string", "maxLength": 256, "description": "Stable idempotency key to reuse when retrying this mutation."},
                },
                "required": ["title", "file"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false},
        }),
        json!({
            "name": "agentz_artifact_list",
            "title": "List artifacts",
            "description": "List this project's published artifacts, newest first, with each one's url, or with all, every project's and the chats'.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "all": {"type": "boolean", "description": "List every project's artifacts, not only this one's."},
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        }),
        json!({
            "name": "agentz_artifact_read",
            "title": "Read an artifact",
            "description": "Read one version of an artifact's source (the latest without version), with the files it offers.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "artifactId": {"type": "string", "maxLength": 64, "description": "From agentz_artifact_list or a publish's answer."},
                    "version": {"type": "integer", "description": "The version to read; the latest without one."},
                },
                "required": ["artifactId"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        }),
    ]
}
