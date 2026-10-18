//! Archiving threads nobody has touched for a while (`Request::SetAutoArchive`), going by their
//! last activity. The setting is the app's; the server keeps its own copy so threads archive
//! while the app is closed.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;
use projects::ThreadId;

use super::Server;
use crate::agent_settings::{read_json, write_json};

const SECONDS_PER_DAY: u64 = 24 * 60 * 60;

/// How often threads are checked. Days are the finest the setting goes by.
pub(super) const AUTO_ARCHIVE_INTERVAL: Duration = Duration::from_secs(10 * 60);

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("auto-archive.json")
}

pub(crate) fn load(data_dir: &Path) -> Result<Option<u32>> {
    Ok(read_json::<Option<u32>>(&path(data_dir))?.flatten())
}

impl Server {
    pub(super) fn set_auto_archive(&mut self, days: Option<u32>) -> Result<()> {
        let days = days.filter(|days| *days > 0);
        if days == self.auto_archive_days {
            return Ok(());
        }
        write_json(&path(&self.data_dir), &days)?;
        self.auto_archive_days = days;
        self.archive_stale_threads();
        Ok(())
    }

    /// Archives the threads whose last activity is older than the setting, except those a client
    /// has open and those whose agent has something to do: a thread in use isn't left alone.
    pub(super) fn archive_stale_threads(&mut self) {
        let Some(days) = self.auto_archive_days else {
            return;
        };
        let Some(cutoff) =
            SystemTime::now().checked_sub(Duration::from_secs(u64::from(days) * SECONDS_PER_DAY))
        else {
            return;
        };
        let watched = self.watched_threads();
        let stale: Vec<ThreadId> = self
            .projects
            .stale_threads(cutoff)
            .into_iter()
            .filter(|thread_id| !watched.contains(thread_id) && !self.needs_agent(*thread_id))
            .collect();
        if stale.is_empty() {
            return;
        }
        for thread_id in stale {
            log::info!(
                "archiving thread {}, left alone for {days} days",
                thread_id.0
            );
            self.projects.archive_thread(thread_id);
        }
        self.thread_archived();
    }
}
