//! Projects' icons, chosen or found in their folders on this machine ([`crate::favicons`]) and
//! sent with the projects, their bytes for clients on other machines, and the image files to
//! choose from.

use std::collections::BTreeMap;
use std::path::PathBuf;

use agentz_protocol::Response;
use agentz_protocol::attachments::MAX_ATTACHMENT_SIZE;
use anyhow::{Context as _, anyhow};
use base64::Engine as _;
use projects::ProjectId;
use util::ResultExt as _;

use super::{ClientId, Server, prompt_requests};
use crate::favicons;

/// The image files the icon picker lists, as t3code's (`WORKSPACE_IMAGE_PREVIEW_EXTENSIONS`)
/// without AVIF, which GPUI can't draw.
const IMAGE_EXTENSIONS: [&str; 7] = ["gif", "ico", "jpeg", "jpg", "png", "svg", "webp"];

impl Server {
    /// Finds every project's icon file: the one chosen while it's there, else one in its
    /// folder.
    pub(super) fn refresh_favicons(&mut self) {
        if self.reading_favicons {
            return;
        }
        self.reading_favicons = true;
        let roots: Vec<_> = self
            .projects
            .projects()
            .iter()
            .map(|project| (project.id, project.path.clone(), project.icon_file()))
            .collect();
        let read = self.runtime.spawn_blocking(move || {
            let projects: BTreeMap<ProjectId, Option<PathBuf>> = roots
                .iter()
                .map(|(id, _, chosen)| (*id, chosen.clone()))
                .collect();
            let favicons: BTreeMap<_, _> = roots
                .into_iter()
                .filter_map(|(id, root, chosen)| {
                    let file = chosen
                        .filter(|chosen| chosen.is_file())
                        .or_else(|| favicons::find(&root))?;
                    Some((id, file))
                })
                .collect();
            (projects, favicons)
        });
        self.spawn_then(read, |server, read| {
            server.reading_favicons = false;
            if let Some((projects, favicons)) = read.log_err() {
                server.favicon_projects = projects;
                server.projects.set_favicons(favicons);
                // An icon chosen while the folders were read.
                server.refresh_new_favicons();
            }
        });
    }

    /// Looks for the icon of a new project, or of one whose icon file was just chosen, at
    /// once.
    pub(super) fn refresh_new_favicons(&mut self) {
        let has_change =
            self.projects.projects().iter().any(|project| {
                self.favicon_projects.get(&project.id) != Some(&project.icon_file())
            });
        if has_change {
            self.refresh_favicons();
        }
    }

    /// The image files in the project's folder, read off the server's task.
    pub(super) fn project_image_files(&mut self, client: ClientId, id: u64, project: ProjectId) {
        let Some(root) = self
            .projects
            .project(project)
            .map(|project| project.path.clone())
        else {
            return self.respond(client, id, Err(anyhow!("no such project")));
        };
        let list = move || {
            let mut listing =
                prompt_requests::list_files(root, |entry| !entry.is_dir && is_image(&entry.path))?;
            // The walk goes in the folder's own order, which reads as random in a list.
            listing
                .entries
                .sort_by(|first, second| first.path.cmp(&second.path));
            anyhow::Ok(listing)
        };
        self.spawn_then(
            async move {
                tokio::task::spawn_blocking(list)
                    .await
                    .context("listing the image files")?
            },
            move |server, listing| server.respond(client, id, listing.map(Response::Files)),
        );
    }

    /// The project's icon file, read off the server's task.
    pub(super) fn project_favicon(&mut self, client: ClientId, id: u64, project: ProjectId) {
        let Some(path) = self
            .projects
            .favicon(project)
            .map(|path| path.to_path_buf())
        else {
            return self.respond(client, id, Err(anyhow!("the project has no icon")));
        };
        let read = move || {
            let size = std::fs::metadata(&path)
                .with_context(|| format!("reading {}", path.display()))?
                .len();
            if size > MAX_ATTACHMENT_SIZE as u64 {
                return Err(anyhow!("{} is too large to send", path.display()));
            }
            let bytes =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            Ok(Response::ProjectFavicon(
                base64::engine::general_purpose::STANDARD.encode(bytes),
            ))
        };
        self.spawn_then(
            async move {
                tokio::task::spawn_blocking(read)
                    .await
                    .context("reading the icon")?
            },
            move |server, result| server.respond(client, id, result),
        );
    }
}

fn is_image(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        IMAGE_EXTENSIONS
            .iter()
            .any(|image| extension.eq_ignore_ascii_case(image))
    })
}
