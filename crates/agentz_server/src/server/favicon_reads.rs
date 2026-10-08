//! Projects' icons, found in their folders on this machine ([`crate::favicons`]) and sent with
//! the projects, and their bytes for clients on other machines.

use std::collections::{BTreeMap, BTreeSet};

use agentz_protocol::Response;
use agentz_protocol::attachments::MAX_ATTACHMENT_SIZE;
use anyhow::{Context as _, anyhow};
use base64::Engine as _;
use projects::ProjectId;
use util::ResultExt as _;

use super::{ClientId, Server};
use crate::favicons;

impl Server {
    /// Looks in every project's folder for its icon.
    pub(super) fn refresh_favicons(&mut self) {
        if self.reading_favicons {
            return;
        }
        self.reading_favicons = true;
        let roots: Vec<_> = self
            .projects
            .projects()
            .iter()
            .map(|project| (project.id, project.path.clone()))
            .collect();
        let read = self.runtime.spawn_blocking(move || {
            let projects: BTreeSet<ProjectId> = roots.iter().map(|(id, _)| *id).collect();
            let favicons: BTreeMap<_, _> = roots
                .into_iter()
                .filter_map(|(id, root)| Some((id, favicons::find(&root)?)))
                .collect();
            (projects, favicons)
        });
        self.spawn_then(read, |server, read| {
            server.reading_favicons = false;
            if let Some((projects, favicons)) = read.log_err() {
                server.favicon_projects = projects;
                server.projects.set_favicons(favicons);
            }
        });
    }

    /// Looks for a new project's icon at once.
    pub(super) fn refresh_new_favicons(&mut self) {
        let has_new_project = self
            .projects
            .projects()
            .iter()
            .any(|project| !self.favicon_projects.contains(&project.id));
        if has_new_project {
            self.refresh_favicons();
        }
    }

    /// The icon found for a project, read off the server's task.
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
