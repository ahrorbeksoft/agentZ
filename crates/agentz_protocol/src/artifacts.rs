//! Artifacts (design/artifacts, design/artifacts-2): what agents publish from a thread or chat,
//! with a version for each publish. A document is one Markdown file; a design is topics of
//! variants (pictures and live pages) that agentZ draws for the user to pick from, with the
//! picks kept beside it. Each machine's server keeps its own in its data folder. The app's own
//! machine serves every machine's pages to the user's browser on `127.0.0.1`: another
//! machine's reach it through the app ([`ArtifactRequest`]).

use std::collections::BTreeMap;

use projects::{ProjectId, ThreadId};
use serde::{Deserialize, Serialize};

use crate::agents::AgentId;

/// The most a document, or a file attached to an artifact, may hold (Claude Code's limit).
pub const MAX_ARTIFACT_SIZE: u64 = 16 * 1024 * 1024;

/// A topic's key for its today's version, in picks and comments; a variant can't be named so.
pub const TODAY: &str = "today";

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ArtifactId(pub String);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// A Markdown file, shown as a document page.
    Document,
    /// Topics of variants to pick from ([`Design`]), drawn by agentZ.
    Design,
}

impl ArtifactKind {
    pub fn label(self) -> &'static str {
        match self {
            ArtifactKind::Document => "Document",
            ArtifactKind::Design => "Design",
        }
    }

    /// How a version's source is kept: the Markdown, or the design as JSON.
    pub fn extension(self) -> &'static str {
        match self {
            ArtifactKind::Document => "md",
            ArtifactKind::Design => "json",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            ArtifactKind::Document => "document",
            ArtifactKind::Design => "design",
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
    /// A document's first lines, or a design's intro, for cards.
    #[serde(default)]
    pub preview: String,
    /// Where a design stands, from its latest version.
    #[serde(default)]
    pub design: Option<DesignState>,
    /// What the artifact's own content (a document, a variant's picture or page) is served
    /// with, rather than the server's token, so an agent's scripts can't use the token.
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

    /// What it is, as cards say it: "Document · v2", "Design · 5 variants · v1".
    pub fn description(&self) -> String {
        match &self.design {
            Some(design) => format!(
                "{} · {} · v{}",
                self.kind.label(),
                design.size(),
                self.latest()
            ),
            None => format!("{} · v{}", self.kind.label(), self.latest()),
        }
    }

    /// A design not submitted yet (the artifacts page's Waiting for you).
    pub fn is_waiting(&self) -> bool {
        self.design
            .as_ref()
            .is_some_and(|design| design.submitted_at.is_none())
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

/// A design's latest version as lists show it (design/artifacts-2, topics 2 and 18).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignState {
    pub topics: u32,
    /// The first topic's variants, not counting today's.
    pub variants: u32,
    /// Topics with a pick.
    pub decided: u32,
    /// When the latest version was submitted; changing a pick clears it.
    pub submitted_at: Option<u64>,
    /// A one-topic design's pick, short ("B"), for "Decided · B".
    #[serde(default)]
    pub pick: Option<String>,
    /// The first topic's pictures, by their file names in the version, for cards.
    #[serde(default)]
    pub pictures: Vec<String>,
}

impl DesignState {
    /// "Decided", "2 of 3 decided" or "Waiting for you".
    pub fn status(&self) -> String {
        if self.submitted_at.is_some() {
            "Decided".to_string()
        } else if self.decided > 0 {
            format!("{} of {} decided", self.decided, self.topics)
        } else {
            "Waiting for you".to_string()
        }
    }

    /// "5 variants", or with several topics "3 topics".
    pub fn size(&self) -> String {
        if self.topics > 1 {
            format!("{} topics", self.topics)
        } else if self.variants == 1 {
            "1 variant".to_string()
        } else {
            format!("{} variants", self.variants)
        }
    }
}

/// A design as published (`agentz_design_publish`): an intro and its topics. Pictures and
/// pages are the version's files, by name.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Design {
    #[serde(default)]
    pub intro: String,
    pub topics: Vec<DesignTopic>,
    /// The variants the version before had that this one hasn't: "D. One plan", with the
    /// topic before it when there are several.
    #[serde(default)]
    pub gone: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignTopic {
    pub title: String,
    /// What's being decided, under the title.
    #[serde(default)]
    pub description: String,
    /// What's there now, as the first card (topic 11 B).
    #[serde(default)]
    pub today: Option<DesignVariant>,
    pub variants: Vec<DesignVariant>,
    /// The variant the agent recommends, by name.
    #[serde(default)]
    pub recommended: Option<String>,
}

impl DesignTopic {
    /// A variant's letter by its place: A for the first.
    pub fn letter(index: usize) -> String {
        let letters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        match letters.get(index) {
            Some(letter) => (*letter as char).to_string(),
            None => (index + 1).to_string(),
        }
    }

    /// The variant a pick or comment names: [`TODAY`] or a variant's name.
    pub fn variant(&self, key: &str) -> Option<&DesignVariant> {
        if key == TODAY {
            return self.today.as_ref();
        }
        self.variants.iter().find(|variant| variant.name == key)
    }

    /// How a pick is said: "B. Compare table", or "Today's page (today's)".
    pub fn label(&self, key: &str) -> String {
        if key == TODAY {
            return match &self.today {
                Some(today) => format!("{} (today's)", today.name),
                None => "Today's".to_string(),
            };
        }
        match self.variants.iter().position(|variant| variant.name == key) {
            Some(index) => format!("{}. {key}", Self::letter(index)),
            None => key.to_string(),
        }
    }

    /// The short form, for one-line summaries: "B", or "today's".
    pub fn short_label(&self, key: &str) -> String {
        if key == TODAY {
            return "today's".to_string();
        }
        match self.variants.iter().position(|variant| variant.name == key) {
            Some(index) => Self::letter(index),
            None => key.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignVariant {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub good: String,
    #[serde(default)]
    pub cost: String,
    /// A picture of it, by its file name in the version.
    #[serde(default)]
    pub picture: Option<String>,
    /// A live page of it, an HTML file, by its file name in the version.
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub phone_picture: Option<String>,
    #[serde(default)]
    pub phone_page: Option<String>,
    /// What it is against the version before (topic 19 A).
    #[serde(default)]
    pub change: Option<VariantChange>,
}

impl DesignVariant {
    pub fn has_phone(&self) -> bool {
        self.phone_picture.is_some() || self.phone_page.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantChange {
    Changed,
    New,
}

/// The user's picks and comments on one version of a design, kept by its server as they go
/// (topic 15 A).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignPicks {
    /// A topic's picks by its place in the design.
    #[serde(default)]
    pub topics: Vec<TopicPicks>,
    /// When it was submitted (topic 16 A); changing a pick clears it.
    #[serde(default)]
    pub submitted_at: Option<u64>,
    /// Whether that Submit also sent it to the thread.
    #[serde(default)]
    pub sent: bool,
    /// The version the picks came over from, when they did (topic 19 A).
    #[serde(default)]
    pub carried_from: Option<u32>,
    /// Picks that didn't come over because their variant is gone: "D. One plan".
    #[serde(default)]
    pub dropped: Vec<String>,
}

impl DesignPicks {
    pub fn topic(&self, index: usize) -> TopicPicks {
        self.topics.get(index).cloned().unwrap_or_default()
    }

    pub fn decided(&self) -> u32 {
        self.topics
            .iter()
            .filter(|topic| topic.pick.is_some())
            .count() as u32
    }

    pub fn comments(&self) -> usize {
        self.topics
            .iter()
            .map(|topic| {
                topic
                    .comments
                    .values()
                    .filter(|comment| !comment.trim().is_empty())
                    .count()
            })
            .sum()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TopicPicks {
    /// The variant picked, by name, or [`TODAY`] to keep today's.
    #[serde(default)]
    pub pick: Option<String>,
    /// Others to take parts of (+ Also).
    #[serde(default)]
    pub also: Vec<String>,
    /// A comment per variant, by name or [`TODAY`].
    #[serde(default)]
    pub comments: BTreeMap<String, String>,
}

/// What a design should be like when the agent doesn't hear otherwise (topic 21): given to
/// the agent at the end of `agentz_design_publish`'s description.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignDefaults {
    pub variants: u8,
    pub include_today: bool,
    pub sizes: DesignSizes,
    pub variants_as: VariantsAs,
}

impl Default for DesignDefaults {
    fn default() -> Self {
        Self {
            variants: 3,
            include_today: true,
            sizes: DesignSizes::Desktop,
            variants_as: VariantsAs::Pictures,
        }
    }
}

impl DesignDefaults {
    pub const MIN_VARIANTS: u8 = 1;
    pub const MAX_VARIANTS: u8 = 9;

    /// The defaults in a line, as Settings says them: "3 variants, today's included, desktop,
    /// pictures."
    pub fn summary(&self) -> String {
        format!(
            "{} variant{}, today's {}, {}, {}.",
            self.variants,
            if self.variants == 1 { "" } else { "s" },
            if self.include_today {
                "included"
            } else {
                "left out"
            },
            match self.sizes {
                DesignSizes::Desktop => "desktop",
                DesignSizes::Phone => "phone",
                DesignSizes::Both => "desktop and phone",
            },
            match self.variants_as {
                VariantsAs::Pictures => "pictures",
                VariantsAs::LivePages => "live pages",
                VariantsAs::Both => "pictures and live pages",
            },
        )
    }

    /// The end of the design tool's description (topic 23).
    pub fn instructions(&self) -> String {
        let today = if self.include_today {
            ", with today's version as one to keep"
        } else {
            ", without today's version"
        };
        let sizes = match self.sizes {
            DesignSizes::Desktop => "Desktop only.",
            DesignSizes::Phone => "Phone only: give each variant as phonePicture or phonePage.",
            DesignSizes::Both => "Desktop and phone: give each variant a phone version too.",
        };
        let variants_as = match self.variants_as {
            VariantsAs::Pictures => "Pictures for the variants.",
            VariantsAs::LivePages => "Live pages for the variants.",
            VariantsAs::Both => "A picture and a live page for each variant.",
        };
        format!(
            "The user's design defaults, for when they haven't said otherwise: {} variant{} a \
             topic{today}. {sizes} {variants_as} agentZ checks every design against them and \
             sends back one that differs, unless it has a deviationReason: one sentence on why \
             this design needs to, which the user sees. Follow the defaults when the user hasn't \
             asked for something else.",
            self.variants,
            if self.variants == 1 { "" } else { "s" },
        )
    }

    /// How a design differs from these defaults, a thing each ("topic 2 has 5 variants, not
    /// 3"); empty when it follows them. Only what the defaults decide: how many variants a
    /// topic has, whether today's is there, which sizes and whether pictures, live pages or
    /// both.
    pub fn deviations(&self, design: &Design) -> Vec<String> {
        let mut found = Vec::new();
        let place = |index: usize| {
            if design.topics.len() > 1 {
                format!("topic {}", index + 1)
            } else {
                "the topic".to_string()
            }
        };
        let plural = |count: usize| if count == 1 { "" } else { "s" };
        for (index, topic) in design.topics.iter().enumerate() {
            let count = topic.variants.len();
            if count != usize::from(self.variants) {
                found.push(format!(
                    "{} has {count} variant{}, not {}",
                    place(index),
                    plural(count),
                    self.variants
                ));
            }
            match (self.include_today, topic.today.is_some()) {
                (true, false) => found.push(format!("{} has no today's version", place(index))),
                (false, true) => found.push(format!("{} has today's version", place(index))),
                _ => {}
            }
        }
        let variants: Vec<&DesignVariant> = design
            .topics
            .iter()
            .flat_map(|topic| &topic.variants)
            .collect();
        let wants_desktop = self.sizes != DesignSizes::Phone;
        let wants_phone = self.sizes != DesignSizes::Desktop;
        let wants_picture = self.variants_as != VariantsAs::LivePages;
        let wants_page = self.variants_as != VariantsAs::Pictures;
        let mut differs = |wanted: bool, has: fn(&DesignVariant) -> bool, what: &str| {
            let count = variants
                .iter()
                .filter(|variant| has(variant) != wanted)
                .count();
            if count > 0 {
                let verb = match (wanted, count) {
                    (true, 1) => "lacks",
                    (true, _) => "lack",
                    (false, 1) => "has",
                    (false, _) => "have",
                };
                found.push(format!("{count} variant{} {verb} {what}", plural(count)));
            }
        };
        differs(
            wants_desktop,
            |variant| variant.picture.is_some() || variant.page.is_some(),
            "a desktop version",
        );
        differs(wants_phone, DesignVariant::has_phone, "a phone version");
        differs(
            wants_picture,
            |variant| variant.picture.is_some() || variant.phone_picture.is_some(),
            "a picture",
        );
        differs(
            wants_page,
            |variant| variant.page.is_some() || variant.phone_page.is_some(),
            "a live page",
        );
        found
    }
}

/// The design defaults the app sends each server: General's, and those of the projects there
/// that have their own.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DesignSettings {
    pub general: DesignDefaults,
    #[serde(default)]
    pub projects: Vec<(ProjectId, DesignDefaults)>,
}

impl DesignSettings {
    /// A project's: its own, or General's.
    pub fn for_project(&self, project_id: Option<ProjectId>) -> DesignDefaults {
        project_id
            .and_then(|project_id| {
                self.projects
                    .iter()
                    .find(|(id, _)| *id == project_id)
                    .map(|(_, defaults)| *defaults)
            })
            .unwrap_or(self.general)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignSizes {
    Desktop,
    Phone,
    Both,
}

impl DesignSizes {
    pub const ALL: [Self; 3] = [Self::Desktop, Self::Phone, Self::Both];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariantsAs {
    Pictures,
    LivePages,
    Both,
}

impl VariantsAs {
    pub const ALL: [Self; 3] = [Self::Pictures, Self::LivePages, Self::Both];
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
            link.push_str(&encode(machine));
        }
        link.push_str(&format!("/a/{}?t={}", id.0, self.token));
        if let Some(version) = version {
            link.push_str(&format!("&v={version}"));
        }
        link
    }

    /// The artifacts page, at a place: a project by its name, or `None` for All artifacts.
    pub fn home_link(&self, project: Option<&str>) -> String {
        let mut link = format!("http://{}/?t={}", self.address, self.token);
        if let Some(project) = project {
            link.push_str("&p=");
            link.push_str(&encode(project));
        }
        link
    }
}

fn encode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// The app's theme, for pages: CSS variables (`--background`, …) and whether it's dark, and
/// the settings the pages follow.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PageTheme {
    pub dark: bool,
    pub variables: Vec<(String, String)>,
    /// The picture behind the app's window, for agentZ's own pages to show behind them too:
    /// its copy's file name in the data directory's `backgrounds`.
    #[serde(default)]
    pub picture: Option<String>,
    /// Chats are on in the app's settings, so the artifacts page lists them as a place.
    #[serde(default = "default_true")]
    pub chats: bool,
}

impl Default for PageTheme {
    fn default() -> Self {
        Self {
            dark: false,
            variables: Vec::new(),
            picture: None,
            chats: true,
        }
    }
}

fn default_true() -> bool {
    true
}

/// What the server that serves pages, or an agent's tool, asks of another machine's, through
/// the app ([`crate::Request::Artifact`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ArtifactRequest {
    /// Every artifact: [`ArtifactReply::List`].
    List,
    /// One artifact, for its page's bar: [`ArtifactReply::Listing`].
    Listing(ArtifactId),
    /// One version's source (a document's Markdown, a design's JSON), the latest without one:
    /// [`ArtifactReply::Page`].
    Read {
        id: ArtifactId,
        version: Option<u32>,
    },
    /// A file of a version (a download, or a design's picture or page), in base64:
    /// [`ArtifactReply::File`].
    File {
        id: ArtifactId,
        version: u32,
        name: String,
    },
    /// A design's version with its picks: [`ArtifactReply::Design`].
    Design {
        id: ArtifactId,
        version: Option<u32>,
    },
    /// Saves a topic's picks and comments: [`ArtifactReply::Saved`].
    SetPicks {
        id: ArtifactId,
        version: u32,
        topic: u32,
        picks: TopicPicks,
    },
    /// Marks a design's version decided, and with `send`, tells its thread with the user's
    /// note: [`ArtifactReply::Saved`].
    Submit {
        id: ArtifactId,
        version: u32,
        send: bool,
        note: String,
    },
    /// What an agent reads (`agentz_artifact_read`): a document's Markdown, or a design's picks
    /// and comments as text. [`ArtifactReply::Text`].
    Text {
        id: ArtifactId,
        version: Option<u32>,
    },
    /// The artifacts whose words hold all of these, with the line that does:
    /// [`ArtifactReply::Found`].
    Search(String),
    /// A project's icon file, the one picked or the favicon found
    /// ([`ListingIcon::file`]): [`ArtifactReply::ProjectIcon`].
    ProjectIcon(ProjectId),
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
    Design {
        listing: ArtifactListing,
        version: u32,
        design: Design,
        picks: DesignPicks,
    },
    Saved,
    Text {
        listing: ArtifactListing,
        version: u32,
        text: String,
    },
    /// Each artifact found, with its matching line.
    Found(Vec<(ArtifactId, String)>),
    /// The file's name, for its type, and its bytes in base64.
    ProjectIcon {
        name: String,
        data: String,
    },
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
    /// The project it came from, by name; `None` for a chat's or a deleted thread's.
    #[serde(default)]
    pub project: Option<String>,
    /// Whether a chat published it.
    #[serde(default)]
    pub chat: bool,
    /// Whether the thread that published it is still there.
    pub has_thread: bool,
    /// The thread's title, when it's there.
    #[serde(default)]
    pub thread_title: Option<String>,
    /// Whether that thread is archived.
    #[serde(default)]
    pub archived: bool,
    pub agent_name: Option<String>,
    /// The agent's icon, as SVG drawn in the text color.
    pub agent_icon: Option<String>,
    /// The project's icon, as the app's sidebar shows it.
    #[serde(default)]
    pub project_icon: Option<ListingIcon>,
}

/// A project's icon for the artifacts page: its emoji, else its icon file, else letters on a
/// tile, which also show while the file loads and if it can't. An icon of the app's own shows
/// as the letters, in its color.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ListingIcon {
    pub emoji: Option<String>,
    /// It has an icon file, from [`ArtifactRequest::ProjectIcon`].
    pub file: bool,
    pub text: String,
    /// The tile's color as `#rrggbb`, in light themes' shade and in dark ones'.
    pub light: String,
    pub dark: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variant(name: &str, picture: bool, page: bool, phone: bool) -> DesignVariant {
        DesignVariant {
            name: name.to_string(),
            picture: picture.then(|| "a.png".to_string()),
            page: page.then(|| "a.html".to_string()),
            phone_picture: (phone && picture).then(|| "a-phone.png".to_string()),
            phone_page: (phone && page).then(|| "a-phone.html".to_string()),
            ..Default::default()
        }
    }

    fn topic(variants: usize, today: bool, picture: bool, page: bool, phone: bool) -> DesignTopic {
        DesignTopic {
            title: "The plans page".to_string(),
            today: today.then(|| variant("Today's", true, false, false)),
            variants: (0..variants)
                .map(|place| variant(&DesignTopic::letter(place), picture, page, phone))
                .collect(),
            ..Default::default()
        }
    }

    fn design(topics: Vec<DesignTopic>) -> Design {
        Design {
            topics,
            ..Default::default()
        }
    }

    #[test]
    fn a_design_as_the_defaults_say_has_no_deviations() {
        let defaults = DesignDefaults::default();
        assert_eq!(
            defaults.deviations(&design(vec![topic(3, true, true, false, false)])),
            Vec::<String>::new()
        );

        let both = DesignDefaults {
            variants: 2,
            include_today: false,
            sizes: DesignSizes::Both,
            variants_as: VariantsAs::Both,
        };
        assert!(
            both.deviations(&design(vec![topic(2, false, true, true, true)]))
                .is_empty()
        );
        let phone = DesignDefaults {
            sizes: DesignSizes::Phone,
            ..defaults
        };
        let mut phone_only = topic(3, true, true, false, false);
        for variant in &mut phone_only.variants {
            variant.phone_picture = variant.picture.take();
        }
        assert!(phone.deviations(&design(vec![phone_only])).is_empty());
    }

    #[test]
    fn a_design_says_how_it_differs_from_the_defaults() {
        let defaults = DesignDefaults::default();
        assert_eq!(
            defaults.deviations(&design(vec![topic(1, false, true, false, false)])),
            [
                "the topic has 1 variant, not 3",
                "the topic has no today's version"
            ]
        );
        assert_eq!(
            defaults.deviations(&design(vec![
                topic(3, true, true, false, false),
                topic(5, true, true, true, true),
            ])),
            [
                "topic 2 has 5 variants, not 3",
                "5 variants have a phone version",
                "5 variants have a live page",
            ]
        );
        let live = DesignDefaults {
            variants_as: VariantsAs::LivePages,
            include_today: false,
            ..defaults
        };
        assert_eq!(
            live.deviations(&design(vec![topic(3, true, true, false, false)])),
            [
                "the topic has today's version",
                "3 variants have a picture",
                "3 variants lack a live page",
            ]
        );
    }
}
