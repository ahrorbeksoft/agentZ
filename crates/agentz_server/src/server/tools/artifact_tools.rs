//! The artifact tools (design/artifacts, design/artifacts-2): `agentz_document_publish`
//! publishes a Markdown file the agent wrote, and `agentz_design_publish` a design given as its
//! arguments (topics of variants, each a picture and/or a live page), a version each time.
//! `agentz_artifact_list` and `agentz_artifact_read` find them again on every machine the app
//! reaches; a design reads as its words with the user's picks and comments, and its files by
//! their paths (another machine's on request, copied over). Each publish is served to the
//! user's browser by the pages server, whose link the answer carries.

use std::path::{Path, PathBuf};

use agentz_protocol::artifacts::{
    Artifact, ArtifactId, ArtifactKind, Design, DesignDefaults, DesignTopic, DesignVariant,
    MAX_ARTIFACT_SIZE,
};
use base64::Engine as _;
use futures::FutureExt as _;
use futures::future::{BoxFuture, join_all};
use projects::ThreadId;
use serde_json::{Value, json};

use super::{Arguments, Caller, Continuation, Failure, Outcome, Server, Step, failure, invalid};
use crate::artifacts::{Content, Publish, design_text};

const MAX_PATH_CHARS: usize = 4_096;
const MAX_FILES: usize = 20;
/// The most of a document a read answers with.
const MAX_READ_CHARS: usize = 50_000;
const MAX_TOPICS: usize = 20;
const MAX_VARIANTS: usize = 12;
const MAX_TEXT_CHARS: usize = 4_000;
/// The most a design's pictures and pages hold together.
const MAX_DESIGN_BYTES: u64 = 128 * 1024 * 1024;
const PICTURES: [&str; 7] = ["png", "jpg", "jpeg", "gif", "webp", "svg", "avif"];
const PAGES: [&str; 2] = ["html", "htm"];
const SCOPES: [&str; 3] = ["thread", "project", "all"];

impl Server {
    pub(super) fn document_publish(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let thread_id = publishing_thread(caller)?;
        let title = title(arguments)?;
        let file = path(arguments, "file")?.ok_or_else(|| invalid("file is required."))?;
        let mut attachments = Vec::new();
        if let Some(paths) = arguments.array("files")? {
            if paths.len() > MAX_FILES {
                return Err(invalid(format!("at most {MAX_FILES} files.")));
            }
            for path in paths {
                let Some(path) = path.as_str() else {
                    return Err(invalid("files holds paths."));
                };
                attachments.push(absolute(path, "each file")?);
            }
        }
        let update = artifact_id(arguments)?;
        let content = async move {
            let source = tokio::fs::read_to_string(&file)
                .await
                .map_err(|error| unreadable(&file, error))?;
            let mut files = Vec::with_capacity(attachments.len());
            for path in attachments {
                let bytes = read_file(&path).await?;
                files.push((file_name_of(&path), bytes));
            }
            Ok(Content::Document { source, files })
        };
        Ok(publish_then(thread_id, update, title, content.boxed()))
    }

    pub(super) fn design_publish(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let thread_id = publishing_thread(caller)?;
        let title = title(arguments)?;
        let intro = text(arguments, "intro", MAX_TEXT_CHARS)?;
        let topics = arguments
            .array("topics")?
            .ok_or_else(|| invalid("topics is required: at least one, each with its variants."))?;
        if topics.is_empty() {
            return Err(invalid("topics needs at least one topic."));
        }
        if topics.len() > MAX_TOPICS {
            return Err(invalid(format!("at most {MAX_TOPICS} topics.")));
        }
        let mut files = DesignFiles::default();
        let mut design = Design {
            intro,
            topics: Vec::with_capacity(topics.len()),
            gone: Vec::new(),
        };
        for (index, topic) in topics.iter().enumerate() {
            let number = index + 1;
            let Value::Object(topic) = topic else {
                return Err(invalid(format!("topic {number} must be an object.")));
            };
            let topic = Arguments(topic);
            let title = topic
                .string("title", 256)?
                .ok_or_else(|| invalid(format!("topic {number} needs a title.")))?
                .to_string();
            let description = text(&topic, "description", MAX_TEXT_CHARS)?;
            let multi = topic.bool("multi")?.unwrap_or(false);
            let today = match topic.0.get("today") {
                None | Some(Value::Null) => None,
                Some(Value::Object(today)) => Some(variant(
                    &Arguments(today),
                    number,
                    agentz_protocol::artifacts::TODAY,
                    false,
                    &mut files,
                )?),
                Some(_) => {
                    return Err(invalid(format!(
                        "topic {number}'s today must be an object."
                    )));
                }
            };
            if multi && today.is_some() {
                return Err(invalid(format!(
                    "topic {number} is multi, which has no today: leave it out."
                )));
            }
            let given = topic
                .array("variants")?
                .ok_or_else(|| invalid(format!("topic {number} needs its variants.")))?;
            if given.is_empty() {
                return Err(invalid(format!(
                    "topic {number} needs at least one variant."
                )));
            }
            if given.len() > MAX_VARIANTS {
                return Err(invalid(format!(
                    "topic {number} has over {MAX_VARIANTS} variants."
                )));
            }
            let mut variants = Vec::with_capacity(given.len());
            for (place, given) in given.iter().enumerate() {
                let Value::Object(given) = given else {
                    return Err(invalid(format!(
                        "topic {number}'s variants must be objects."
                    )));
                };
                variants.push(variant(
                    &Arguments(given),
                    number,
                    &DesignTopic::letter(place),
                    multi,
                    &mut files,
                )?);
            }
            // An agent may name the one it recommends by its letter.
            let recommended = Some(text(&topic, "recommended", 256)?)
                .filter(|recommended| !recommended.is_empty())
                .map(|recommended| {
                    if variants.iter().any(|variant| variant.name == recommended) {
                        return recommended;
                    }
                    (0..variants.len())
                        .find(|place| {
                            DesignTopic::letter(*place).eq_ignore_ascii_case(&recommended)
                        })
                        .map(|place| variants[place].name.clone())
                        .unwrap_or(recommended)
                });
            design.topics.push(DesignTopic {
                title,
                description,
                today,
                variants,
                recommended,
                multi,
            });
        }
        // The description tells agents the defaults, but a session may have listed its tools
        // before the user changed them, and agents treat a description as advice. The check
        // answers with the defaults as they are now.
        let defaults = self.artifacts.design_defaults(caller.project_id);
        let deviations = defaults.deviations(&design);
        if !deviations.is_empty() {
            let reason = text(arguments, "deviationReason", MAX_TEXT_CHARS)?;
            let deviations = deviations.join("; ");
            if reason.is_empty() {
                return Err(invalid(format!(
                    "This design isn't as the user's design defaults say ({}) because {deviations}. \
                     Make it as they say. If this design needs to differ, publish it again with \
                     deviationReason: one sentence on why, which the user sees.",
                    defaults.summary().trim_end_matches('.'),
                )));
            }
            let note = format!("Not as your design defaults say ({deviations}): {reason}");
            design.intro = if design.intro.is_empty() {
                note
            } else {
                format!("{}\n\n{note}", design.intro)
            };
        }
        let update = artifact_id(arguments)?;
        let content = async move {
            let mut read = Vec::with_capacity(files.files.len());
            let mut total = 0u64;
            for (path, name) in files.files {
                let bytes = read_file(&path).await?;
                total += bytes.len() as u64;
                if total > MAX_DESIGN_BYTES {
                    return Err(invalid(format!(
                        "the design's pictures and pages are over {} MiB together.",
                        MAX_DESIGN_BYTES / 1024 / 1024
                    )));
                }
                read.push((name, bytes));
            }
            Ok(Content::Design {
                design,
                files: read,
            })
        };
        Ok(publish_then(thread_id, update, title, content.boxed()))
    }

    /// This machine's artifacts in the scope, then, for the project or all, those of the other
    /// machines the app reaches, each naming its machine.
    pub(super) fn artifact_list(&mut self, caller: Caller, arguments: &Arguments) -> Outcome {
        let scope = arguments.string("scope", 16)?.unwrap_or("project");
        if !SCOPES.contains(&scope) {
            return Err(invalid("scope is thread, project or all."));
        }
        if scope == "thread" && caller.thread_id.is_none() {
            return Err(invalid(
                "Only a thread has artifacts of its own; list the project's or all.",
            ));
        }
        let (this_machine, machines) = self.relays.machines();
        let this_machine = this_machine.unwrap_or_else(|| "this machine".to_string());
        let local: Vec<Value> = self
            .artifacts
            .artifacts()
            .iter()
            .filter(|artifact| match scope {
                "thread" => artifact.thread_id == caller.thread_id,
                "project" if caller.is_chat() => artifact.is_from_chat(),
                "project" => {
                    caller.project_id.is_some() && artifact.project_id == caller.project_id
                }
                _ => true,
            })
            .map(|artifact| self.artifact_json(artifact, &this_machine))
            .collect();
        if caller.relayed || scope == "thread" {
            return Ok(Step::Done(json!({ "artifacts": local })));
        }
        // A project's artifacts are on the machines with its checkouts; a chat's and all of
        // them, on every machine.
        let others: Vec<(String, bool)> = machines
            .iter()
            .filter(|machine| {
                scope == "all"
                    || caller.is_chat()
                    || self.relays.has_checkout(caller.project_id, &machine.name)
            })
            .map(|machine| (machine.name.clone(), machine.online))
            .collect();
        let mut lists = Vec::new();
        let mut offline = Vec::new();
        for (machine, online) in others {
            if !online {
                offline.push(machine);
                continue;
            }
            let needs_project = scope == "project" && !caller.is_chat();
            let list = match self.relay_target(caller, &machine, needs_project) {
                Ok(target) => self.relay(target, "agentz_artifact_list", json!({ "scope": scope })),
                Err(failure) => async move { Err(failure) }.boxed(),
            };
            lists.push(async move {
                let list = tokio::time::timeout(super::relay::LIST_RELAY_TIMEOUT, list)
                    .await
                    .unwrap_or_else(|_| {
                        Err(failure(
                            "machine_unavailable",
                            format!("{machine} didn't answer in time."),
                        ))
                    });
                (machine, list)
            });
        }
        if lists.is_empty() && offline.is_empty() {
            return Ok(Step::Done(json!({ "artifacts": local })));
        }
        Ok(Step::Background(
            async move {
                let mut artifacts = local;
                let mut unreached: Vec<Value> = offline
                    .into_iter()
                    .map(|machine| json!({"machine": machine, "error": "not connected"}))
                    .collect();
                for (machine, list) in join_all(lists).await {
                    match list {
                        Ok(list) => {
                            for mut artifact in
                                list["artifacts"].as_array().cloned().unwrap_or_default()
                            {
                                // Only the app's own machine's pages reach the user's browser.
                                if let Value::Object(artifact) = &mut artifact {
                                    artifact.remove("url");
                                    artifact.insert("machine".into(), json!(machine));
                                }
                                artifacts.push(artifact);
                            }
                        }
                        Err(failure) => {
                            unreached.push(json!({"machine": machine, "error": failure.message}))
                        }
                    }
                }
                // RFC 3339 times in UTC sort as text.
                artifacts.sort_by(|a, b| {
                    let time =
                        |artifact: &Value| artifact["publishedAt"].as_str().map(str::to_string);
                    time(b).cmp(&time(a))
                });
                let mut answer = json!({ "artifacts": artifacts });
                if !unreached.is_empty() {
                    answer["unreachedMachines"] = Value::Array(unreached);
                }
                Ok(answer)
            }
            .boxed(),
        ))
    }

    fn artifact_json(&self, artifact: &Artifact, machine: &str) -> Value {
        let url = self
            .pages
            .as_ref()
            .map(|pages| pages.pages().link(None, &artifact.id, None));
        let listing = self.listing(artifact);
        let mut value = json!({
            "artifactId": artifact.id.0,
            "title": artifact.title,
            "kind": artifact.kind.name(),
            "version": artifact.latest(),
            "publishedAt": super::timestamp(
                std::time::UNIX_EPOCH + std::time::Duration::from_millis(artifact.published_at())
            ),
            "from": listing.place,
            "machine": machine,
            "url": url,
        });
        if listing.archived {
            value["archivedThread"] = json!(true);
        }
        if let Some(design) = &artifact.design {
            value["state"] = json!(design.status());
            value["size"] = json!(design.size());
        }
        value
    }

    /// A document's Markdown, or a design's picks and comments as text (topic 24 B); with
    /// `file`, one of the version's files, by its path on this machine.
    pub(super) fn artifact_read(&self, caller: Caller, arguments: &Arguments) -> Outcome {
        let id = arguments
            .string("artifactId", 64)?
            .ok_or_else(|| invalid("artifactId is required, from agentz_artifact_list."))?;
        let id = ArtifactId(id.to_string());
        let Some(artifact) = self.artifacts.get(&id) else {
            return Err(invalid(format!(
                "There's no artifact {} on this machine. agentz_artifact_list names each one's \
                 machine; pass it as machine.",
                id.0
            )));
        };
        let version = arguments
            .number("version")?
            .map(|version| version as u32)
            .unwrap_or_else(|| artifact.latest());
        let Some(published) = artifact.version(version) else {
            return Err(invalid(format!(
                "{} has versions 1 to {}.",
                artifact.title,
                artifact.latest()
            )));
        };
        if let Some(name) = arguments.string("file", 256)? {
            let path = self
                .artifacts
                .file_path(&id, version, name)
                .map_err(|error| invalid(format!("{error:#}")))?;
            let name = name.to_string();
            let answer = json!({
                "artifactId": id.0,
                "version": version,
                "file": name,
            });
            if !caller.relayed {
                let mut answer = answer;
                answer["path"] = json!(path);
                return Ok(Step::Done(answer));
            }
            // The calling machine keeps it where its agent can read it.
            return Ok(Step::Background(
                async move {
                    let bytes = read_file(&path).await?;
                    let mut answer = answer;
                    answer["base64"] =
                        json!(base64::engine::general_purpose::STANDARD.encode(bytes));
                    Ok(answer)
                }
                .boxed(),
            ));
        }
        let listing = self.listing(artifact);
        let text = match artifact.kind {
            ArtifactKind::Document => self
                .artifacts
                .read_source(&id, version)
                .map_err(|error| invalid(format!("{error:#}")))?,
            ArtifactKind::Design => {
                let design = self
                    .artifacts
                    .read_design(&id, version)
                    .map_err(|error| invalid(format!("{error:#}")))?;
                let picks = self.artifacts.read_picks(&id, version);
                design_text(&artifact.title, version, &listing.place, &design, &picks)
            }
        };
        let (text, truncated) = super::truncate(&text, MAX_READ_CHARS);
        // A relayed read's paths would be on this machine, not the caller's.
        let folder = (!caller.relayed).then(|| self.artifacts.files_folder(&id, version));
        let files: Vec<Value> = published
            .files
            .iter()
            .map(|file| {
                let mut value = json!({"name": file.name, "bytes": file.bytes});
                if let Some(folder) = &folder {
                    value["path"] = json!(folder.join(&file.name));
                }
                value
            })
            .collect();
        let mut answer = json!({
            "artifactId": id.0,
            "title": artifact.title,
            "kind": artifact.kind.name(),
            "version": version,
            "versions": artifact.latest(),
            "from": listing.place,
            "text": text,
            "files": files,
        });
        if truncated {
            answer["truncated"] = json!(true);
        }
        if artifact.kind == ArtifactKind::Design {
            answer["note"] = json!(if caller.relayed {
                "text names each variant's files. They're on another machine: pass file with \
                 one's name to copy it here and get its path, to look at a picture or read a \
                 page."
            } else {
                "text names each variant's files, and files gives each one's path, to look at \
                 a picture or read a page."
            });
        }
        Ok(Step::Done(answer))
    }

    /// What a publish answers: the link, and for a design, what came over from the version
    /// before.
    fn published_json(&self, id: &ArtifactId, version: u32) -> Value {
        let url = self
            .pages
            .as_ref()
            .map(|pages| pages.pages().link(None, id, None));
        let mut answer = json!({
            "artifactId": id.0,
            "version": version,
            "url": url,
        });
        let is_design = self
            .artifacts
            .get(id)
            .is_some_and(|artifact| artifact.kind == ArtifactKind::Design);
        if is_design {
            let picks = self.artifacts.read_picks(id, version);
            if let Some(from) = picks.carried_from {
                answer["picksCarriedOverFrom"] = json!(from);
            }
            if !picks.dropped.is_empty() {
                answer["picksDropped"] = json!(picks.dropped);
            }
            if let Ok(design) = self.artifacts.read_design(id, version)
                && !design.gone.is_empty()
            {
                answer["variantsGone"] = json!(design.gone);
            }
        }
        answer
    }
}

/// Publishes once the files are read off the server's task.
fn publish_then(
    thread_id: ThreadId,
    update: Option<ArtifactId>,
    title: String,
    content: BoxFuture<'static, Result<Content, Failure>>,
) -> Step {
    Step::Then(
        async move {
            let content = content.await;
            Box::new(move |server: &mut Server| {
                let content = content?;
                let thread = server
                    .projects
                    .thread(thread_id)
                    .ok_or_else(|| failure("thread_not_found", "The thread is gone."))?;
                let project_id = thread.project_id;
                let agent_id = thread
                    .agent_id
                    .clone()
                    .map(agentz_protocol::agents::AgentId::new);
                let (id, version) = server
                    .artifacts
                    .publish(Publish {
                        id: update,
                        title,
                        content,
                        thread_id,
                        project_id,
                        agent_id,
                    })
                    .map_err(|error| invalid(format!("{error:#}")))?;
                server.artifacts_changed(Some(id.clone()));
                Ok(Step::Done(server.published_json(&id, version)))
            }) as Continuation
        }
        .boxed(),
    )
}

/// A file another machine's read gave as base64, kept in this machine's data folder so the
/// agent reads it from its path.
pub(super) async fn keep_fetched_file(
    folder: PathBuf,
    machine: String,
    mut answer: Value,
) -> Result<Value, Failure> {
    let Some(data) = answer
        .as_object_mut()
        .and_then(|answer| answer.remove("base64"))
    else {
        return Ok(answer);
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_str().unwrap_or_default())
        .map_err(|error| {
            failure(
                "operation_failed",
                format!("The file came garbled: {error}"),
            )
        })?;
    let name = answer["file"].as_str().unwrap_or_default().to_string();
    let name = crate::artifacts::file_name(&name)
        .map_err(|error| invalid(format!("{error:#}")))?
        .to_string();
    let folder = folder
        .join(crate::artifacts::file_name(&machine).unwrap_or("machine"))
        .join(answer["artifactId"].as_str().unwrap_or_default())
        .join(answer["version"].as_u64().unwrap_or_default().to_string());
    let path = folder.join(&name);
    let written = async {
        tokio::fs::create_dir_all(&folder).await?;
        tokio::fs::write(&path, &bytes).await
    };
    written.await.map_err(|error| {
        failure(
            "operation_failed",
            format!("couldn't keep {}: {error}", path.display()),
        )
    })?;
    answer["path"] = json!(path);
    Ok(answer)
}

fn publishing_thread(caller: Caller) -> Result<ThreadId, Failure> {
    caller.thread_id.ok_or_else(|| {
        failure(
            "capability_denied",
            "Only a thread's agent can publish an artifact.",
        )
    })
}

fn title(arguments: &Arguments) -> Result<String, Failure> {
    Ok(arguments
        .string("title", 256)?
        .ok_or_else(|| invalid("title is required."))?
        .to_string())
}

fn artifact_id(arguments: &Arguments) -> Result<Option<ArtifactId>, Failure> {
    Ok(arguments
        .string("artifactId", 64)?
        .map(|id| ArtifactId(id.to_string())))
}

/// An optional text, where an empty one is none.
fn text(arguments: &Arguments, key: &str, max_chars: usize) -> Result<String, Failure> {
    match arguments.0.get(key) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(text)) => {
            let text = text.trim();
            if text.chars().count() > max_chars {
                return Err(invalid(format!(
                    "{key} is over {max_chars} characters long."
                )));
            }
            Ok(text.to_string())
        }
        Some(_) => Err(invalid(format!("{key} must be a string."))),
    }
}

fn path(arguments: &Arguments, key: &str) -> Result<Option<PathBuf>, Failure> {
    arguments
        .string(key, MAX_PATH_CHARS)?
        .map(|path| absolute(path, key))
        .transpose()
}

fn absolute(path: &str, what: &str) -> Result<PathBuf, Failure> {
    let path = crate::directories::expand_home(Path::new(path));
    if !path.is_absolute() {
        return Err(invalid(format!(
            "{what} must be an absolute path or start with ~."
        )));
    }
    Ok(path)
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn unreadable(path: &Path, error: std::io::Error) -> Failure {
    invalid(format!("{} couldn't be read: {error}", path.display()))
}

async fn read_file(path: &Path) -> Result<Vec<u8>, Failure> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|error| unreadable(path, error))?;
    if metadata.len() > MAX_ARTIFACT_SIZE {
        return Err(invalid(format!("{} is over 16 MiB.", path.display())));
    }
    tokio::fs::read(path)
        .await
        .map_err(|error| unreadable(path, error))
}

/// A design's pictures and pages by their paths, each named in the version by its topic and
/// variant ("1-B.png", "1-today-phone.png").
#[derive(Default)]
struct DesignFiles {
    files: Vec<(PathBuf, String)>,
}

impl DesignFiles {
    fn add(
        &mut self,
        arguments: &Arguments,
        key: &str,
        name: &str,
        extensions: &[&str],
    ) -> Result<Option<String>, Failure> {
        let Some(path) = path(arguments, key)? else {
            return Ok(None);
        };
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_lowercase)
            .unwrap_or_default();
        if !extensions.contains(&extension.as_str()) {
            return Err(invalid(format!(
                "{key} {} must be a .{} file.",
                path.display(),
                extensions.join(", .")
            )));
        }
        if let Some((_, name)) = self.files.iter().find(|(known, _)| *known == path) {
            return Ok(Some(name.clone()));
        }
        let name = format!("{name}.{extension}");
        self.files.push((path, name.clone()));
        Ok(Some(name))
    }
}

fn variant(
    arguments: &Arguments,
    topic: usize,
    key: &str,
    words_only: bool,
    files: &mut DesignFiles,
) -> Result<DesignVariant, Failure> {
    let is_today = key == agentz_protocol::artifacts::TODAY;
    let name = text(arguments, "name", 256)?;
    let name = if !name.is_empty() {
        name
    } else if is_today {
        "Today's".to_string()
    } else {
        return Err(invalid(format!(
            "every variant in topic {topic} needs a name."
        )));
    };
    let base = format!("{topic}-{key}");
    let picture = files.add(arguments, "picture", &base, &PICTURES)?;
    let page = files.add(arguments, "page", &base, &PAGES)?;
    let phone = format!("{base}-phone");
    let phone_picture = files.add(arguments, "phonePicture", &phone, &PICTURES)?;
    if !words_only && picture.is_none() && page.is_none() && phone_picture.is_none() {
        return Err(invalid(format!(
            "{name:?} in topic {topic} needs a picture or a page."
        )));
    }
    Ok(DesignVariant {
        name,
        description: text(arguments, "description", MAX_TEXT_CHARS)?,
        good: text(arguments, "good", MAX_TEXT_CHARS)?,
        cost: text(arguments, "cost", MAX_TEXT_CHARS)?,
        picture,
        page,
        phone_picture,
        change: None,
    })
}

pub(super) fn definitions(design_defaults: &DesignDefaults) -> Vec<Value> {
    let client_request_id = json!({"type": "string", "maxLength": 256, "description": "Stable idempotency key to reuse when retrying this mutation."});
    let variant = |what: &str| {
        json!({
            "type": "object",
            "description": what,
            "properties": {
                "name": {"type": "string", "maxLength": 256, "description": "Its name, short: \"Compare table\". Picks carry over to the next version by name, so keep it when you revise the variant."},
                "description": {"type": "string", "maxLength": MAX_TEXT_CHARS, "description": "What it is, in a sentence or two."},
                "good": {"type": "string", "maxLength": MAX_TEXT_CHARS, "description": "What's good about it."},
                "cost": {"type": "string", "maxLength": MAX_TEXT_CHARS, "description": "What it costs or gives up."},
                "picture": {"type": "string", "maxLength": MAX_PATH_CHARS, "description": "A picture of it (.png, .jpg, .webp, .gif, .svg), by its absolute path or one starting with ~: a screenshot or a mock at its real size."},
                "page": {"type": "string", "maxLength": MAX_PATH_CHARS, "description": "A live page of it, a self-contained .html file by its path, which the user can scroll and click through. Its scripts may use only a few well-known CDNs, and its images must be embedded. Make it responsive: the user resizes it to see a phone's layout, so it has no phone version."},
                "phonePicture": {"type": "string", "maxLength": MAX_PATH_CHARS, "description": "Its picture at a phone's size (about 390 px wide), when the design has phone pictures."},
            },
            "additionalProperties": false,
        })
    };
    let mut variant_required = variant(
        "A variant: a name, a picture and/or a page (one is required, except in a multi topic), and what's good and bad about it.",
    );
    variant_required["required"] = json!(["name"]);
    let today = variant(
        "What's there now, as the first card, with a Keep today's button: its words, Good and Cost, and a picture (a screenshot) or page. name defaults to \"Today's\".",
    );
    let defaults = design_defaults.instructions();
    vec![
        json!({
            "name": "agentz_document_publish",
            "title": "Publish a document",
            "description": "Publish a Markdown document for the user to read in their browser, as Claude Code's artifacts: a report, research, a spec, a plan, benchmarks, release notes; anything that reads better as a page than as a chat reply. Write the .md file first (16 MiB at most), then publish it by its path. Each publish is a new version at the same link: pass artifactId from an earlier publish to add one, otherwise a new document is made. files attaches downloads (a CSV, a .docx) to the version. The answer's url opens it. The user reads it; to talk about it they mention it in the thread. For something to pick from, publish a design with agentz_design_publish instead.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": {"type": "string", "maxLength": 256, "description": "What it is, as its card and page say it."},
                    "file": {"type": "string", "maxLength": MAX_PATH_CHARS, "description": "The Markdown file, by its absolute path or one starting with ~."},
                    "artifactId": {"type": "string", "maxLength": 64, "description": "Publish a new version of this document instead of starting one."},
                    "files": {"type": "array", "maxItems": MAX_FILES, "items": {"type": "string", "maxLength": MAX_PATH_CHARS}, "description": "Files to offer as downloads, by path."},
                    "clientRequestId": client_request_id,
                },
                "required": ["title", "file"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false},
        }),
        json!({
            "name": "agentz_design_publish",
            "title": "Publish a design",
            "description": format!("Publish a design for the user to pick from in their browser: a few variants of a thing (a page, a screen, a flow, a component, a name), which agentZ draws as cards to pick between, take parts of with + Also, and comment on. Most designs have one topic; give several only for separate decisions (the plans page, then the checkout's plan picker). To ask which features or options to build, not which one, make that a multi topic: each variant is a feature the user marks Want this or not. For each variant, make a picture (a screenshot or mock) or a live page first, then give them by their paths. Give what's there now as today, so the user can keep it. agentZ checks the design before publishing and keeps the user's picks with it: read them with agentz_artifact_read, whenever you like or once the user submits (they may say so in the thread). Each publish is a new version at the same link: pass artifactId to add one after the user's comments; picks carry over by variant name, so keep names, and a variant you leave out takes its pick with it. The answer's url opens it. {defaults}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "title": {"type": "string", "maxLength": 256, "description": "What's being designed, as its card and page say it: \"Plans page\"."},
                    "intro": {"type": "string", "maxLength": MAX_TEXT_CHARS, "description": "A line or two over the design: what you made and from what."},
                    "topics": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": MAX_TOPICS,
                        "description": "What the user decides, one topic per decision.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": {"type": "string", "maxLength": 256, "description": "The topic: \"The plans page\"."},
                                "description": {"type": "string", "maxLength": MAX_TEXT_CHARS, "description": "What's being decided, and anything to know first."},
                                "today": today,
                                "variants": {"type": "array", "minItems": 1, "maxItems": MAX_VARIANTS, "items": variant_required, "description": "The variants, lettered A, B, C… in this order."},
                                "recommended": {"type": "string", "maxLength": 256, "description": "The variant you recommend, by its name or letter."},
                                "multi": {"type": "boolean", "description": "Pick any: the variants are features or options that go together, and the user marks each one they want (or none) instead of choosing one. Then a variant may be words alone (its name and description, with a picture or page only when it helps), there's no today and no variant count or size to follow."},
                            },
                            "required": ["title", "variants"],
                            "additionalProperties": false,
                        },
                    },
                    "artifactId": {"type": "string", "maxLength": 64, "description": "Publish a new version of this design instead of starting one."},
                    "deviationReason": {"type": "string", "maxLength": MAX_TEXT_CHARS, "description": "One sentence on why this design differs from the user's design defaults, when it must (the user's own request, or what it decides needs more or less). The user sees it. Without it, a design that differs is sent back."},
                    "clientRequestId": client_request_id,
                },
                "required": ["title", "topics"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": false},
        }),
        json!({
            "name": "agentz_artifact_list",
            "title": "List artifacts",
            "description": "List published documents and designs, newest first, on every machine agentZ reaches: this thread's, this project's (the default; a chat's lists the chats'), or all. Each names its machine, where it came from, and a design's state (Waiting for you, 2 of 3 decided, Decided). Read one with agentz_artifact_read.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope": {"type": "string", "enum": SCOPES, "description": "thread, project (the default) or all."},
                },
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        }),
        json!({
            "name": "agentz_artifact_read",
            "title": "Read an artifact",
            "description": "Read an artifact, the latest version without version: a document's Markdown, or a whole design as text: its state and intro, then a heading per topic with the user's pick by letter and name, and every variant with its words, its files by name, and the user's comment. files lists the version's files with their paths, to look at a variant's picture or read its page. For an artifact on another machine, pass a file's name as file to copy it here and get its path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "artifactId": {"type": "string", "maxLength": 64, "description": "From agentz_artifact_list, a publish's answer or a mention."},
                    "version": {"type": "integer", "minimum": 1, "description": "The version to read; the latest without one."},
                    "file": {"type": "string", "maxLength": 256, "description": "One of the version's files, by name, to get its path instead of the text; from another machine, it's copied here first."},
                    "machine": {"type": "string", "maxLength": 256, "description": "The machine it's on, as agentz_artifact_list or a mention names it. Omit for this machine."},
                },
                "required": ["artifactId"],
                "additionalProperties": false,
            },
            "annotations": {"readOnlyHint": true, "destructiveHint": false, "idempotentHint": true},
        }),
    ]
}
