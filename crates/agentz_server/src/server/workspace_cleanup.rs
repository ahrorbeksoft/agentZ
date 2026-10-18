//! Removing worktrees and pastures on their own (design/archive-cleanup), by t3code's storage
//! cleanup rules (`references/t3code/apps/server/src/storageCleanup.ts`): some time after their
//! thread is archived, once it's deleted or inactive, when merged, or when they have nothing of
//! their own. A workspace with changes or unpushed commits stays, as does one two threads use,
//! and one a working or open thread or a terminal is in. The rules run every hour, as they
//! change, and with Clean Up Now. A removed workspace stays one of its project's, so its
//! thread's next message makes it again ([`Server::remake_workspace`]).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use agentz_protocol::Response;
use agentz_protocol::storage::{
    ArchivedCleanup, CleanupEntry, CleanupOutcome, CleanupReport, CleanupRules,
};
use agentz_protocol::workspace::WorkspaceRemoval;
use anyhow::{Result, anyhow};
use collections::{HashMap, HashSet};
use projects::{ProjectId, Thread, ThreadId, Workspace, WorkspaceKind};
use serde::{Deserialize, Serialize};
use util::ResultExt as _;

use super::{ClientId, Server};
use crate::agent_settings::{read_json, write_json};
use crate::storage;
use crate::workspaces;

/// t3code's hourly cleanup.
pub(super) const CLEANUP_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// The first run after the server starts, which updates restart often enough to skip runs.
pub(super) const FIRST_CLEANUP_DELAY: Duration = Duration::from_secs(5 * 60);
const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("workspace-cleanup.json")
}

/// What `workspace-cleanup.json` keeps.
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct CleanupFile {
    #[serde(default)]
    pub rules: CleanupRules,
    #[serde(default)]
    pub last_run: Option<CleanupReport>,
    #[serde(default)]
    deleted: Vec<DeletedWorkspace>,
    #[serde(default)]
    removed: Vec<RemovedWorkspace>,
}

/// A workspace whose thread was deleted, for the deleted rule.
#[derive(Clone, Serialize, Deserialize)]
struct DeletedWorkspace {
    path: PathBuf,
    thread_title: String,
}

/// A workspace a cleanup removed while its thread stayed, to make again.
#[derive(Clone, Serialize, Deserialize)]
struct RemovedWorkspace {
    path: PathBuf,
    /// The commit it was at.
    head: Option<String>,
}

pub(crate) fn load(data_dir: &Path) -> Result<CleanupFile> {
    Ok(read_json::<CleanupFile>(&path(data_dir))?.unwrap_or_default())
}

#[derive(Default)]
pub(super) struct Cleanups {
    deleted: Vec<DeletedWorkspace>,
    removed: Vec<RemovedWorkspace>,
    running: bool,
    /// Another run follows this one: the rules or the threads changed meanwhile.
    again: bool,
    /// Clean Up Now's requests, answered as the run ends.
    answering: Vec<(ClientId, u64)>,
    /// Those that came during it, answered after the next.
    answering_next: Vec<(ClientId, u64)>,
    /// Folders being removed now, which no agent may start in.
    pub(super) removing: HashSet<PathBuf>,
}

impl Cleanups {
    pub(super) fn new(file: &CleanupFile) -> Self {
        Self {
            deleted: file.deleted.clone(),
            removed: file.removed.clone(),
            ..Self::default()
        }
    }

    pub(super) fn is_deleted_threads(&self, path: &Path) -> bool {
        self.deleted.iter().any(|deleted| deleted.path == path)
    }

    /// The commit a removed workspace was at.
    pub(super) fn removed_head(&self, path: &Path) -> Option<String> {
        self.removed
            .iter()
            .find(|removed| removed.path == path)
            .and_then(|removed| removed.head.clone())
    }
}

struct Candidate {
    project_id: ProjectId,
    repo: PathBuf,
    workspace: Workspace,
    thread_title: Option<String>,
    /// The thread working there, with its subthreads; `None` for a deleted thread's.
    thread_id: Option<ThreadId>,
    last_activity: Option<SystemTime>,
    /// As last measured.
    bytes: Option<u64>,
    verdict: Verdict,
}

enum Verdict {
    Keep(String),
    Remove(String),
    /// Removed if its branch is merged into the default branch, or has nothing beyond it.
    CheckBranch {
        /// Why it stays otherwise, if a time rule says.
        keep: Option<String>,
    },
}

struct Inspected {
    candidate: Candidate,
    /// `Ok` to remove it, with why, or why it stays.
    removal: std::result::Result<String, String>,
    head: Option<String>,
    bytes: u64,
}

impl Server {
    pub(super) fn set_cleanup_rules(&mut self, mut rules: CleanupRules) -> Result<()> {
        rules.inactive_days = rules
            .inactive_days
            .map(|days| days.clamp(1, CleanupRules::MAX_INACTIVE_DAYS));
        if rules == self.storage.cleanup.rules {
            return Ok(());
        }
        self.storage.cleanup.rules = rules;
        self.save_cleanups()?;
        self.clean_up_workspaces();
        Ok(())
    }

    pub(super) fn clean_up_now(&mut self, client: ClientId, id: u64) {
        if !self.storage.cleanup.rules.any() {
            return self.respond(client, id, Err(anyhow!("turn on a cleanup rule first")));
        }
        if self.cleanups.running {
            self.cleanups.answering_next.push((client, id));
        } else {
            self.cleanups.answering.push((client, id));
        }
        self.clean_up_workspaces();
    }

    /// An archived thread's workspace goes at once with Right away.
    pub(super) fn thread_archived(&mut self) {
        if self.storage.cleanup.rules.archived == ArchivedCleanup::RightAway {
            self.clean_up_workspaces();
        }
    }

    /// Notes the workspaces of a thread about to be deleted, with its subthreads, that no other
    /// thread uses: the deleted rule removes them.
    pub(super) fn note_deleted_workspaces(&mut self, thread_id: ThreadId) -> Vec<PathBuf> {
        let Some(thread) = self
            .projects
            .thread(thread_id)
            .filter(|thread| !thread.is_draft)
        else {
            return Vec::new();
        };
        let title = thread.title.clone();
        let threads = self.projects.thread_and_subthreads(thread_id);
        let mut paths: Vec<PathBuf> = threads
            .iter()
            .filter_map(|thread_id| self.projects.thread_workspace(*thread_id))
            .map(|workspace| workspace.path.clone())
            .collect();
        paths.sort();
        paths.dedup();
        for path in &paths {
            if !self.cleanups.is_deleted_threads(path) {
                self.cleanups.deleted.push(DeletedWorkspace {
                    path: path.clone(),
                    thread_title: title.clone(),
                });
            }
        }
        paths
    }

    /// After a thread's deletion: its workspaces another thread still uses aren't a deleted
    /// thread's, and the deleted rule removes the rest.
    pub(super) fn thread_deleted(&mut self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let used: Vec<PathBuf> = paths
            .into_iter()
            .filter(|path| !self.workspace_users(path).is_empty())
            .collect();
        self.cleanups
            .deleted
            .retain(|deleted| !used.contains(&deleted.path));
        self.forget_gone_workspaces();
        self.save_cleanups().log_err();
        if self.storage.cleanup.rules.deleted {
            self.clean_up_workspaces();
        }
    }

    /// The workspace was made again.
    pub(super) fn forget_removed_workspace(&mut self, path: &Path) {
        self.cleanups.removed.retain(|removed| removed.path != path);
        self.save_cleanups().log_err();
    }

    /// Starts a run, or another once the one running ends.
    pub(super) fn clean_up_workspaces(&mut self) {
        if self.cleanups.running {
            self.cleanups.again = true;
            return;
        }
        self.forget_gone_workspaces();
        let rules = self.storage.cleanup.rules.clone();
        if !rules.any() {
            self.answer_cleanup_requests();
            return;
        }
        let candidates = self.cleanup_candidates(&rules);
        self.cleanups.running = true;
        self.storage.cleanup.running = true;
        self.spawn_then(
            inspect(candidates, rules.clone()),
            move |server, inspected| server.remove_cleaned_up(inspected, rules),
        );
    }

    /// Forgets workspaces whose folders are gone and that no thread uses: there's nothing to
    /// make again or to remove.
    fn forget_gone_workspaces(&mut self) {
        let mut gone = Vec::new();
        for project in self.projects.projects() {
            for workspace in &project.workspaces {
                if !workspace.path.exists() && self.workspace_users(&workspace.path).is_empty() {
                    gone.push((project.id, workspace.path.clone()));
                }
            }
        }
        for (project_id, path) in &gone {
            self.projects.remove_workspace(*project_id, path);
        }
        let projects = &self.projects;
        let is_kept = |path: &Path| {
            projects
                .projects()
                .iter()
                .any(|project| project.workspaces.iter().any(|kept| kept.path == path))
        };
        let deleted_count = self.cleanups.deleted.len();
        let removed_count = self.cleanups.removed.len();
        self.cleanups
            .deleted
            .retain(|deleted| is_kept(&deleted.path) && deleted.path.exists());
        self.cleanups
            .removed
            .retain(|removed| is_kept(&removed.path) && !removed.path.exists());
        if self.cleanups.deleted.len() != deleted_count
            || self.cleanups.removed.len() != removed_count
        {
            self.save_cleanups().log_err();
        }
    }

    /// The threads working in the folder, each with its subthreads.
    fn workspace_users(&self, path: &Path) -> Vec<ThreadId> {
        let mut users: Vec<ThreadId> = self
            .projects
            .threads()
            .iter()
            .filter(|thread| thread.workspace.as_deref() == Some(path))
            .map(|thread| self.projects.root_thread(thread.id))
            .collect();
        users.sort();
        users.dedup();
        users
    }

    fn cleanup_candidates(&self, rules: &CleanupRules) -> Vec<Candidate> {
        let now = SystemTime::now();
        let watched = self.watched_threads();
        let thread_rules = rules.archived != ArchivedCleanup::Never
            || rules.inactive_days.is_some()
            || rules.merged
            || rules.unchanged;
        let mut candidates = Vec::new();
        for project in self.projects.projects() {
            for workspace in &project.workspaces {
                let path = &workspace.path;
                if !path.exists() {
                    continue;
                }
                let users = self.workspace_users(path);
                let mut candidate = Candidate {
                    project_id: project.id,
                    repo: project.path.clone(),
                    workspace: workspace.clone(),
                    thread_title: None,
                    thread_id: None,
                    last_activity: None,
                    bytes: self
                        .storage
                        .checkouts
                        .iter()
                        .find(|checkout| &checkout.path == path)
                        .map(|checkout| checkout.bytes),
                    verdict: Verdict::Keep(String::new()),
                };
                candidate.verdict = match users.as_slice() {
                    [] => {
                        let Some(deleted) = self
                            .cleanups
                            .deleted
                            .iter()
                            .find(|deleted| &deleted.path == path)
                            .filter(|_| rules.deleted)
                        else {
                            continue;
                        };
                        candidate.thread_title = Some(deleted.thread_title.clone());
                        match self.busy_reason(path, None, &watched) {
                            Some(reason) => Verdict::Keep(reason),
                            None => Verdict::Remove("Its thread was deleted".into()),
                        }
                    }
                    [thread_id] => {
                        let Some(thread) = self.projects.thread(*thread_id) else {
                            continue;
                        };
                        let Some(verdict) = rule_verdict(rules, thread, workspace.kind, now) else {
                            continue;
                        };
                        candidate.thread_title = Some(thread.title.clone());
                        candidate.thread_id = Some(*thread_id);
                        candidate.last_activity = thread.last_activity_at;
                        match verdict {
                            Verdict::Keep(reason) => Verdict::Keep(reason),
                            verdict => self
                                .busy_reason(path, Some(*thread_id), &watched)
                                .map_or(verdict, Verdict::Keep),
                        }
                    }
                    users => {
                        if !thread_rules {
                            continue;
                        }
                        candidate.thread_title = self
                            .projects
                            .thread(users[0])
                            .map(|thread| thread.title.clone());
                        Verdict::Keep(format!("Shared by {} threads", users.len()))
                    }
                };
                candidates.push(candidate);
            }
        }
        candidates
    }

    /// Why a workspace that rules may remove stays as things are now: a thread there is working
    /// or open, or a terminal is.
    fn busy_reason(
        &self,
        path: &Path,
        thread_id: Option<ThreadId>,
        watched: &HashSet<ThreadId>,
    ) -> Option<String> {
        let mut threads = self.projects.threads_in_folder(path);
        if let Some(thread_id) = thread_id {
            threads.extend(self.projects.thread_and_subthreads(thread_id));
        }
        if threads
            .iter()
            .any(|thread_id| self.needs_agent(*thread_id) || self.has_workspace_setup(*thread_id))
        {
            return Some("Its thread is working".into());
        }
        if threads.iter().any(|thread_id| watched.contains(thread_id)) {
            return Some("Its thread is open".into());
        }
        if self.has_terminal_in(path) {
            return Some("A terminal is open in it".into());
        }
        None
    }

    /// Removes what the rules found, unless something changed since they looked.
    fn remove_cleaned_up(&mut self, inspected: Vec<Inspected>, rules: CleanupRules) {
        let watched = self.watched_threads();
        let mut entries = Vec::new();
        let mut removals = Vec::new();
        for item in inspected {
            let removal = match &item.removal {
                Ok(reason) => match self.changed_since_check(&item.candidate, &rules, &watched) {
                    Some(changed) => Err(changed),
                    None => Ok(reason.clone()),
                },
                Err(reason) => Err(reason.clone()),
            };
            match removal {
                Ok(reason) => {
                    self.cleanups
                        .removing
                        .insert(item.candidate.workspace.path.clone());
                    removals.push((item, reason));
                }
                Err(reason) => entries.push(entry(&item, CleanupOutcome::Kept, reason)),
            }
        }
        let removing = async move {
            let mut results = Vec::new();
            for (item, reason) in removals {
                let candidate = &item.candidate;
                let result = workspaces::remove(&candidate.repo, &candidate.workspace, false).await;
                results.push((item, reason, result));
            }
            results
        };
        self.spawn_then(removing, move |server, results| {
            server.finish_cleanup(entries, results)
        });
    }

    fn changed_since_check(
        &self,
        candidate: &Candidate,
        rules: &CleanupRules,
        watched: &HashSet<ThreadId>,
    ) -> Option<String> {
        let path = &candidate.workspace.path;
        if *rules != self.storage.cleanup.rules {
            return Some("The rules changed during the check".into());
        }
        let users = self.workspace_users(path);
        if users != candidate.thread_id.into_iter().collect::<Vec<_>>() {
            return Some("Another thread started using it during the check".into());
        }
        if let Some(thread_id) = candidate.thread_id
            && self
                .projects
                .thread(thread_id)
                .is_none_or(|thread| thread.last_activity_at != candidate.last_activity)
        {
            return Some("Its thread was used during the check".into());
        }
        if candidate.thread_id.is_none() && !self.cleanups.is_deleted_threads(path) {
            return Some("Its thread was deleted, but another uses it now".into());
        }
        self.busy_reason(path, candidate.thread_id, watched)
    }

    fn finish_cleanup(
        &mut self,
        mut entries: Vec<CleanupEntry>,
        results: Vec<(Inspected, String, Result<WorkspaceRemoval>)>,
    ) {
        let mut removed = Vec::new();
        let mut failed = Vec::new();
        for (item, reason, result) in results {
            let path = item.candidate.workspace.path.clone();
            self.cleanups.removing.remove(&path);
            match result {
                Ok(WorkspaceRemoval::Removed) => {
                    log::info!("cleanup removed {} ({reason})", path.display());
                    self.workspace_cleaned_up(&item);
                    removed.push(entry(&item, CleanupOutcome::Removed, reason));
                }
                Ok(WorkspaceRemoval::NeedsConfirmation(why)) => {
                    entries.push(entry(&item, CleanupOutcome::Kept, why));
                }
                Ok(WorkspaceRemoval::Unknown(_)) => {
                    entries.push(entry(
                        &item,
                        CleanupOutcome::Kept,
                        "It wasn't removed".into(),
                    ));
                }
                Err(error) => {
                    log::warn!("cleanup couldn't remove {}: {error:#}", path.display());
                    failed.push(entry(&item, CleanupOutcome::Failed, format!("{error:#}")));
                }
            }
        }
        removed.extend(failed);
        removed.extend(entries);
        self.storage.cleanup.last_run = Some(CleanupReport {
            finished_at: SystemTime::now(),
            entries: removed,
        });
        self.storage.cleanup.running = false;
        self.cleanups.running = false;
        self.save_cleanups().log_err();
        self.answer_cleanup_requests();
        if std::mem::take(&mut self.cleanups.again) {
            self.cleanups.answering = std::mem::take(&mut self.cleanups.answering_next);
            self.clean_up_workspaces();
        }
    }

    /// A deleted thread's workspace is forgotten; another's stays its project's, to be made
    /// again with its thread's next message. The agents that worked there stop.
    fn workspace_cleaned_up(&mut self, item: &Inspected) {
        let candidate = &item.candidate;
        let path = &candidate.workspace.path;
        match candidate.thread_id {
            None => {
                self.projects.remove_workspace(candidate.project_id, path);
                self.cleanups
                    .deleted
                    .retain(|deleted| &deleted.path != path);
            }
            Some(thread_id) => {
                self.cleanups
                    .removed
                    .retain(|removed| &removed.path != path);
                self.cleanups.removed.push(RemovedWorkspace {
                    path: path.clone(),
                    head: item.head.clone(),
                });
                for thread_id in self.projects.thread_and_subthreads(thread_id) {
                    if self.threads.contains_key(&thread_id) {
                        self.stop_agent(thread_id);
                    }
                }
            }
        }
        for thread_id in self.projects.threads_in_folder(path) {
            if self.threads.contains_key(&thread_id) {
                self.stop_agent(thread_id);
            }
        }
        self.storage
            .checkouts
            .retain(|checkout| &checkout.path != path);
    }

    fn answer_cleanup_requests(&mut self) {
        for (client, id) in std::mem::take(&mut self.cleanups.answering) {
            self.respond(client, id, Ok(Response::Ok));
        }
    }

    fn save_cleanups(&self) -> Result<()> {
        let file = CleanupFile {
            rules: self.storage.cleanup.rules.clone(),
            last_run: self.storage.cleanup.last_run.clone(),
            deleted: self.cleanups.deleted.clone(),
            removed: self.cleanups.removed.clone(),
        };
        write_json(&path(&self.data_dir), &file)
    }
}

/// What the time rules say about a thread's workspace, and whether its branch is to be checked;
/// `None` when no rule is about such a workspace.
fn rule_verdict(
    rules: &CleanupRules,
    thread: &Thread,
    kind: WorkspaceKind,
    now: SystemTime,
) -> Option<Verdict> {
    let days_since = |time: SystemTime| now.duration_since(time).unwrap_or_default();
    if let (Some(after), Some(archived_at)) = (rules.archived.after(), thread.archived_at) {
        let archived_for = days_since(archived_at);
        if archived_for >= after {
            return Some(Verdict::Remove(match whole_days(archived_for) {
                0 => "Archived".into(),
                days => format!("Archived for {}", plural_days(days)),
            }));
        }
    }
    let idle_days = thread
        .last_activity_at
        .or(thread.created_at)
        .map(|time| whole_days(days_since(time)));
    if let (Some(days), Some(idle_days)) = (rules.inactive_days, idle_days)
        && idle_days >= u64::from(days)
    {
        return Some(Verdict::Remove(format!(
            "Inactive for {}",
            plural_days(idle_days)
        )));
    }
    let checks_branch = rules.unchanged || (rules.merged && kind == WorkspaceKind::Worktree);
    let keep = if let Some(days) = rules.inactive_days {
        Some(format!(
            "Inactive for {} of {days} days",
            idle_days.unwrap_or_default()
        ))
    } else if let Some(after) = rules.archived.after() {
        match thread.archived_at {
            Some(archived_at) => Some(match whole_days(days_since(archived_at)) {
                0 => "Archived less than a day ago".into(),
                days => format!("Archived for {days} of {} days", whole_days(after)),
            }),
            None if checks_branch => None,
            None => Some("Not archived".into()),
        }
    } else {
        None
    };
    match (keep, checks_branch) {
        (keep, true) => Some(Verdict::CheckBranch { keep }),
        (Some(keep), false) => Some(Verdict::Keep(keep)),
        (None, false) => None,
    }
}

fn whole_days(duration: Duration) -> u64 {
    duration.as_secs() / SECONDS_PER_DAY
}

fn plural_days(days: u64) -> String {
    if days == 1 {
        "1 day".into()
    } else {
        format!("{days} days")
    }
}

fn entry(item: &Inspected, outcome: CleanupOutcome, reason: String) -> CleanupEntry {
    CleanupEntry {
        path: item.candidate.workspace.path.clone(),
        kind: item.candidate.workspace.kind,
        thread_title: item.candidate.thread_title.clone(),
        outcome,
        reason,
        bytes: item.bytes,
    }
}

/// Looks at what removing each workspace would lose, and at the branches the merged and
/// unchanged rules go by, fetching each project's default branch once (t3code).
async fn inspect(candidates: Vec<Candidate>, rules: CleanupRules) -> Vec<Inspected> {
    let mut defaults: HashMap<PathBuf, Option<String>> = HashMap::default();
    let mut inspected = Vec::new();
    for candidate in candidates {
        let path = candidate.workspace.path.clone();
        let mut head = None;
        let removal = removal(&candidate, &rules, &mut defaults, &mut head).await;
        let bytes = match candidate.bytes {
            Some(bytes) => bytes,
            None => {
                let measured = path.clone();
                tokio::task::spawn_blocking(move || storage::disk_usage(&measured))
                    .await
                    .unwrap_or_default()
            }
        };
        inspected.push(Inspected {
            candidate,
            removal,
            head,
            bytes,
        });
    }
    inspected
}

/// `Ok` with why the workspace goes, or why it stays.
async fn removal(
    candidate: &Candidate,
    rules: &CleanupRules,
    defaults: &mut HashMap<PathBuf, Option<String>>,
    head: &mut Option<String>,
) -> std::result::Result<String, String> {
    let path = &candidate.workspace.path;
    if let Verdict::Keep(reason) = &candidate.verdict {
        return Err(reason.clone());
    }
    if let Some(reason) = local_work(path).await {
        return Err(reason);
    }
    *head = workspaces::head_commit(path).await;
    match &candidate.verdict {
        Verdict::Keep(reason) => Err(reason.clone()),
        Verdict::Remove(reason) => Ok(reason.clone()),
        Verdict::CheckBranch { keep } => {
            let repo = &candidate.repo;
            let default = match defaults.get(repo) {
                Some(default) => default.clone(),
                None => {
                    let default = fetch_default_branch(repo).await;
                    defaults.insert(repo.clone(), default.clone());
                    default
                }
            };
            branch_removal(
                rules,
                candidate,
                default.as_deref(),
                head.as_deref(),
                keep.clone(),
            )
            .await
        }
    }
}

/// Why removing the folder would lose work: changes, or commits no remote has (or no remote
/// to push them to).
async fn local_work(path: &Path) -> Option<String> {
    match workspaces::changed_files(path).await {
        Ok(0) => {}
        Ok(1) => return Some("Has uncommitted changes (1 file)".into()),
        Ok(count) => return Some(format!("Has uncommitted changes ({count} files)")),
        Err(error) => return Some(format!("Couldn't read its changes: {error:#}")),
    }
    match workspaces::unpushed_commits(path).await {
        Ok(Some(0)) => None,
        Ok(Some(1)) => Some("1 commit not pushed".into()),
        Ok(Some(count)) => Some(format!("{count} commits not pushed")),
        Ok(None) => Some("Not pushed: the repository has no remote".into()),
        Err(error) => Some(format!("Couldn't read its commits: {error:#}")),
    }
}

/// origin's default branch, fetched now. A failed fetch leaves the last one.
async fn fetch_default_branch(repo: &Path) -> Option<String> {
    let branch = workspaces::origin_default(repo).await?;
    if let Err(error) = workspaces::fetch_base(repo, &branch).await {
        log::warn!("cleanup couldn't fetch origin's {branch}: {error:#}");
    }
    Some(branch)
}

async fn branch_removal(
    rules: &CleanupRules,
    candidate: &Candidate,
    default: Option<&str>,
    head: Option<&str>,
    keep: Option<String>,
) -> std::result::Result<String, String> {
    let merged_rule = rules.merged && candidate.workspace.kind == WorkspaceKind::Worktree;
    let (Some(default), Some(head)) = (default, head) else {
        return Err(keep.unwrap_or_else(|| "The default branch is unknown".into()));
    };
    if workspaces::origin_branch_has(&candidate.repo, default, head).await {
        return Ok(if rules.unchanged {
            format!("No commits beyond {default}")
        } else {
            format!("Merged into {default}")
        });
    }
    Err(keep.unwrap_or_else(|| {
        if merged_rule {
            format!("Not merged into {default}")
        } else {
            format!("Has commits beyond {default}")
        }
    }))
}
