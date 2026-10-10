//! The machine's artifacts (design/artifacts): each in `artifacts/<id>/` in the data folder,
//! with a `meta.json` ([`Artifact`]), one `pages/<version>.html|md` per publish, and the files
//! the agent attached in `files/<version>/`. Deleting the thread that published one keeps it;
//! deleting the artifact removes the folder.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use agentz_protocol::agents::AgentId;
use agentz_protocol::artifacts::{
    Artifact, ArtifactFile, ArtifactId, ArtifactKind, ArtifactVersion, MAX_ARTIFACT_SIZE,
};
use anyhow::{Context as _, Result, anyhow, bail, ensure};
use projects::{ProjectId, ThreadId};
use util::ResultExt as _;

const MAX_TITLE_CHARS: usize = 256;
const MAX_FILE_NAME_CHARS: usize = 256;

/// What a publish brings.
pub(crate) struct Publish {
    /// The artifact to add a version to; `None` starts one.
    pub id: Option<ArtifactId>,
    pub title: String,
    pub kind: ArtifactKind,
    pub source: String,
    /// Files to offer as downloads: `(name, bytes)`.
    pub files: Vec<(String, Vec<u8>)>,
    pub thread_id: ThreadId,
    pub project_id: ProjectId,
    pub agent_id: Option<AgentId>,
}

pub(crate) struct ArtifactStore {
    folder: PathBuf,
    artifacts: Vec<Artifact>,
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
                    // A half-written or older meta doesn't keep the rest from loading.
                    Err(error) => {
                        log::warn!("skipping {}: {error:#}", meta.display());
                    }
                }
            }
        }
        let mut store = Self {
            folder,
            artifacts,
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

    /// Adds a version, or starts the artifact. Returns it with its new version's number.
    pub fn publish(&mut self, publish: Publish) -> Result<(ArtifactId, u32)> {
        let title = publish.title.trim();
        ensure!(!title.is_empty(), "a title is required");
        ensure!(
            title.chars().count() <= MAX_TITLE_CHARS,
            "the title is over {MAX_TITLE_CHARS} characters"
        );
        ensure!(
            publish.source.len() as u64 <= MAX_ARTIFACT_SIZE,
            "the page is over 16 MiB"
        );
        let now = millis(SystemTime::now());
        let mut files = Vec::with_capacity(publish.files.len());
        for (name, bytes) in &publish.files {
            let name = file_name(name)?;
            ensure!(
                bytes.len() as u64 <= MAX_ARTIFACT_SIZE,
                "{name} is over 16 MiB"
            );
            files.push((
                name,
                ArtifactFile {
                    name: name.to_string(),
                    bytes: bytes.len() as u64,
                },
            ));
        }
        let position = match &publish.id {
            Some(id) => self
                .artifacts
                .iter()
                .position(|artifact| &artifact.id == id)
                .ok_or_else(|| anyhow!("there's no artifact {id:?} to update"))?,
            None => {
                let id = self.new_id();
                self.artifacts.push(Artifact {
                    id,
                    title: String::new(),
                    kind: publish.kind,
                    versions: Vec::new(),
                    thread_id: None,
                    project_id: None,
                    agent_id: None,
                    sent_at: None,
                    frame_key: key(),
                });
                self.artifacts.len() - 1
            }
        };
        let artifact = &mut self.artifacts[position];
        artifact.title = title.to_string();
        artifact.kind = publish.kind;
        artifact.thread_id = Some(publish.thread_id);
        artifact.project_id = Some(publish.project_id);
        artifact.agent_id = publish.agent_id;
        artifact.versions.push(ArtifactVersion {
            published_at: now,
            bytes: publish.source.len() as u64,
            files: files.iter().map(|(_, file)| file.clone()).collect(),
        });
        let version = artifact.latest();
        let folder = self.folder.join(&artifact.id.0);
        write(
            &folder.join("pages"),
            &page_name(version, artifact.kind),
            publish.source.as_bytes(),
        )?;
        for ((name, _), (_, bytes)) in files.iter().zip(publish.files.iter()) {
            write(&folder.join("files").join(version.to_string()), name, bytes)?;
        }
        let id = artifact.id.clone();
        self.save_at(position)?;
        self.sort();
        self.revision += 1;
        Ok((id, version))
    }

    /// A version's source.
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

    /// A file attached to a version.
    pub fn read_file(&self, id: &ArtifactId, version: u32, name: &str) -> Result<Vec<u8>> {
        let name = file_name(name)?;
        let artifact = self.get(id).ok_or_else(|| anyhow!("no such artifact"))?;
        let known = artifact
            .version(version)
            .ok_or_else(|| anyhow!("the artifact has no version {version}"))?
            .files
            .iter()
            .any(|file| file.name == name);
        ensure!(known, "the version has no file {name}");
        let path = self
            .folder
            .join(&id.0)
            .join("files")
            .join(version.to_string())
            .join(name);
        std::fs::read(&path).with_context(|| format!("reading {}", path.display()))
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

    /// The page sent something back to its thread.
    pub fn note_sent(&mut self, id: &ArtifactId) {
        let Some(position) = self
            .artifacts
            .iter()
            .position(|artifact| &artifact.id == id)
        else {
            return;
        };
        self.artifacts[position].sent_at = Some(millis(SystemTime::now()));
        self.save_at(position).log_err();
        self.revision += 1;
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

/// A download's name: no path, no tricks.
fn file_name(name: &str) -> Result<&str> {
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

    fn publish(title: &str, source: &str) -> Publish {
        Publish {
            id: None,
            title: title.to_string(),
            kind: ArtifactKind::Page,
            source: source.to_string(),
            files: Vec::new(),
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
            .publish(publish("Layouts", "<p>one</p>"))
            .expect("publish");
        assert_eq!(version, 1);
        let (again, version) = store
            .publish(Publish {
                id: Some(id.clone()),
                source: "<p>two</p>".to_string(),
                ..publish("Layouts", "")
            })
            .expect("republish");
        assert_eq!((again, version), (id.clone(), 2));
        assert_eq!(store.read_source(&id, 1).expect("v1"), "<p>one</p>");
        assert_eq!(store.read_source(&id, 2).expect("v2"), "<p>two</p>");
        assert_eq!(store.get(&id).expect("artifact").latest(), 2);

        // A restart loads it.
        let store = ArtifactStore::load(temp.path());
        let artifact = store.get(&id).expect("artifact after reload");
        assert_eq!(artifact.title, "Layouts");
        assert_eq!(artifact.thread_id, Some(ThreadId(1)));
        assert_eq!(
            store.read_source(&id, 2).expect("v2 after reload"),
            "<p>two</p>"
        );
    }

    #[test]
    fn files_come_back_and_names_are_safe() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (id, _) = store
            .publish(Publish {
                files: vec![("../report.csv".to_string(), b"a,b".to_vec())],
                ..publish("Report", "<p>r</p>")
            })
            .expect("publish");
        assert_eq!(store.read_file(&id, 1, "report.csv").expect("file"), b"a,b");
        assert!(store.read_file(&id, 1, "../meta.json").is_err());
    }

    #[test]
    fn deleting_removes_the_folder() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut store = ArtifactStore::load(temp.path());
        let (id, _) = store
            .publish(publish("Layouts", "<p>one</p>"))
            .expect("publish");
        store.delete(&id).expect("delete");
        assert!(store.get(&id).is_none());
        assert!(!temp.path().join("artifacts").join(&id.0).exists());
    }
}
