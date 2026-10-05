//! Images attached to a thread's messages, kept by the thread's server as t3code keeps
//! attachments: each named by a hash of its bytes, so the same image pasted twice is kept once,
//! and clients fetch one by its name ([`crate::Request::Attachment`]). Messages link to them as
//! `[@Image](agentz://attachment/<id>)`.

use serde::{Deserialize, Serialize};

/// The largest image or file a client sends, so its base64 stays under
/// [`crate::MAX_FRAME_SIZE`].
pub const MAX_ATTACHMENT_SIZE: usize = 32 * 1024 * 1024;

const URI_PREFIX: &str = "agentz://attachment/";

/// The image types agentZ keeps, by the extension it names them with.
const IMAGE_TYPES: [(&str, &str); 7] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("tiff", "image/tiff"),
    ("svg", "image/svg+xml"),
];

/// An attached image's name: the hex SHA-256 of its bytes, and its type's extension. Clients
/// send these, so only well-formed ones parse: one can't name a file outside the store.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AttachmentId(String);

impl AttachmentId {
    /// The id of an image of `mime_type` whose bytes hash to `hash`, for a type agentZ keeps.
    pub fn new(hash: &str, mime_type: &str) -> Option<Self> {
        let (extension, _) = IMAGE_TYPES.iter().find(|(_, known)| *known == mime_type)?;
        Self::parse(&format!("{hash}.{extension}"))
    }

    /// The id of these bytes, an image of `mime_type`. Clients name an image before the server
    /// has it, so its chip doesn't wait for the upload.
    pub fn for_image(bytes: &[u8], mime_type: &str) -> Option<Self> {
        Self::new(&content_hash(bytes), mime_type)
    }

    pub fn parse(id: &str) -> Option<Self> {
        let (hash, extension) = id.split_once('.')?;
        let is_hash = hash.len() == 64
            && hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
        let is_known = IMAGE_TYPES.iter().any(|(known, _)| *known == extension);
        (is_hash && is_known).then(|| Self(id.to_string()))
    }

    /// The id a message's link names, `agentz://attachment/<id>`.
    pub fn from_uri(uri: &str) -> Option<Self> {
        Self::parse(uri.strip_prefix(URI_PREFIX)?)
    }

    pub fn uri(&self) -> String {
        format!("{URI_PREFIX}{}", self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn mime_type(&self) -> &'static str {
        let extension = self.0.rsplit('.').next().unwrap_or_default();
        IMAGE_TYPES
            .iter()
            .find(|(known, _)| *known == extension)
            .map_or("application/octet-stream", |(_, mime_type)| mime_type)
    }

    /// How a message's markdown shows it: a mention, as Zed writes one.
    pub fn markdown_link(&self) -> String {
        format!("[@Image]({})", self.uri())
    }
}

impl TryFrom<String> for AttachmentId {
    type Error = String;

    fn try_from(id: String) -> Result<Self, Self::Error> {
        Self::parse(&id).ok_or_else(|| format!("not an attachment id: {id}"))
    }
}

impl From<AttachmentId> for String {
    fn from(id: AttachmentId) -> Self {
        id.0
    }
}

/// Whether agentZ keeps images of this type.
pub fn is_image_type(mime_type: &str) -> bool {
    IMAGE_TYPES.iter().any(|(_, known)| *known == mime_type)
}

/// The hex SHA-256 of the bytes.
pub fn content_hash(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// An attachment's bytes, in base64: [`crate::Response::AttachmentData`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttachmentData {
    pub mime_type: String,
    pub data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

    #[test]
    fn ids_name_only_hashed_images() {
        let id = AttachmentId::new(HASH, "image/jpeg").expect("a JPEG");
        assert_eq!(id.as_str(), format!("{HASH}.jpg"));
        assert_eq!(id.mime_type(), "image/jpeg");
        assert_eq!(AttachmentId::from_uri(&id.uri()), Some(id.clone()));
        let json = serde_json::to_value(&id).expect("an id encodes");
        assert_eq!(serde_json::from_value::<AttachmentId>(json).ok(), Some(id));

        assert_eq!(
            AttachmentId::for_image(b"test", "image/jpeg").map(|id| id.as_str().to_string()),
            Some(format!("{HASH}.jpg"))
        );
        assert!(AttachmentId::new(HASH, "text/plain").is_none());
        assert!(AttachmentId::parse(&format!("{HASH}.exe")).is_none());
        assert!(AttachmentId::parse("../../etc/passwd.png").is_none());
        assert!(AttachmentId::parse(&format!("{}.png", &HASH[1..])).is_none());
        assert!(serde_json::from_value::<AttachmentId>("../secret.png".into()).is_err());
    }
}
