//! Adding a project, as the app's Add Project does: with `machine`, an agent that cloned a
//! repository on another machine adds the clone there, and once the app combines it with the
//! caller's project, the other tools reach that checkout. Any thread may add one, since it
//! runs nothing and the user can remove it; removing projects stays with the user.

use std::path::{Path, PathBuf};
use std::time::Instant;

use futures::FutureExt as _;
use serde_json::{Value, json};

use super::{Arguments, Continuation, Outcome, Server, Step, failure, invalid};
use crate::repositories;

const MAX_PATH_CHARS: usize = 4_096;

impl Server {
    pub(super) fn project_add(&mut self, arguments: &Arguments) -> Outcome {
        let path = arguments
            .string("path", MAX_PATH_CHARS)?
            .ok_or_else(|| invalid("path is required."))?;
        // Collecting the components drops a trailing `/` left by completion.
        let path: PathBuf = crate::directories::expand_home(Path::new(path))
            .components()
            .collect();
        if !path.is_absolute() {
            return Err(invalid("path must be absolute or start with ~."));
        }
        if !path.is_dir() {
            return Err(invalid(format!(
                "{} isn't a folder on this machine.",
                path.display()
            )));
        }
        let canonical = std::fs::canonicalize(&path).unwrap_or(path);
        let already_added = self
            .projects
            .projects()
            .iter()
            .any(|project| project.path == canonical);
        let project_id = self.projects.add_project(canonical.clone());
        let resolved = repositories::resolve(canonical.clone());
        // Looked up here rather than left to the next repository refresh, so the answer says
        // which repository it is and the app can combine it with its others right away.
        Ok(Step::Then(
            async move {
                let resolved = resolved.await;
                Box::new(move |server: &mut Server| {
                    server.repository_checks.finish(
                        canonical.clone(),
                        matches!(resolved, Ok(Some(_))),
                        Instant::now(),
                    );
                    match resolved {
                        Ok(repository) => {
                            server
                                .projects
                                .set_project_repository(project_id, repository);
                        }
                        Err(error) => log::warn!(
                            "couldn't look up {}'s repository: {error:#}",
                            canonical.display()
                        ),
                    }
                    let project = server.projects.project(project_id).ok_or_else(|| {
                        failure("project_not_found", "The project was removed meanwhile.")
                    })?;
                    Ok(Step::Done(json!({
                        "projectId": project.id.0,
                        "name": project.name().to_string(),
                        "path": project.path,
                        "repository": project
                            .repository
                            .as_ref()
                            .map(|repository| repository.canonical_key.clone()),
                        "alreadyAdded": already_added,
                    })))
                }) as Continuation
            }
            .boxed(),
        ))
    }
}

pub(super) fn definitions() -> Vec<Value> {
    vec![json!({
        "name": "agentz_project_add",
        "title": "Add an agentZ project",
        "description": "Add a folder as a project in agentZ, as the user's Add Project does, or find the project it already is. Use it after cloning a repository on another machine (in a Workspaces terminal from agentz_terminal_start), with machine: once the app combines the clone with this project, which it does for the same repository unless the user's project grouping keeps them apart, the tools that take machine work in it. There, combinedWithThisProject says whether it did. repository is the repository's remote as host/owner/name, or null outside git or without a remote.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": {"type": "string", "maxLength": MAX_PATH_CHARS, "description": "The folder, an absolute path or one starting with ~ on the machine."},
                "clientRequestId": {"type": "string", "maxLength": 256, "description": "Stable idempotency key to reuse when retrying this mutation."},
            },
            "required": ["path"],
            "additionalProperties": false,
        },
        "annotations": {"readOnlyHint": false, "destructiveHint": false, "idempotentHint": true},
    })]
}
