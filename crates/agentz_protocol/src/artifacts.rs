//! Artifacts (design/artifacts): pages agents publish from a thread or chat, one `.html` or
//! `.md` file each, with a version for each publish. Each machine's server keeps its own in its
//! data folder. The app's own machine serves every machine's pages to the user's browser on
//! `127.0.0.1`: another machine's reach it through the app ([`ArtifactRequest`]).

use projects::{ProjectId, ThreadId};
use serde::{Deserialize, Serialize};

use crate::agents::AgentId;

/// The most a page, or a file attached to it, may hold (Claude Code's limit).
pub const MAX_ARTIFACT_SIZE: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArtifactId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// An `.html` file, shown as it is.
    Page,
    /// A `.md` file, shown as a document page.
    Document,
}

impl ArtifactKind {
    pub fn label(self) -> &'static str {
        match self {
            ArtifactKind::Page => "Page",
            ArtifactKind::Document => "Document",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ArtifactKind::Page => "html",
            ArtifactKind::Document => "md",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    pub title: String,
    pub kind: ArtifactKind,
    /// Oldest first: version `n` is `versions[n - 1]`.
    pub versions: Vec<ArtifactVersion>,
    /// The thread or chat that published the latest version. Deleting it keeps the artifact.
    pub thread_id: Option<ThreadId>,
    /// Its project when it was published; [`ProjectId::CHATS`] for a chat's.
    #[serde(default)]
    pub project_id: Option<ProjectId>,
    #[serde(default)]
    pub agent_id: Option<AgentId>,
    /// When the page last sent something back to its thread, in milliseconds since the epoch.
    #[serde(default)]
    pub sent_at: Option<u64>,
    /// What the page's own frames are served with, rather than the server's token, so a page's
    /// scripts can't use the token.
    pub frame_key: String,
}

impl Artifact {
    pub fn latest(&self) -> u32 {
        self.versions.len() as u32
    }

    pub fn version(&self, number: u32) -> Option<&ArtifactVersion> {
        let index = usize::try_from(number).ok()?.checked_sub(1)?;
        self.versions.get(index)
    }

    /// When the latest version was published.
    pub fn published_at(&self) -> u64 {
        self.versions
            .last()
            .map_or(0, |version| version.published_at)
    }

    pub fn is_from_chat(&self) -> bool {
        self.project_id == Some(ProjectId::CHATS)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactVersion {
    /// In milliseconds since the epoch.
    pub published_at: u64,
    pub bytes: u64,
    /// The files the agent attached, offered as downloads from the page.
    #[serde(default)]
    pub files: Vec<ArtifactFile>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactFile {
    pub name: String,
    pub bytes: u64,
}

/// A machine's artifacts, newest publish first, and where its server serves pages.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Artifacts {
    pub artifacts: Vec<Artifact>,
    pub pages: Option<ArtifactPages>,
}

/// Where a server serves pages: links are `http://<address>/…?t=<token>`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactPages {
    pub address: String,
    pub token: String,
}

impl ArtifactPages {
    /// The page of one of this machine's artifacts, or with `machine`, of the machine of that
    /// name in the app; at a version, or the latest.
    pub fn link(&self, machine: Option<&str>, id: &ArtifactId, version: Option<u32>) -> String {
        let mut link = format!("http://{}", self.address);
        if let Some(machine) = machine {
            link.push_str("/m/");
            link.push_str(
                &url::form_urlencoded::byte_serialize(machine.as_bytes()).collect::<String>(),
            );
        }
        link.push_str(&format!("/a/{}?t={}", id.0, self.token));
        if let Some(version) = version {
            link.push_str(&format!("&v={version}"));
        }
        link
    }

    /// The All artifacts page.
    pub fn gallery_link(&self) -> String {
        format!("http://{}/?t={}", self.address, self.token)
    }
}

/// The app's theme, for pages: CSS variables (`--background`, …) and whether it's dark.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PageTheme {
    pub dark: bool,
    pub variables: Vec<(String, String)>,
}

/// What the server that serves pages asks of another machine's, through the app
/// ([`crate::Request::Artifact`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ArtifactRequest {
    /// Every artifact, for All artifacts: [`ArtifactReply::List`].
    List,
    /// One artifact, for its page's header: [`ArtifactReply::Listing`].
    Listing(ArtifactId),
    /// One version's source, the latest without one: [`ArtifactReply::Page`].
    Read {
        id: ArtifactId,
        version: Option<u32>,
    },
    /// A file attached to a version, in base64: [`ArtifactReply::File`].
    File {
        id: ArtifactId,
        version: u32,
        name: String,
    },
    /// Send to thread: puts what the page gave and the user's note in its thread as the user's
    /// message, queued while the agent works. [`ArtifactReply::Sent`].
    Send {
        id: ArtifactId,
        version: u32,
        text: String,
        note: String,
    },
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ArtifactReply {
    List(Vec<ArtifactListing>),
    Listing(ArtifactListing),
    Page {
        listing: ArtifactListing,
        version: u32,
        source: String,
    },
    File(String),
    Sent,
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

/// An artifact with what its server knows of where it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactListing {
    pub artifact: Artifact,
    /// "storefront › Add the checkout page", "Chat · Async runtimes", or "from a deleted
    /// thread".
    pub place: String,
    /// Whether the thread that published it is still there.
    pub has_thread: bool,
    pub agent_name: Option<String>,
    /// The agent's icon, as SVG drawn in the text color.
    pub agent_icon: Option<String>,
}
