//! Where the app keeps its data on disk.

use std::path::PathBuf;
use std::sync::OnceLock;

const DATA_DIR_ENV_VAR: &str = "AGENTZ_DATA_DIR";

/// The app's data directory, `~/Library/Application Support/agentZ` on macOS.
///
/// `AGENTZ_DATA_DIR` overrides it, so development runs and tests can use a scratch directory
/// instead of the real one.
pub fn data_dir() -> &'static PathBuf {
    static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
    DATA_DIR.get_or_init(|| {
        if let Some(dir) = std::env::var_os(DATA_DIR_ENV_VAR) {
            return PathBuf::from(dir);
        }
        dirs::data_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(std::env::temp_dir)
            .join("agentZ")
    })
}

pub fn state_file() -> PathBuf {
    data_dir().join("state.json")
}

/// App-wide preferences, such as the theme.
pub fn settings_file() -> PathBuf {
    data_dir().join("settings.json")
}

/// Agents installed from the ACP registry, and the cached registry index.
pub fn registry_dir() -> PathBuf {
    data_dir().join("agents").join("registry")
}

/// Where `agentz-server` listens.
pub fn server_socket() -> PathBuf {
    data_dir().join("server.sock")
}

/// The running server's process id.
pub fn server_pid_file() -> PathBuf {
    data_dir().join("server.pid")
}

/// The server's log, when it was started in the background.
pub fn server_log_file() -> PathBuf {
    data_dir().join("logs").join("server.log")
}
