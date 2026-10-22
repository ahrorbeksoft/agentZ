//! Whether agents are asked for ACP v2 before v1 (`Request::SetAcpV2`), an experimental
//! setting that's off until most agents' v2 is stable. The setting is the app's; the server
//! keeps its own copy so agents started while the app is closed keep to it.

use std::path::{Path, PathBuf};

use anyhow::Result;

use super::Server;
use crate::agent_settings::{read_json, write_json};

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("acp-v2.json")
}

pub(crate) fn load(data_dir: &Path) -> Result<bool> {
    Ok(read_json::<bool>(&path(data_dir))?.unwrap_or(false))
}

impl Server {
    pub(super) fn set_acp_v2(&mut self, acp_v2: bool) -> Result<()> {
        if acp_v2 == self.acp_v2 {
            return Ok(());
        }
        write_json(&path(&self.data_dir), &acp_v2)?;
        self.acp_v2 = acp_v2;
        Ok(())
    }
}
