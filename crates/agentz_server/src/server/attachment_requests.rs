//! Images attached to threads' messages ([`agentz_protocol::attachments`]), image files their
//! agents name, and files uploaded for agents on another machine than the app.

use std::io::Cursor;
use std::path::Path;

use agent_thread::Attachments;
use agentz_protocol::attachments::{self, AttachmentData, AttachmentId, MAX_ATTACHMENT_SIZE};
use agentz_protocol::{Request, Response};
use anyhow::{Context as _, Result, anyhow};
use base64::Engine as _;
use util::ResultExt as _;

use super::{ClientId, Server};

/// Thumbnails fit hover previews of up to 320 by 240 points on a Retina screen.
const THUMBNAIL_SIZE: u32 = 640;

impl Server {
    /// Keeps or reads an attachment. Reading, and keeping files, happen off the server's task,
    /// since they may be large.
    pub(super) fn attachment_request(
        &mut self,
        client: ClientId,
        request_id: u64,
        request: Request,
    ) {
        let thread_id = match &request {
            Request::AddAttachment { thread_id, .. }
            | Request::Attachment { thread_id, .. }
            | Request::ImageFile { thread_id, .. }
            | Request::UploadFile { thread_id, .. } => *thread_id,
            _ => {
                return self.respond(
                    client,
                    request_id,
                    Err(anyhow!("not an attachment request")),
                );
            }
        };
        if let Err(error) = self.existing_thread(thread_id) {
            return self.respond(client, request_id, Err(error));
        }
        let attachments = Attachments::for_thread(&self.data_dir, thread_id);
        let thread_folder = self.projects.thread_folder(thread_id);
        // A client names an image itself and sends the message with it right after, so the
        // image is kept before the server takes the client's next request.
        let is_image = matches!(request, Request::AddAttachment { .. });
        let work = move || match request {
            Request::AddAttachment {
                mime_type, data, ..
            } => attachments
                .add_base64(&mime_type, &data)
                .map(Response::Attachment),
            Request::Attachment { id, thumbnail, .. } => {
                read_attachment(&attachments, &id, thumbnail).map(Response::AttachmentData)
            }
            Request::ImageFile { path, .. } => {
                read_image_file(&path, thread_folder.as_deref()).map(Response::AttachmentData)
            }
            Request::UploadFile { name, data, .. } => {
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .context("decoding the file")?;
                attachments
                    .add_file(&name, &bytes)
                    .map(Response::UploadedFile)
            }
            _ => Err(anyhow!("not an attachment request")),
        };
        if is_image {
            let result = work();
            return self.respond(client, request_id, result);
        }
        self.spawn_then(
            async move {
                tokio::task::spawn_blocking(work)
                    .await
                    .context("reading the attachment")?
            },
            move |server, result| server.respond(client, request_id, result),
        );
    }
}

/// The image, or a thumbnail of it, made once. A small image is its own thumbnail, as is one
/// that can't be scaled here (an SVG).
fn read_attachment(
    attachments: &Attachments,
    id: &AttachmentId,
    thumbnail: bool,
) -> Result<AttachmentData> {
    let base64 = &base64::engine::general_purpose::STANDARD;
    if thumbnail && let Some(png) = attachments.thumbnail(id) {
        return Ok(AttachmentData {
            mime_type: "image/png".to_string(),
            data: base64.encode(png),
        });
    }
    let original = attachments.read(id)?;
    if thumbnail && id.mime_type() != "image/svg+xml" {
        match make_thumbnail(&original) {
            Ok(Some(png)) => {
                attachments.keep_thumbnail(id, &png).log_err();
                return Ok(AttachmentData {
                    mime_type: "image/png".to_string(),
                    data: base64.encode(png),
                });
            }
            Ok(None) => {}
            Err(error) => log::warn!("failed to make a thumbnail of {}: {error:#}", id.as_str()),
        }
    }
    Ok(AttachmentData {
        mime_type: id.mime_type().to_string(),
        data: base64.encode(original),
    })
}

/// An image file a thread names, at `path` from the folder it works in. Only images are read,
/// and none larger than a client could be sent.
fn read_image_file(path: &str, thread_folder: Option<&Path>) -> Result<AttachmentData> {
    let path = crate::directories::expand_home(Path::new(path));
    let path = if path.is_absolute() {
        path
    } else {
        thread_folder
            .context("the thread has no folder")?
            .join(path)
    };
    let mime_type = attachments::image_type_of_path(&path)
        .with_context(|| format!("not an image: {}", path.display()))?;
    let size = std::fs::metadata(&path)
        .with_context(|| format!("reading {}", path.display()))?
        .len();
    if size > MAX_ATTACHMENT_SIZE as u64 {
        return Err(anyhow!(
            "{} is too large to show ({})",
            path.display(),
            size
        ));
    }
    let bytes = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(AttachmentData {
        mime_type: mime_type.to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

/// A PNG of the image scaled to fit [`THUMBNAIL_SIZE`], unless it fits already.
fn make_thumbnail(bytes: &[u8]) -> Result<Option<Vec<u8>>> {
    let image = image::load_from_memory(bytes).context("decoding the image")?;
    if image.width() <= THUMBNAIL_SIZE && image.height() <= THUMBNAIL_SIZE {
        return Ok(None);
    }
    let thumbnail = image.thumbnail(THUMBNAIL_SIZE, THUMBNAIL_SIZE);
    let mut png = Vec::new();
    thumbnail
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .context("encoding the thumbnail")?;
    Ok(Some(png))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
        let mut png = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("encoded");
        png
    }

    #[test]
    fn thumbnails_fit_hover_previews() {
        let data_dir = tempfile::tempdir().expect("a temporary folder");
        let attachments = Attachments::for_thread(data_dir.path(), projects::ThreadId(1));
        let large = attachments.add("image/png", &png(1600, 800)).expect("kept");
        let thumbnail = read_attachment(&attachments, &large, true).expect("a thumbnail");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(thumbnail.data)
            .expect("base64");
        let scaled = image::load_from_memory(&bytes).expect("a PNG");
        assert_eq!((scaled.width(), scaled.height()), (640, 320));
        assert!(attachments.thumbnail(&large).is_some());

        let original = read_attachment(&attachments, &large, false).expect("the original");
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(original.data)
                .expect("base64"),
            png(1600, 800)
        );

        let small = attachments.add("image/png", &png(20, 10)).expect("kept");
        let thumbnail = read_attachment(&attachments, &small, true).expect("a thumbnail");
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(thumbnail.data)
                .expect("base64"),
            png(20, 10)
        );
        assert!(attachments.thumbnail(&small).is_none());
    }

    #[test]
    fn image_files_are_read_from_the_threads_folder() {
        let folder = tempfile::tempdir().expect("a temporary folder");
        std::fs::create_dir(folder.path().join("shots")).expect("a folder");
        std::fs::write(folder.path().join("shots/cart.png"), png(1600, 800)).expect("written");
        std::fs::write(folder.path().join("notes.txt"), "not an image").expect("written");
        let decode = |data: AttachmentData| {
            base64::engine::general_purpose::STANDARD
                .decode(data.data)
                .expect("base64")
        };

        let relative = read_image_file("shots/cart.png", Some(folder.path())).expect("the image");
        assert_eq!(relative.mime_type, "image/png");
        assert_eq!(decode(relative), png(1600, 800));
        let absolute = folder.path().join("shots/cart.png");
        let absolute = read_image_file(&absolute.to_string_lossy(), None).expect("the image");
        assert_eq!(decode(absolute), png(1600, 800));

        assert!(read_image_file("notes.txt", Some(folder.path())).is_err());
        assert!(read_image_file("missing.png", Some(folder.path())).is_err());
        assert!(read_image_file("shots/cart.png", None).is_err());
    }
}
