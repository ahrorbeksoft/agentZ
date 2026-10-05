//! Images attached to threads' messages ([`agentz_protocol::attachments`]), and files uploaded
//! for agents on another machine than the app.

use std::io::Cursor;

use agent_thread::Attachments;
use agentz_protocol::attachments::{AttachmentData, AttachmentId};
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
}
