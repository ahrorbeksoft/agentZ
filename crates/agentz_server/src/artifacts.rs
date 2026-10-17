//! The machine's artifacts (design/artifacts, design/artifacts-2): each in `artifacts/<id>/`
//! in the data folder, with a `meta.json` ([`Artifact`]), one `pages/<version>.md|json` per
//! publish (a document's Markdown, or the [`Design`]), the version's files in
//! `files/<version>/` (a document's downloads, a design's pictures and pages), and a design's
//! picks in `picks/<version>.json`. Deleting the thread that published one keeps it; deleting
//! the artifact removes the folder.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use agentz_protocol::agents::AgentId;
use agentz_protocol::artifacts::{
    Artifact, ArtifactFile, ArtifactId, ArtifactKind, ArtifactVersion, Design, DesignDefaults,
    DesignPicks, DesignSettings, DesignState, DesignTopic, DesignVariant, MAX_ARTIFACT_SIZE, TODAY,
    TopicPicks, VariantChange,
};
use anyhow::{Context as _, Result, anyhow, bail, ensure};
use projects::{ProjectId, ThreadId};
use util::ResultExt as _;

const MAX_TITLE_CHARS: usize = 256;
const MAX_FILE_NAME_CHARS: usize = 256;
const MAX_PREVIEW_CHARS: usize = 280;
/// The most a comment holds, and a design's words in all.
const MAX_COMMENT_CHARS: usize = 4_000;
const MAX_DESIGN_CHARS: usize = 200_000;

/// What a publish brings.
pub(crate) struct Publish {
    /// The artifact to add a version to; `None` starts one.
    pub id: Option<ArtifactId>,
    pub title: String,
    pub content: Content,
    pub thread_id: ThreadId,
    pub project_id: ProjectId,
    pub agent_id: Option<AgentId>,
}

pub(crate) enum Content {
    /// The Markdown, and files to offer as downloads: `(name, bytes)`.
    Document {
        source: String,
        files: Vec<(String, Vec<u8>)>,
    },
    /// The design, naming its pictures and pages, which come along: `(name, bytes)`.
    Design {
        design: Design,
        files: Vec<(String, Vec<u8>)>,
    },
}

impl Content {
    fn kind(&self) -> ArtifactKind {
        match self {
            Content::Document { .. } => ArtifactKind::Document,
            Content::Design { .. } => ArtifactKind::Design,
        }
    }
}

pub(crate) struct ArtifactStore {
    folder: PathBuf,
    artifacts: Vec<Artifact>,
    /// The design defaults the app last sent (`design-defaults.json`), kept for agents that
    /// start while it isn't connected.
    design_settings: DesignSettings,
    /// Bumped on every change, so the server's session subscribers are sent the new list.
    revision: u64,
}

impl ArtifactStore {
    pub fn load(data_dir: &Path) -> Self {
        let folder = data_dir.join("artifacts");
        let mut artifacts = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&folder) {
            for entry in entries.flatten() {
                let meta = entry.path().join("meta.json");
                let Ok(bytes) = std::fs::read(&meta) else {
                    continue;
                };
                match serde_json::from_slice::<Artifact>(&bytes) {
                    Ok(artifact) => artifacts.push(artifact),
                    // A half-written meta, or a plain HTML page from before designs, doesn't
                    // keep the rest from loading.
                    Err(error) => {
                        log::warn!("skipping {}: {error:#}", meta.display());
                    }
                }
            }
        }
        let design_settings = std::fs::read(folder.join("design-defaults.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let mut store = Self {
            folder,
            artifacts,
            design_settings,
            revision: 0,
        };
        store.sort();
        store
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Newest publish first.
    fn sort(&mut self) {
        self.artifacts
            .sort_by_key(|artifact| std::cmp::Reverse(artifact.published_at()));
    }

    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    pub fn get(&self, id: &ArtifactId) -> Option<&Artifact> {
        self.artifacts.iter().find(|artifact| &artifact.id == id)
    }

    fn position(&self, id: &ArtifactId) -> Result<usize> {
        self.artifacts
            .iter()
            .position(|artifact| &artifact.id == id)
            .ok_or_else(|| anyhow!("no such artifact"))
    }

    /// Adds a version, or starts the artifact. Returns it with its new version's number.
    pub fn publish(&mut self, publish: Publish) -> Result<(ArtifactId, u32)> {
        let title = publish.title.trim();
        ensure!(!title.is_empty(), "a title is required");
        ensure!(
            title.chars().count() <= MAX_TITLE_CHARS,
            "the title is over {MAX_TITLE_CHARS} characters"
        );
        let kind = publish.content.kind();
        let now = millis(SystemTime::now());
        let position = match &publish.id {
            Some(id) => {
                let position = self
                    .artifacts
                    .iter()
                    .position(|artifact| &artifact.id == id)
                    .ok_or_else(|| anyhow!("there's no artifact {} to update", id.0))?;
                ensure!(
                    self.artifacts[position].kind == kind,
                    "{} is a {}; publish it again with the {} tool",
                    id.0,
                    self.artifacts[position].kind.name(),
                    self.artifacts[position].kind.name()
                );
                Some(position)
            }
            None => None,
        };
        let (source, files, design) = match publish.content {
            Content::Document { source, files } => {
                ensure!(
                    source.len() as u64 <= MAX_ARTIFACT_SIZE,
                    "the document is over 16 MiB"
                );
                (source, files, None)
            }
            Content::Design { mut design, files } => {
                check_design(&design, &files)?;
                let before = position.and_then(|position| {
                    let artifact = &self.artifacts[position];
                    let version = artifact.latest();
                    let design = self.read_design(&artifact.id, version).log_err()?;
                    let picks = self.read_picks(&artifact.id, version);
                    Some((artifact.id.clone(), version, design, picks))
                });
                let picks = match before {
                    Some((id, version, old, picks)) => {
                        let old_files = |name: &str| self.read_file(&id, version, name).ok();
                        carry_over(&old, &picks, version, &mut design, &files, old_files)
                    }
                    None => DesignPicks::default(),
                };
                let source =
                    serde_json::to_string_pretty(&design).context("encoding the design")?;
                (source, files, Some((design, picks)))
            }
        };
        let mut entries = Vec::with_capacity(files.len());
        for (name, bytes) in &files {
            let name = file_name(name)?;
            ensure!(
                bytes.len() as u64 <= MAX_ARTIFACT_SIZE,
                "{name} is over 16 MiB"
            );
            entries.push(ArtifactFile {
                name: name.to_string(),
                bytes: bytes.len() as u64,
            });
        }
        let position = match position {
            Some(position) => position,
            None => {
                let id = self.new_id();
                self.artifacts.push(Artifact {
                    id,
                    title: String::new(),
                    kind,
                    versions: Vec::new(),
                    thread_id: None,
                    project_id: None,
                    agent_id: None,
                    preview: String::new(),
                    design: None,
                    frame_key: key(),
                });
                self.artifacts.len() - 1
            }
        };
        let folder = self.folder.join(&self.artifacts[position].id.0);
        let version = self.artifacts[position].latest() + 1;
        // The files go first, so a version is never listed without them.
        write(
            &folder.join("pages"),
            &page_name(version, kind),
            source.as_bytes(),
        )?;
        for (entry, (_, bytes)) in entries.iter().zip(files.iter()) {
            write(
                &folder.join("files").join(version.to_string()),
                &entry.name,
                bytes,
            )?;
        }
        let artifact = &mut self.artifacts[position];
        artifact.title = title.to_string();
        artifact.thread_id = Some(publish.thread_id);
        artifact.project_id = Some(publish.project_id);
        artifact.agent_id = publish.agent_id;
        artifact.versions.push(ArtifactVersion {
            published_at: now,
            bytes: source.len() as u64,
            files: entries,
        });
        match &design {
            Some((design, picks)) => {
                artifact.preview = preview_of_design(design);
                artifact.design = Some(design_state(design, picks));
            }
            None => artifact.preview = preview_of_document(&source, title),
        }
        let id = artifact.id.clone();
        if let Some((_, picks)) = &design {
            self.save_picks(&id, version, picks)?;
        }
        self.save_at(position)?;
        self.sort();
        self.revision += 1;
        Ok((id, version))
    }

    /// A version's source: a document's Markdown, or a design's JSON.
    pub fn read_source(&self, id: &ArtifactId, version: u32) -> Result<String> {
        let artifact = self.get(id).ok_or_else(|| anyhow!("no such artifact"))?;
        artifact
            .version(version)
            .ok_or_else(|| anyhow!("the artifact has no version {version}"))?;
        let path = self
            .folder
            .join(&id.0)
            .join("pages")
            .join(page_name(version, artifact.kind));
        let source = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        Ok(source)
    }

    pub fn read_design(&self, id: &ArtifactId, version: u32) -> Result<Design> {
        let artifact = self.get(id).ok_or_else(|| anyhow!("no such artifact"))?;
        ensure!(
            artifact.kind == ArtifactKind::Design,
            "{} isn't a design",
            artifact.title
        );
        let source = self.read_source(id, version)?;
        serde_json::from_str(&source).context("parsing the design")
    }

    /// A design version's picks; none yet is empty.
    pub fn read_picks(&self, id: &ArtifactId, version: u32) -> DesignPicks {
        let path = self
            .folder
            .join(&id.0)
            .join("picks")
            .join(format!("{version}.json"));
        std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).log_err())
            .unwrap_or_default()
    }

    fn save_picks(&self, id: &ArtifactId, version: u32, picks: &DesignPicks) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(picks).context("encoding picks")?;
        write(
            &self.folder.join(&id.0).join("picks"),
            &format!("{version}.json"),
            &bytes,
        )
    }

    /// Saves a topic's picks on the latest version. Changing them takes back a Submit.
    pub fn set_picks(
        &mut self,
        id: &ArtifactId,
        version: u32,
        topic: usize,
        picks: TopicPicks,
    ) -> Result<()> {
        let position = self.position(id)?;
        ensure!(
            version == self.artifacts[position].latest(),
            "only the latest version takes picks"
        );
        let design = self.read_design(id, version)?;
        let design_topic = design
            .topics
            .get(topic)
            .ok_or_else(|| anyhow!("the design has no topic {}", topic + 1))?;
        let picks = clean_picks(design_topic, picks)?;
        let mut all = self.read_picks(id, version);
        if all.topics.len() < design.topics.len() {
            all.topics
                .resize_with(design.topics.len(), TopicPicks::default);
        }
        if all.topics[topic] == picks {
            return Ok(());
        }
        // A comment is a change too, but only a pick takes back the Submit (topic 16 A).
        let before = &all.topics[topic];
        if before.pick != picks.pick
            || before.also != picks.also
            || before.wanted != picks.wanted
            || before.none != picks.none
        {
            all.submitted_at = None;
            all.sent = false;
        }
        all.topics[topic] = picks;
        self.save_picks(id, version, &all)?;
        self.artifacts[position].design = Some(design_state(&design, &all));
        self.save_at(position)?;
        self.revision += 1;
        Ok(())
    }

    /// Marks the latest version decided (topic 16 A), `sent` when it went to the thread too.
    pub fn submit(&mut self, id: &ArtifactId, version: u32, sent: bool) -> Result<DesignPicks> {
        let position = self.position(id)?;
        ensure!(
            version == self.artifacts[position].latest(),
            "only the latest version can be submitted"
        );
        let design = self.read_design(id, version)?;
        let mut picks = self.read_picks(id, version);
        picks.submitted_at = Some(millis(SystemTime::now()));
        picks.sent = sent;
        self.save_picks(id, version, &picks)?;
        self.artifacts[position].design = Some(design_state(&design, &picks));
        self.save_at(position)?;
        self.revision += 1;
        Ok(picks)
    }

    /// A file of a version: a download, or a design's picture or page.
    pub fn read_file(&self, id: &ArtifactId, version: u32, name: &str) -> Result<Vec<u8>> {
        let path = self.file_path(id, version, name)?;
        std::fs::read(&path).with_context(|| format!("reading {}", path.display()))
    }

    /// Where a file of a version is kept.
    pub fn file_path(&self, id: &ArtifactId, version: u32, name: &str) -> Result<PathBuf> {
        let name = file_name(name)?;
        let artifact = self.get(id).ok_or_else(|| anyhow!("no such artifact"))?;
        let known = artifact
            .version(version)
            .ok_or_else(|| anyhow!("the artifact has no version {version}"))?
            .files
            .iter()
            .any(|file| file.name == name);
        ensure!(known, "the version has no file {name}");
        Ok(self.files_folder(id, version).join(name))
    }

    /// Where a version's files are kept, each by its name ([`ArtifactVersion::files`]).
    pub fn files_folder(&self, id: &ArtifactId, version: u32) -> PathBuf {
        self.folder
            .join(&id.0)
            .join("files")
            .join(version.to_string())
    }

    /// Deletes the artifact and its folder.
    pub fn delete(&mut self, id: &ArtifactId) -> Result<()> {
        let Some(position) = self
            .artifacts
            .iter()
            .position(|artifact| &artifact.id == id)
        else {
            bail!("no such artifact");
        };
        self.artifacts.remove(position);
        let folder = self.folder.join(&id.0);
        if folder.exists() {
            std::fs::remove_dir_all(&folder)
                .with_context(|| format!("removing {}", folder.display()))?;
        }
        self.revision += 1;
        Ok(())
    }

    /// The design defaults for a project's agents: its own, or General's.
    pub fn design_defaults(&self, project_id: Option<ProjectId>) -> DesignDefaults {
        self.design_settings.for_project(project_id)
    }

    pub fn set_design_settings(&mut self, settings: DesignSettings) -> Result<()> {
        if settings == self.design_settings {
            return Ok(());
        }
        let bytes = serde_json::to_vec_pretty(&settings).context("encoding design defaults")?;
        write(&self.folder, "design-defaults.json", &bytes)?;
        self.design_settings = settings;
        Ok(())
    }

    /// Every artifact whose words hold all of the query's, with the line that does: a
    /// document's, or a design's topics, variants and comments (topic 5 A). A match in the
    /// title alone has no line.
    pub fn search(&self, query: &str) -> Vec<(ArtifactId, String)> {
        let words: Vec<String> = query
            .split_whitespace()
            .map(str::to_lowercase)
            .filter(|word| !word.is_empty())
            .collect();
        if words.is_empty() {
            return Vec::new();
        }
        let holds = |text: &str| {
            let text = text.to_lowercase();
            words.iter().all(|word| text.contains(word))
        };
        let mut found = Vec::new();
        for artifact in &self.artifacts {
            let lines = self.words_of(artifact).unwrap_or_default();
            if let Some(line) = lines.iter().find(|line| holds(line)) {
                found.push((artifact.id.clone(), line.clone()));
            } else if holds(&artifact.title) || holds(&lines.join(" ")) {
                found.push((artifact.id.clone(), String::new()));
            }
        }
        found
    }

    /// The latest version's lines to search: a document's, or a design's words.
    fn words_of(&self, artifact: &Artifact) -> Result<Vec<String>> {
        let version = artifact.latest();
        match artifact.kind {
            ArtifactKind::Document => Ok(self
                .read_source(&artifact.id, version)?
                .lines()
                .map(|line| plain_line(line))
                .filter(|line| !line.is_empty())
                .collect()),
            ArtifactKind::Design => {
                let design = self.read_design(&artifact.id, version)?;
                let picks = self.read_picks(&artifact.id, version);
                let mut lines = Vec::new();
                if !design.intro.is_empty() {
                    lines.push(design.intro.clone());
                }
                for (index, topic) in design.topics.iter().enumerate() {
                    lines.push(format!("{}. {}", index + 1, topic.title));
                    if !topic.description.is_empty() {
                        lines.push(format!("{}: {}", topic.title, topic.description));
                    }
                    let variants = topic
                        .today
                        .iter()
                        .map(|today| (TODAY.to_string(), today))
                        .chain(
                            topic
                                .variants
                                .iter()
                                .map(|variant| (variant.name.clone(), variant)),
                        );
                    for (key, variant) in variants {
                        let label = topic.label(&key);
                        for text in [&variant.description, &variant.good, &variant.cost] {
                            if !text.is_empty() {
                                lines.push(format!("{label}: {text}"));
                            }
                        }
                        if variant.description.is_empty() {
                            lines.push(label);
                        }
                    }
                    for (key, comment) in &picks.topic(index).comments {
                        lines.push(format!("Comment on {}: {comment}", topic.short_label(key)));
                    }
                }
                Ok(lines)
            }
        }
    }

    fn save_at(&self, position: usize) -> Result<()> {
        let artifact = &self.artifacts[position];
        let bytes = serde_json::to_vec_pretty(artifact).context("encoding an artifact")?;
        write(&self.folder.join(&artifact.id.0), "meta.json", &bytes)
    }

    fn new_id(&self) -> ArtifactId {
        loop {
            let id: String = uuid::Uuid::new_v4()
                .simple()
                .to_string()
                .chars()
                .take(8)
                .collect();
            if !self.artifacts.iter().any(|artifact| artifact.id.0 == id) {
                return ArtifactId(id);
            }
        }
    }
}

/// What agentZ checks before publishing a design (topic 23 B): every name given once, today's
/// key left to today's, and every picture and page named among the files.
fn check_design(design: &Design, files: &[(String, Vec<u8>)]) -> Result<()> {
    ensure!(
        !design.topics.is_empty(),
        "a design needs at least one topic"
    );
    let mut topics = std::collections::HashSet::new();
    let mut words = design.intro.len();
    for (index, topic) in design.topics.iter().enumerate() {
        let number = index + 1;
        ensure!(
            !topic.title.trim().is_empty(),
            "topic {number} needs a title"
        );
        ensure!(
            topics.insert(topic.title.trim().to_lowercase()),
            "two topics are called {:?}",
            topic.title
        );
        ensure!(
            !topic.variants.is_empty(),
            "topic {number} ({}) has no variants",
            topic.title
        );
        ensure!(
            !(topic.multi && topic.today.is_some()),
            "topic {number} ({}) is multi, so it has no today's version",
            topic.title
        );
        let mut names = std::collections::HashSet::new();
        for variant in topic.today.iter().chain(&topic.variants) {
            ensure!(
                !variant.name.trim().is_empty(),
                "every variant in topic {number} needs a name"
            );
            words += variant.name.len()
                + variant.description.len()
                + variant.good.len()
                + variant.cost.len();
            for name in [
                &variant.picture,
                &variant.page,
                &variant.phone_picture,
                &variant.phone_page,
            ]
            .into_iter()
            .flatten()
            {
                ensure!(
                    files.iter().any(|(file, _)| file == name),
                    "{name} isn't among the design's files"
                );
            }
            // A multi topic's features can be told in words alone.
            ensure!(
                topic.multi
                    || variant.picture.is_some()
                    || variant.page.is_some()
                    || variant.phone_picture.is_some()
                    || variant.phone_page.is_some(),
                "{:?} in topic {number} needs a picture or a page",
                variant.name
            );
        }
        for variant in &topic.variants {
            ensure!(
                !variant.name.eq_ignore_ascii_case(TODAY),
                "a variant can't be called {TODAY:?}; give what's there now as the topic's today"
            );
            ensure!(
                names.insert(variant.name.to_lowercase()),
                "two variants in topic {number} are called {:?}",
                variant.name
            );
        }
        if let Some(recommended) = &topic.recommended {
            ensure!(
                topic.variant(recommended).is_some(),
                "topic {number} recommends {recommended:?}, which isn't one of its variants"
            );
        }
    }
    ensure!(
        words <= MAX_DESIGN_CHARS,
        "the design's words are over {MAX_DESIGN_CHARS} characters"
    );
    Ok(())
}

/// The picks of the version before, moved to this one by variant name (topic 19 A): a changed
/// variant is marked Changed and a new one New; one that's gone takes its pick and comment with
/// it. A submitted design waits for the user again.
fn carry_over(
    old: &Design,
    old_picks: &DesignPicks,
    old_version: u32,
    design: &mut Design,
    files: &[(String, Vec<u8>)],
    old_file: impl Fn(&str) -> Option<Vec<u8>>,
) -> DesignPicks {
    let file = |name: &Option<String>| -> Option<&[u8]> {
        let name = name.as_ref()?;
        files
            .iter()
            .find(|(file, _)| file == name)
            .map(|(_, bytes)| bytes.as_slice())
    };
    let several = design.topics.len() > 1 || old.topics.len() > 1;
    let mut picks = DesignPicks {
        topics: Vec::with_capacity(design.topics.len()),
        submitted_at: None,
        sent: false,
        carried_from: None,
        dropped: Vec::new(),
    };
    let mut gone = Vec::new();
    let mut matched = vec![false; old.topics.len()];
    for (index, topic) in design.topics.iter_mut().enumerate() {
        let old_index = old
            .topics
            .iter()
            .position(|old_topic| old_topic.title == topic.title)
            .or_else(|| {
                // A renamed topic in the same place is the same topic.
                old.topics
                    .get(index)
                    .filter(|old_topic| {
                        !design_has_title(&topic.title, &old.topics) && !old_topic.title.is_empty()
                    })
                    .map(|_| index)
            })
            .filter(|old_index| !matched[*old_index]);
        let Some(old_index) = old_index else {
            for variant in &mut topic.variants {
                variant.change = Some(VariantChange::New);
            }
            picks.topics.push(TopicPicks::default());
            continue;
        };
        matched[old_index] = true;
        let old_topic = &old.topics[old_index];
        let prefix = if several {
            format!("{}: ", topic.title)
        } else {
            String::new()
        };
        let same = |new: &agentz_protocol::artifacts::DesignVariant,
                    before: &agentz_protocol::artifacts::DesignVariant| {
            new.description == before.description
                && new.good == before.good
                && new.cost == before.cost
                && [
                    (&new.picture, &before.picture),
                    (&new.page, &before.page),
                    (&new.phone_picture, &before.phone_picture),
                    (&new.phone_page, &before.phone_page),
                ]
                .into_iter()
                .all(|(new_name, old_name)| match (new_name, old_name) {
                    (None, None) => true,
                    (Some(_), Some(old_name)) => {
                        file(new_name).map(<[u8]>::to_vec) == old_file(old_name.as_str())
                    }
                    _ => false,
                })
        };
        if let (Some(today), Some(before)) = (&mut topic.today, &old_topic.today) {
            today.change = (!same(today, before)).then_some(VariantChange::Changed);
        }
        for variant in &mut topic.variants {
            variant.change = match old_topic
                .variants
                .iter()
                .find(|before| before.name == variant.name)
            {
                Some(before) => (!same(variant, before)).then_some(VariantChange::Changed),
                None => Some(VariantChange::New),
            };
        }
        for (old_place, before) in old_topic.variants.iter().enumerate() {
            if topic.variant(&before.name).is_none() {
                gone.push(format!(
                    "{prefix}{}. {}",
                    DesignTopic::letter(old_place),
                    before.name
                ));
            }
        }
        let before = old_picks.topic(old_index);
        let exists = |key: &String| topic.variant(key).is_some();
        let mut topic_picks = TopicPicks {
            comments: before
                .comments
                .iter()
                .filter(|(key, _)| exists(key))
                .map(|(key, comment)| (key.clone(), comment.clone()))
                .collect(),
            ..TopicPicks::default()
        };
        // What the topic was before decides what comes over: a topic that has changed between
        // pick one and pick any keeps its comments only.
        if topic.multi && old_topic.multi {
            topic_picks.wanted = before
                .wanted
                .iter()
                .filter(|key| exists(key))
                .cloned()
                .collect();
            topic_picks.none = before.none;
        } else if !topic.multi && !old_topic.multi {
            topic_picks.pick = before.pick.clone().filter(exists);
            topic_picks.also = before
                .also
                .iter()
                .filter(|key| exists(key) && Some(*key) != before.pick.as_ref())
                .cloned()
                .collect();
        }
        for key in before.pick.iter().chain(&before.wanted) {
            if !exists(key) {
                picks
                    .dropped
                    .push(format!("{prefix}{}", old_topic.label(key)));
            }
        }
        picks.topics.push(topic_picks);
    }
    design.gone = gone;
    if old_picks
        .topics
        .iter()
        .any(|topic| *topic != TopicPicks::default())
    {
        picks.carried_from = Some(old_version);
    }
    picks
}

fn design_has_title(title: &str, topics: &[DesignTopic]) -> bool {
    topics.iter().any(|topic| topic.title == title)
}

/// A topic's picks as the page sent them, checked against the design: only its variants, the
/// pick not also an Also, what's wanted once each in the topic's order, comments trimmed and
/// empty ones dropped.
fn clean_picks(topic: &DesignTopic, picks: TopicPicks) -> Result<TopicPicks> {
    let known = |key: &String| topic.variant(key).is_some();
    if topic.multi {
        ensure!(
            picks.pick.is_none() && picks.also.is_empty(),
            "topic {:?} is pick any: mark what's wanted, not a pick",
            topic.title
        );
    } else {
        ensure!(
            picks.wanted.is_empty() && !picks.none,
            "topic {:?} is pick one: it has no wanted or none of these",
            topic.title
        );
    }
    for key in &picks.wanted {
        ensure!(known(key), "{key:?} isn't one of the topic's variants");
    }
    let wanted: Vec<String> = topic
        .variants
        .iter()
        .filter(|variant| picks.wanted.contains(&variant.name))
        .map(|variant| variant.name.clone())
        .collect();
    if let Some(pick) = &picks.pick {
        ensure!(known(pick), "{pick:?} isn't one of the topic's variants");
    }
    let mut also = Vec::new();
    for key in picks.also {
        ensure!(known(&key), "{key:?} isn't one of the topic's variants");
        if Some(&key) != picks.pick.as_ref() && !also.contains(&key) {
            also.push(key);
        }
    }
    let mut comments = BTreeMap::new();
    for (key, comment) in picks.comments {
        ensure!(known(&key), "{key:?} isn't one of the topic's variants");
        let comment = comment.trim();
        ensure!(
            comment.chars().count() <= MAX_COMMENT_CHARS,
            "a comment is over {MAX_COMMENT_CHARS} characters"
        );
        if !comment.is_empty() {
            comments.insert(key, comment.to_string());
        }
    }
    Ok(TopicPicks {
        pick: picks.pick,
        also,
        none: picks.none && wanted.is_empty(),
        wanted,
        comments,
    })
}

pub(crate) fn design_state(design: &Design, picks: &DesignPicks) -> DesignState {
    let first = design.topics.first();
    DesignState {
        topics: design.topics.len() as u32,
        variants: first.map_or(0, |topic| topic.variants.len() as u32),
        decided: picks.decided(design),
        submitted_at: picks.submitted_at,
        pick: match design.topics.as_slice() {
            [topic] if topic.multi => topic.answer(&picks.topic(0), true),
            [topic] => picks.topic(0).pick.map(|pick| topic.short_label(&pick)),
            _ => None,
        },
        pictures: first
            .map(|topic| {
                topic
                    .variants
                    .iter()
                    .filter_map(|variant| variant.picture.clone())
                    .take(4)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// A design as the agent reads it (topic 24 B), like a design round's README and
/// `decisions.md` together: whether it's submitted, the intro, then a heading per topic with
/// the pick by letter and name, the topic's description, and every variant, today's first,
/// with its words, its files by name, whether it's picked, an Also, wanted or recommended,
/// and its comment. An agent reading another thread's design learns what the pick was up
/// against.
pub(crate) fn design_text(
    title: &str,
    version: u32,
    place: &str,
    design: &Design,
    picks: &DesignPicks,
) -> String {
    let total = design.topics.len();
    let decided = picks.decided(design);
    let state = if picks.submitted_at.is_some() {
        "submitted".to_string()
    } else {
        "not submitted".to_string()
    };
    let mut text = format!("# {title}, v{version}: {decided} of {total} decided, {state}\n");
    text.push_str(&format!("From {place}. "));
    text.push_str(if picks.submitted_at.is_some() {
        "The user submitted these picks and comments.\n"
    } else {
        "The user's picks and comments so far.\n"
    });
    if !design.intro.is_empty() {
        text.push_str(&format!("\n{}\n", design.intro));
    }
    for (index, topic) in design.topics.iter().enumerate() {
        let topic_picks = picks.topic(index);
        text.push('\n');
        let heading = topic
            .answer(&topic_picks, false)
            .unwrap_or_else(|| "not decided".to_string());
        // Untagged variants of a pick-any topic are the ones the user doesn't want.
        let kind = if topic.multi { " (pick any)" } else { "" };
        text.push_str(&format!(
            "## {}. {}{kind}: {heading}\n",
            index + 1,
            topic.title
        ));
        if !topic.description.is_empty() {
            text.push_str(&format!("{}\n", topic.description));
        }
        let today = topic.today.iter().map(|today| (TODAY, today));
        let variants = topic
            .variants
            .iter()
            .map(|variant| (variant.name.as_str(), variant));
        for (key, variant) in today.chain(variants) {
            text.push('\n');
            variant_text(&mut text, topic, key, variant, &topic_picks);
        }
    }
    text
}

/// A variant in [`design_text`]: "**B. Compare table (picked, recommended):** Its words.",
/// then its Good, Cost, files and comment.
fn variant_text(
    text: &mut String,
    topic: &DesignTopic,
    key: &str,
    variant: &DesignVariant,
    picks: &TopicPicks,
) {
    let mut tags = Vec::new();
    if key == TODAY {
        tags.push("today's");
    }
    if picks.pick.as_deref() == Some(key) {
        tags.push("picked");
    }
    if picks.also.iter().any(|also| also == key) {
        tags.push("also");
    }
    if picks.wanted.iter().any(|wanted| wanted == key) {
        tags.push("wanted");
    }
    if key != TODAY && topic.recommended.as_deref() == Some(key) {
        tags.push("recommended");
    }
    let mut label = if key == TODAY {
        variant.name.clone()
    } else {
        topic.label(key)
    };
    if !tags.is_empty() {
        label.push_str(&format!(" ({})", tags.join(", ")));
    }
    if variant.description.is_empty() {
        text.push_str(&format!("**{label}**\n"));
    } else {
        text.push_str(&format!("**{label}:** {}\n", variant.description));
    }
    if !variant.good.is_empty() {
        text.push_str(&format!("- Good: {}\n", variant.good));
    }
    if !variant.cost.is_empty() {
        text.push_str(&format!("- Cost: {}\n", variant.cost));
    }
    let files: Vec<String> = [
        (&variant.picture, "picture"),
        (&variant.page, "page"),
        (&variant.phone_picture, "phone picture"),
        (&variant.phone_page, "phone page"),
    ]
    .into_iter()
    .filter_map(|(name, what)| Some(format!("{} ({what})", name.as_ref()?)))
    .collect();
    if !files.is_empty() {
        text.push_str(&format!("- Files: {}\n", files.join(", ")));
    }
    if let Some(comment) = picks.comments.get(key) {
        text.push_str(&format!("- The user's comment: {comment}\n"));
    }
}

/// What Submit's message says (topic 17 B): "Submitted: picked B, also C, with 2 comments."
/// ("wanted A, C" for a pick-any topic), or with several topics, each topic's pick by its
/// title.
pub(crate) fn submit_summary(design: &Design, picks: &DesignPicks) -> String {
    let short = |index: usize, topic: &DesignTopic| {
        topic
            .answer(&picks.topic(index), true)
            .unwrap_or_else(|| "not decided".to_string())
    };
    let mut text = match design.topics.as_slice() {
        [topic] if topic.multi => match topic.answer(&picks.topic(0), true) {
            Some(answer) => format!("Submitted: wanted {answer}"),
            None => "Submitted with nothing marked".to_string(),
        },
        [topic] => match topic.answer(&picks.topic(0), true) {
            Some(answer) => format!("Submitted: picked {answer}"),
            None => "Submitted with nothing picked".to_string(),
        },
        topics => {
            let each: Vec<String> = topics
                .iter()
                .enumerate()
                .map(|(index, topic)| format!("{}: {}", topic.title, short(index, topic)))
                .collect();
            format!("Submitted: {}", each.join("; "))
        }
    };
    let comments = picks.comments();
    match comments {
        0 => {}
        1 => text.push_str(", with 1 comment"),
        comments => text.push_str(&format!(", with {comments} comments")),
    }
    text.push('.');
    text
}

/// A document's first lines for its card, past a first heading that's its title.
fn preview_of_document(source: &str, title: &str) -> String {
    let mut preview = String::new();
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("```") || line.starts_with('|') {
            if !preview.is_empty() && line.is_empty() {
                break;
            }
            continue;
        }
        if line.starts_with('#') {
            if preview.is_empty() {
                continue;
            }
            break;
        }
        let plain = plain_line(line);
        if plain.eq_ignore_ascii_case(title) {
            continue;
        }
        if !preview.is_empty() {
            preview.push(' ');
        }
        preview.push_str(&plain);
        if preview.chars().count() >= MAX_PREVIEW_CHARS {
            break;
        }
    }
    shorten(&preview)
}

fn preview_of_design(design: &Design) -> String {
    let text = if design.intro.trim().is_empty() {
        design
            .topics
            .first()
            .map(|topic| topic.description.clone())
            .unwrap_or_default()
    } else {
        design.intro.clone()
    };
    shorten(text.trim())
}

fn shorten(text: &str) -> String {
    if text.chars().count() <= MAX_PREVIEW_CHARS {
        return text.to_string();
    }
    let mut short: String = text.chars().take(MAX_PREVIEW_CHARS).collect();
    short.push('…');
    short
}

/// A line of Markdown without its marks, for previews and search.
fn plain_line(line: &str) -> String {
    let line = line.trim();
    let line = line.trim_start_matches(['#', '>', ' ']);
    let line = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .unwrap_or(line);
    line.replace("**", "")
        .replace("__", "")
        .replace('`', "")
        .trim()
        .to_string()
}

/// The folder's server file: the port and token pages are served with, kept across restarts
/// and handed-off servers so open pages and links keep working.
pub(crate) struct PagesConfig {
    pub port: u16,
    pub token: String,
}

impl PagesConfig {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let path = data_dir.join("artifacts").join("server.json");
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let config = Self {
                    port: 0,
                    token: key(),
                };
                config.save(data_dir)?;
                Ok(config)
            }
            Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
        }
    }

    pub fn save(&self, data_dir: &Path) -> Result<()> {
        let bytes = serde_json::to_vec(self).context("encoding the pages server file")?;
        write(&data_dir.join("artifacts"), "server.json", &bytes)
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PagesConfigFile {
    #[serde(default)]
    port: u16,
    #[serde(default)]
    token: String,
}

impl serde::Serialize for PagesConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        PagesConfigFile {
            port: self.port,
            token: self.token.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for PagesConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let file = PagesConfigFile::deserialize(deserializer)?;
        Ok(Self {
            port: file.port,
            token: if file.token.is_empty() {
                key()
            } else {
                file.token
            },
        })
    }
}

/// The theme the app last sent, kept so pages opened while no app is connected still open in
/// it (`artifacts/theme.json`).
pub(crate) fn load_theme(data_dir: &Path) -> agentz_protocol::artifacts::PageTheme {
    let path = data_dir.join("artifacts").join("theme.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub(crate) fn save_theme(
    data_dir: &Path,
    theme: &agentz_protocol::artifacts::PageTheme,
) -> Result<()> {
    let bytes = serde_json::to_vec(theme).context("encoding the pages theme")?;
    write(&data_dir.join("artifacts"), "theme.json", &bytes)
}

fn page_name(version: u32, kind: ArtifactKind) -> String {
    format!("{version}.{}", kind.extension())
}

/// A file's name: no path, no tricks.
pub(crate) fn file_name(name: &str) -> Result<&str> {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    ensure!(
        !name.is_empty() && name != "." && name != "..",
        "a file needs a name"
    );
    ensure!(
        name.chars().count() <= MAX_FILE_NAME_CHARS,
        "a file name is over {MAX_FILE_NAME_CHARS} characters"
    );
    Ok(name)
}

fn write(folder: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    std::fs::create_dir_all(folder).with_context(|| format!("creating {}", folder.display()))?;
    let path = folder.join(name);
    let staged = folder.join(format!(".{name}.tmp"));
    std::fs::write(&staged, bytes).with_context(|| format!("writing {}", path.display()))?;
    std::fs::rename(&staged, &path).with_context(|| format!("renaming {}", path.display()))?;
    Ok(())
}

fn key() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub(crate) fn millis(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(title: &str, source: &str) -> Publish {
        Publish {
            id: None,
            title: title.to_string(),
            content: Content::Document {
                source: source.to_string(),
                files: Vec::new(),
            },
            thread_id: ThreadId(1),
            project_id: ProjectId(7),
            agent_id: None,
        }
    }

    fn variant(name: &str, description: &str) -> DesignVariant {
        DesignVariant {
            name: name.to_string(),
            description: description.to_string(),
            picture: Some(format!("{name}.png")),
            ..DesignVariant::default()
        }
    }

    fn design(id: Option<ArtifactId>, variants: &[(&str, &str, &[u8])]) -> Publish {
        let topic = DesignTopic {
            title: "The plans page".to_string(),
            variants: variants
                .iter()
                .map(|(name, description, _)| variant(name, description))
                .collect(),
            ..DesignTopic::default()
        };
        Publish {
            id,
            title: "Plans page".to_string(),
            content: Content::Design {
                design: Design {
                    intro: "Five ways to show the plans.".to_string(),
                    topics: vec![topic],
                    gone: Vec::new(),
                },
                files: variants
                    .iter()
                    .map(|(name, _, bytes)| (format!("{name}.png"), bytes.to_vec()))
                    .collect(),
            },
            thread_id: ThreadId(1),
            project_id: ProjectId(7),
            agent_id: None,
        }
    }

    #[test]
    fn publishing_keeps_every_version() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (id, version) = store
            .publish(document("Notes", "# Notes\n\nThe first lines.\n\n## More"))
            .expect("publish");
        assert_eq!(version, 1);
        assert_eq!(
            store.get(&id).expect("artifact").preview,
            "The first lines."
        );
        let (again, version) = store
            .publish(Publish {
                id: Some(id.clone()),
                ..document("Notes", "two")
            })
            .expect("republish");
        assert_eq!((again, version), (id.clone(), 2));
        assert_eq!(
            store.read_source(&id, 1).expect("v1"),
            "# Notes\n\nThe first lines.\n\n## More"
        );
        assert_eq!(store.read_source(&id, 2).expect("v2"), "two");

        // A restart loads it.
        let store = ArtifactStore::load(temp.path());
        let artifact = store.get(&id).expect("artifact after reload");
        assert_eq!(artifact.title, "Notes");
        assert_eq!(artifact.thread_id, Some(ThreadId(1)));
        assert_eq!(store.read_source(&id, 2).expect("v2 after reload"), "two");
    }

    #[test]
    fn files_come_back_and_names_are_safe() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (id, _) = store
            .publish(Publish {
                content: Content::Document {
                    source: "r".to_string(),
                    files: vec![("../report.csv".to_string(), b"a,b".to_vec())],
                },
                ..document("Report", "")
            })
            .expect("publish");
        assert_eq!(store.read_file(&id, 1, "report.csv").expect("file"), b"a,b");
        assert!(store.read_file(&id, 1, "../meta.json").is_err());
    }

    #[test]
    fn deleting_removes_the_folder() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (id, _) = store.publish(document("Notes", "one")).expect("publish");
        store.delete(&id).expect("delete");
        assert!(store.get(&id).is_none());
        assert!(!temp.path().join("artifacts").join(&id.0).exists());
    }

    #[test]
    fn a_design_is_checked_before_it_is_published() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let mut publish = design(
            None,
            &[("Three tiers", "", b"a"), ("Three tiers", "", b"b")],
        );
        assert!(store.publish(publish).is_err(), "two variants of one name");
        publish = design(None, &[("today", "", b"a")]);
        assert!(store.publish(publish).is_err(), "today is today's");
        publish = design(None, &[("Three tiers", "", b"a")]);
        if let Content::Design { files, .. } = &mut publish.content {
            files.clear();
        }
        assert!(
            store.publish(publish).is_err(),
            "a picture that isn't there"
        );
        assert!(store.artifacts().is_empty());
    }

    #[test]
    fn picks_carry_over_to_the_next_version_by_name() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (id, _) = store
            .publish(design(
                None,
                &[
                    ("Three tiers", "Cards", b"a"),
                    ("Compare table", "A table", b"b"),
                    ("One plan", "One", b"d"),
                ],
            ))
            .expect("publish");
        let artifact = store.get(&id).expect("design");
        assert_eq!(
            artifact.design.as_ref().map(DesignState::status),
            Some("Waiting for you".to_string())
        );
        store
            .set_picks(
                &id,
                1,
                0,
                TopicPicks {
                    pick: Some("One plan".to_string()),
                    also: vec!["Compare table".to_string(), "One plan".to_string()],
                    comments: [
                        ("Compare table".to_string(), " Yearly first. ".to_string()),
                        ("Three tiers".to_string(), " ".to_string()),
                    ]
                    .into(),
                    ..TopicPicks::default()
                },
            )
            .expect("picks");
        let picks = store.read_picks(&id, 1);
        assert_eq!(picks.topic(0).also, vec!["Compare table".to_string()]);
        assert_eq!(picks.comments(), 1);
        store.submit(&id, 1, false).expect("submit");
        assert_eq!(
            store
                .get(&id)
                .and_then(|artifact| artifact.design.clone())
                .map(|state| state.status()),
            Some("Decided".to_string())
        );

        // v2: the table changed, One plan is gone, and Slider is new.
        store
            .publish(design(
                Some(id.clone()),
                &[
                    ("Three tiers", "Cards", b"a"),
                    ("Compare table", "A table", b"b2"),
                    ("Slider", "Slide", b"c"),
                ],
            ))
            .expect("v2");
        let design = store.read_design(&id, 2).expect("v2's design");
        let changes: Vec<Option<VariantChange>> = design.topics[0]
            .variants
            .iter()
            .map(|variant| variant.change)
            .collect();
        assert_eq!(
            changes,
            vec![None, Some(VariantChange::Changed), Some(VariantChange::New)]
        );
        assert_eq!(design.gone, vec!["C. One plan".to_string()]);
        let picks = store.read_picks(&id, 2);
        assert_eq!(picks.topic(0).pick, None, "the gone variant took its pick");
        assert_eq!(picks.topic(0).also, vec!["Compare table".to_string()]);
        assert_eq!(
            picks
                .topic(0)
                .comments
                .get("Compare table")
                .map(String::as_str),
            Some("Yearly first.")
        );
        assert_eq!(picks.dropped, vec!["C. One plan".to_string()]);
        assert_eq!(picks.carried_from, Some(1));
        assert_eq!(picks.submitted_at, None, "it waits for the user again");
        // v1 keeps its own.
        assert!(store.read_picks(&id, 1).submitted_at.is_some());
        assert!(store.set_picks(&id, 1, 0, TopicPicks::default()).is_err());
    }

    #[test]
    fn the_agent_reads_the_design_with_its_picks_as_text() {
        let topic = DesignTopic {
            title: "The plans page".to_string(),
            description: "How the plans are laid out.".to_string(),
            today: Some(variant("Today's page", "A plain table.")),
            variants: vec![
                variant("Three tiers", "Cards."),
                DesignVariant {
                    good: "Everything at once.".to_string(),
                    cost: "Long on a phone.".to_string(),
                    page: Some("1-B.html".to_string()),
                    phone_picture: Some("1-B-phone.png".to_string()),
                    ..variant("Compare table", "Every feature in one table.")
                },
                variant("Slider by size", "One plan priced by size."),
            ],
            recommended: Some("Compare table".to_string()),
            ..DesignTopic::default()
        };
        let other = DesignTopic {
            title: "The yearly discount".to_string(),
            variants: vec![variant("10% off", ""), variant("20% off", "")],
            ..DesignTopic::default()
        };
        let design = Design {
            intro: "Two decisions for the pricing page.".to_string(),
            topics: vec![topic, other],
            gone: Vec::new(),
        };
        let picks = DesignPicks {
            topics: vec![
                TopicPicks {
                    pick: Some("Compare table".to_string()),
                    also: vec!["Slider by size".to_string()],
                    comments: [
                        (
                            "Compare table".to_string(),
                            "Yearly by default.".to_string(),
                        ),
                        (TODAY.to_string(), "Keep its footer.".to_string()),
                    ]
                    .into(),
                    ..TopicPicks::default()
                },
                TopicPicks {
                    comments: [("20% off".to_string(), "What do others give?".to_string())].into(),
                    ..TopicPicks::default()
                },
            ],
            ..DesignPicks::default()
        };
        let text = design_text(
            "Pricing",
            2,
            "storefront › Design the pricing",
            &design,
            &picks,
        );
        assert_eq!(
            text,
            "# Pricing, v2: 1 of 2 decided, not submitted\n\
             From storefront › Design the pricing. The user's picks and comments so far.\n\
             \n\
             Two decisions for the pricing page.\n\
             \n\
             ## 1. The plans page: B. Compare table, also C. Slider by size\n\
             How the plans are laid out.\n\
             \n\
             **Today's page (today's):** A plain table.\n\
             - Files: Today's page.png (picture)\n\
             - The user's comment: Keep its footer.\n\
             \n\
             **A. Three tiers:** Cards.\n\
             - Files: Three tiers.png (picture)\n\
             \n\
             **B. Compare table (picked, recommended):** Every feature in one table.\n\
             - Good: Everything at once.\n\
             - Cost: Long on a phone.\n\
             - Files: Compare table.png (picture), 1-B.html (page), 1-B-phone.png (phone picture)\n\
             - The user's comment: Yearly by default.\n\
             \n\
             **C. Slider by size (also):** One plan priced by size.\n\
             - Files: Slider by size.png (picture)\n\
             \n\
             ## 2. The yearly discount: not decided\n\
             \n\
             **A. 10% off**\n\
             - Files: 10% off.png (picture)\n\
             \n\
             **B. 20% off**\n\
             - Files: 20% off.png (picture)\n\
             - The user's comment: What do others give?\n"
        );
    }

    fn features(id: Option<ArtifactId>, names: &[&str]) -> Publish {
        let topic = DesignTopic {
            title: "Features to build".to_string(),
            multi: true,
            variants: names
                .iter()
                .map(|name| DesignVariant {
                    name: name.to_string(),
                    description: format!("{name}, in words."),
                    ..DesignVariant::default()
                })
                .collect(),
            ..DesignTopic::default()
        };
        Publish {
            id,
            title: "Features".to_string(),
            content: Content::Design {
                design: Design {
                    topics: vec![topic],
                    ..Design::default()
                },
                files: Vec::new(),
            },
            thread_id: ThreadId(1),
            project_id: ProjectId(7),
            agent_id: None,
        }
    }

    fn wanted(names: &[&str]) -> TopicPicks {
        TopicPicks {
            wanted: names.iter().map(|name| name.to_string()).collect(),
            ..TopicPicks::default()
        }
    }

    #[test]
    fn a_pick_any_topic_is_told_in_words_and_wanted_variant_by_variant() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (id, _) = store
            .publish(features(None, &["Export", "Search", "Sharing"]))
            .expect("features without pictures");
        let state = |store: &ArtifactStore| {
            store
                .get(&id)
                .and_then(|artifact| artifact.design.clone())
                .expect("design state")
        };
        assert_eq!(state(&store).status(), "Waiting for you");

        // What's wanted is kept once each, in the topic's order.
        store
            .set_picks(&id, 1, 0, wanted(&["Sharing", "Export", "Sharing"]))
            .expect("wanted");
        assert_eq!(
            store.read_picks(&id, 1).topic(0).wanted,
            ["Export".to_string(), "Sharing".to_string()]
        );
        let design = store.read_design(&id, 1).expect("design");
        assert_eq!(state(&store).decided, 1);
        assert_eq!(state(&store).pick.as_deref(), Some("A, C"));

        // Nothing wanted is a decision only when the user says so.
        store
            .set_picks(&id, 1, 0, wanted(&[]))
            .expect("none wanted");
        assert_eq!(state(&store).decided, 0);
        store
            .set_picks(
                &id,
                1,
                0,
                TopicPicks {
                    none: true,
                    ..wanted(&[])
                },
            )
            .expect("none of these");
        assert_eq!(state(&store).decided, 1);
        assert_eq!(state(&store).pick.as_deref(), Some("none of these"));
        assert_eq!(
            submit_summary(&design, &store.read_picks(&id, 1)),
            "Submitted: wanted none of these."
        );
        // Wanting something takes the None back.
        store
            .set_picks(
                &id,
                1,
                0,
                TopicPicks {
                    none: true,
                    ..wanted(&["Search"])
                },
            )
            .expect("wanted over none");
        assert!(!store.read_picks(&id, 1).topic(0).none);

        // A change to what's wanted takes back the Submit; a comment doesn't.
        store.submit(&id, 1, false).expect("submit");
        assert_eq!(state(&store).status(), "Decided");
        store
            .set_picks(
                &id,
                1,
                0,
                TopicPicks {
                    comments: [("Search".to_string(), "Just titles.".to_string())].into(),
                    ..wanted(&["Search"])
                },
            )
            .expect("comment");
        assert!(state(&store).submitted_at.is_some());
        store
            .set_picks(&id, 1, 0, wanted(&["Search", "Sharing"]))
            .expect("one more");
        assert!(state(&store).submitted_at.is_none());

        // A pick-any topic takes no pick, and only its variants.
        let pick = TopicPicks {
            pick: Some("Export".to_string()),
            ..TopicPicks::default()
        };
        assert!(store.set_picks(&id, 1, 0, pick).is_err());
        assert!(store.set_picks(&id, 1, 0, wanted(&["Nothing"])).is_err());
    }

    #[test]
    fn a_pick_one_topic_takes_no_wanted_and_a_pick_any_topic_no_today() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (id, _) = store
            .publish(design(None, &[("Three tiers", "", b"a")]))
            .expect("publish");
        assert!(
            store
                .set_picks(&id, 1, 0, wanted(&["Three tiers"]))
                .is_err()
        );
        let none = TopicPicks {
            none: true,
            ..TopicPicks::default()
        };
        assert!(store.set_picks(&id, 1, 0, none).is_err());

        let mut with_today = features(None, &["Export"]);
        if let Content::Design { design, .. } = &mut with_today.content {
            design.topics[0].today = Some(DesignVariant {
                name: "Today's".to_string(),
                ..DesignVariant::default()
            });
        }
        assert!(store.publish(with_today).is_err());
        // Words alone is for a pick-any topic only.
        let mut plain = features(None, &["Export"]);
        if let Content::Design { design, .. } = &mut plain.content {
            design.topics[0].multi = false;
        }
        assert!(store.publish(plain).is_err());
    }

    #[test]
    fn what_is_wanted_carries_over_to_the_next_version_by_name() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (id, _) = store
            .publish(features(None, &["Export", "Search", "Sharing"]))
            .expect("publish");
        store
            .set_picks(
                &id,
                1,
                0,
                TopicPicks {
                    comments: [("Search".to_string(), "Titles only.".to_string())].into(),
                    ..wanted(&["Export", "Search"])
                },
            )
            .expect("wanted");
        store.submit(&id, 1, false).expect("submit");

        // v2: Search is gone and Comments is new.
        store
            .publish(features(
                Some(id.clone()),
                &["Export", "Sharing", "Comments"],
            ))
            .expect("v2");
        let picks = store.read_picks(&id, 2);
        assert_eq!(picks.topic(0).wanted, ["Export".to_string()]);
        assert_eq!(picks.dropped, ["B. Search".to_string()]);
        assert_eq!(picks.carried_from, Some(1));
        assert_eq!(picks.submitted_at, None);
        assert!(
            picks.topic(0).comments.is_empty(),
            "Search took its comment"
        );

        // Said as a pick-one topic, it's a new question: the comments are all that stay.
        let mut single = features(Some(id.clone()), &["Export", "Sharing"]);
        if let Content::Design { design, .. } = &mut single.content {
            design.topics[0].multi = false;
            for variant in &mut design.topics[0].variants {
                variant.picture = Some(format!("{}.png", variant.name));
            }
        }
        if let Content::Design { files, .. } = &mut single.content {
            *files = vec![
                ("Export.png".to_string(), b"a".to_vec()),
                ("Sharing.png".to_string(), b"b".to_vec()),
            ];
        }
        store.publish(single).expect("v3");
        let picks = store.read_picks(&id, 3);
        assert!(picks.topic(0).wanted.is_empty());
        assert_eq!(picks.topic(0).pick, None);
    }

    #[test]
    fn the_agent_reads_what_is_wanted_in_a_pick_any_topic() {
        let design = Design {
            topics: vec![DesignTopic {
                title: "Features to build".to_string(),
                multi: true,
                variants: vec![
                    DesignVariant {
                        name: "Export".to_string(),
                        description: "As CSV.".to_string(),
                        ..DesignVariant::default()
                    },
                    DesignVariant {
                        name: "Search".to_string(),
                        ..DesignVariant::default()
                    },
                    DesignVariant {
                        name: "Sharing".to_string(),
                        ..DesignVariant::default()
                    },
                ],
                recommended: Some("Search".to_string()),
                ..DesignTopic::default()
            }],
            ..Design::default()
        };
        let picks = DesignPicks {
            topics: vec![wanted(&["Export", "Search"])],
            ..DesignPicks::default()
        };
        assert_eq!(
            design_text("Features", 1, "storefront", &design, &picks),
            "# Features, v1: 1 of 1 decided, not submitted\n\
             From storefront. The user's picks and comments so far.\n\
             \n\
             ## 1. Features to build (pick any): A. Export, B. Search\n\
             \n\
             **A. Export (wanted):** As CSV.\n\
             \n\
             **B. Search (wanted, recommended)**\n\
             \n\
             **C. Sharing**\n"
        );
        assert_eq!(submit_summary(&design, &picks), "Submitted: wanted A, B.");
        assert_eq!(
            submit_summary(&design, &DesignPicks::default()),
            "Submitted with nothing marked."
        );
    }

    #[test]
    fn search_finds_words_inside() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (notes, _) = store
            .publish(document(
                "Notes",
                "# Notes\n\nShow the **yearly** price first.",
            ))
            .expect("publish");
        let (plans, _) = store
            .publish(design(
                None,
                &[("Compare table", "Monthly and Yearly", b"b")],
            ))
            .expect("design");
        let found = store.search("yearly");
        assert!(found.contains(&(notes, "Show the yearly price first.".to_string())));
        assert!(found.contains(&(plans, "A. Compare table: Monthly and Yearly".to_string())));
        assert!(store.search("nothing like it").is_empty());
    }
}
