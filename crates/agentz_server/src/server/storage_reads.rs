//! Keeps [`Storage`] current for Settings › Storage (design/storage) while an app is open:
//! measured as threads change, at most every few seconds, and walked through again every few
//! minutes, since agents and builds fill worktrees without a word to the server.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_thread::Attachments;
use agentz_protocol::Response;
use agentz_protocol::agents::AgentId;
use agentz_protocol::storage::StorageCache;
use anyhow::{Context as _, Result, anyhow};

use super::terminal_requests::home_relative;
use super::{ClientId, Server};
use crate::continuations;
use crate::storage::{self, ProjectCheckout, StoragePlan};
use crate::transcripts;

/// How soon after one measurement the next may start, as threads change.
const MEASURE_INTERVAL: Duration = Duration::from_secs(5);
/// How often worktrees, pastures and agents are walked through again.
const FULL_MEASURE_INTERVAL: Duration = Duration::from_secs(5 * 60);
/// How often whether one is due is looked at, without a change to the threads.
pub(super) const STORAGE_CHECK_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(super) struct StorageReads {
    reading: bool,
    /// A measurement waits for [`MEASURE_INTERVAL`] to pass.
    scheduled: bool,
    /// The projects' revision as of the last measurement.
    measured_revision: Option<u64>,
    measured_at: Option<Instant>,
    fully_measured_at: Option<Instant>,
    /// The next measurement walks through the agents again: one was installed or removed.
    pub(super) agents_due: bool,
}

impl Server {
    /// Measures what changed since the last measurement, or everything when that's due.
    pub(super) fn refresh_storage(&mut self) {
        let reads = &self.storage_reads;
        if reads.reading || reads.scheduled || !self.has_session_subscribers() {
            return;
        }
        let full = reads
            .fully_measured_at
            .is_none_or(|at| at.elapsed() >= FULL_MEASURE_INTERVAL);
        let revision = self.projects.revision();
        if !full && !reads.agents_due && reads.measured_revision == Some(revision) {
            return;
        }
        if let Some(wait) = reads
            .measured_at
            .and_then(|at| MEASURE_INTERVAL.checked_sub(at.elapsed()))
        {
            self.storage_reads.scheduled = true;
            self.spawn_then(tokio::time::sleep(wait), |server, ()| {
                server.storage_reads.scheduled = false;
                server.refresh_storage();
            });
            return;
        }
        let plan = self.storage_plan(full);
        let reads = &mut self.storage_reads;
        reads.reading = true;
        reads.agents_due = false;
        self.spawn_then(storage::measure(plan), move |server, mut measured| {
            let reads = &mut server.storage_reads;
            reads.reading = false;
            reads.measured_revision = Some(revision);
            reads.measured_at = Some(Instant::now());
            if full {
                reads.fully_measured_at = Some(Instant::now());
            }
            // Deleted while it was measured.
            measured.checkouts.retain(|checkout| checkout.path.exists());
            server.storage = measured;
            server.forget_thread_storage();
            server.update_node_in_use();
            // Whatever changed meanwhile.
            server.refresh_storage();
        });
    }

    fn storage_plan(&self, full: bool) -> StoragePlan {
        let data_dir = &self.data_dir;
        let threads = self
            .projects
            .threads()
            .iter()
            .filter(|thread| thread.parent().is_none() && !thread.is_draft)
            .map(|thread| {
                let mut paths = Vec::new();
                for thread_id in self.projects.thread_and_subthreads(thread.id) {
                    paths.push(transcripts::path(data_dir, thread_id));
                    paths.push(continuations::path(data_dir, thread_id));
                    paths.push(
                        Attachments::for_thread(data_dir, thread_id)
                            .directory()
                            .to_path_buf(),
                    );
                }
                if thread.is_chat()
                    && let Some(folder) = &thread.workspace
                {
                    paths.push(folder.clone());
                }
                (thread.id, paths)
            })
            .collect();
        let checkouts = self
            .projects
            .projects()
            .iter()
            .flat_map(|project| {
                project.workspaces.iter().map(|workspace| ProjectCheckout {
                    project_id: project.id,
                    project_folder: project.path.clone(),
                    workspace: workspace.clone(),
                })
            })
            .collect();
        let agents = self
            .registry
            .installed()
            .map(|agent_id| {
                let folders = self.registry.agent_folders(agent_id).to_vec();
                (agent_id.clone(), folders)
            })
            .collect();
        StoragePlan {
            data_dir: data_dir.clone(),
            data_folder: home_relative(data_dir),
            threads,
            checkouts,
            agents,
            registry_cache: self.registry.cache_paths().to_vec(),
            node_dir: self.registry.node_dir().to_path_buf(),
            server_log: server_log(data_dir),
            previous: (!full).then(|| {
                let mut previous = self.storage.clone();
                if self.storage_reads.agents_due {
                    previous.agents.clear();
                }
                previous
            }),
        }
    }

    fn has_session_subscribers(&self) -> bool {
        self.clients
            .values()
            .any(|client| client.subscribed_to_session)
    }

    /// Kept as agents start and stop, which don't change the projects.
    pub(super) fn update_node_in_use(&mut self) {
        let in_use = self.storage.node.is_some() && self.node_in_use();
        if let Some(node) = &mut self.storage.node {
            node.in_use = in_use;
        }
    }

    /// Whether an agent running now runs on the downloaded Node.js.
    fn node_in_use(&self) -> bool {
        if !self.registry.uses_downloaded_node() {
            return false;
        }
        let threads = self.threads.keys().filter_map(|thread_id| {
            let agent_id = self.projects.thread(*thread_id)?.agent_id.clone()?;
            Some(AgentId::new(agent_id))
        });
        let login_sessions = self
            .login_sessions
            .values()
            .map(|login_session| login_session.agent_id.clone());
        threads
            .chain(login_sessions)
            .any(|agent_id| self.registry.is_npm_agent(&agent_id))
    }

    pub(super) fn clear_storage(&mut self, client: ClientId, id: u64, cache: StorageCache) {
        match cache {
            StorageCache::Node => {
                if self.node_in_use() {
                    return self.respond(
                        client,
                        id,
                        Err(anyhow!(
                            "an agent is running on it; stop its threads before deleting it"
                        )),
                    );
                }
                let deleting = self.registry.delete_downloaded_node();
                self.spawn_then(deleting, move |server, deleted| {
                    if deleted.is_ok() {
                        server.storage.node = None;
                    }
                    server.respond(client, id, deleted.map(|()| Response::Ok));
                });
            }
            StorageCache::ServerLog => {
                let log = server_log(&self.data_dir);
                let clearing = tokio::task::spawn_blocking(move || empty_log(&log));
                self.spawn_then(clearing, move |server, cleared| {
                    let cleared = cleared
                        .map_err(anyhow::Error::from)
                        .and_then(|cleared| cleared);
                    if cleared.is_ok() {
                        server.storage.server_log = 0;
                    }
                    server.respond(client, id, cleared.map(|()| Response::Ok));
                });
            }
            StorageCache::RegistryCache => {
                // Measured again once it's fetched, as the registry changes.
                self.registry.clear_cache();
                self.storage.registry_cache = 0;
                self.respond(client, id, Ok(Response::Ok));
            }
        }
    }

    /// Forgets deleted threads' sizes at once, rather than at the next measurement.
    pub(super) fn forget_thread_storage(&mut self) {
        let projects = &self.projects;
        self.storage
            .threads
            .retain(|thread| projects.thread(thread.thread_id).is_some());
    }
}

fn server_log(data_dir: &Path) -> PathBuf {
    data_dir.join("logs").join("server.log")
}

/// The server appends to it, so it's emptied in place.
fn empty_log(path: &Path) -> Result<()> {
    match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(log) => log
            .set_len(0)
            .with_context(|| format!("emptying {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("opening {}", path.display())),
    }
}
