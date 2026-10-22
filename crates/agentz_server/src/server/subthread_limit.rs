//! How many delegated tasks a thread runs at once (`Request::SetSubthreadLimit`); more wait
//! their turn ([`projects::Task::queued`]). The setting is the app's; the server keeps its own
//! copy so tasks delegated while the app is closed keep to it.

use std::path::{Path, PathBuf};

use agentz_protocol::DEFAULT_SUBTHREAD_LIMIT;
use anyhow::Result;

use super::Server;
use crate::agent_settings::{read_json, write_json};

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("subthread-limit.json")
}

pub(crate) fn load(data_dir: &Path) -> Result<u32> {
    Ok(read_json::<u32>(&path(data_dir))?
        .unwrap_or(DEFAULT_SUBTHREAD_LIMIT)
        .max(1))
}

impl Server {
    pub(super) fn set_subthread_limit(&mut self, limit: u32) -> Result<()> {
        let limit = limit.max(1);
        if limit == self.subthread_limit {
            return Ok(());
        }
        write_json(&path(&self.data_dir), &limit)?;
        self.subthread_limit = limit;
        // A higher limit lets queued tasks start now.
        self.start_queued_tasks();
        Ok(())
    }
}
