//! Starting this Mac's server when the user logs in: a launchd agent that runs
//! `agentz-server start`, which returns once the server listens. Nothing restarts it after
//! that, so a server the user stops stays stopped until the next login. Remote servers start
//! when the app connects.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

const LABEL: &str = "dev.agentz.server";
const DATA_DIR_ENV_VAR: &str = "AGENTZ_DATA_DIR";

/// The launchd agent's label and contents. A scratch data directory gets its own agent, so
/// development runs don't replace the real one.
struct LaunchAgent {
    label: String,
    plist: String,
}

impl LaunchAgent {
    fn new(executable: &Path, data_dir: Option<&Path>) -> Self {
        let label = match data_dir {
            Some(data_dir) => format!("{LABEL}.{}", label_suffix(data_dir)),
            None => LABEL.to_string(),
        };
        let environment = data_dir
            .map(|data_dir| {
                format!(
                    "\t<key>EnvironmentVariables</key>\n\t<dict>\n\t\t<key>{DATA_DIR_ENV_VAR}</key>\n\t\t<string>{}</string>\n\t</dict>\n",
                    xml_escape(&data_dir.to_string_lossy())
                )
            })
            .unwrap_or_default();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{executable}</string>
		<string>start</string>
	</array>
{environment}	<key>RunAtLoad</key>
	<true/>
	<key>AbandonProcessGroup</key>
	<true/>
	<key>ProcessType</key>
	<string>Background</string>
</dict>
</plist>
"#,
            label = xml_escape(&label),
            executable = xml_escape(&executable.to_string_lossy()),
        );
        Self { label, plist }
    }

    fn path(&self) -> Result<PathBuf> {
        let home = dirs::home_dir().context("finding the home folder")?;
        Ok(home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{}.plist", self.label)))
    }
}

fn current() -> Result<LaunchAgent> {
    let executable = crate::server_client::server_binary()?;
    let data_dir = std::env::var_os(DATA_DIR_ENV_VAR).map(PathBuf::from);
    Ok(LaunchAgent::new(&executable, data_dir.as_deref()))
}

pub fn is_enabled() -> bool {
    current()
        .and_then(|agent| agent.path())
        .is_ok_and(|path| path.is_file())
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let agent = current()?;
    let path = agent.path()?;
    if enabled {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        std::fs::write(&path, agent.plist).with_context(|| format!("writing {}", path.display()))
    } else {
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).with_context(|| format!("removing {}", path.display())),
        }
    }
}

/// Points an enabled launch agent at the server this app runs, which moves when the app does.
pub fn refresh() -> Result<()> {
    let agent = current()?;
    let path = agent.path()?;
    match std::fs::read_to_string(&path) {
        Ok(contents) if contents != agent.plist => std::fs::write(&path, agent.plist)
            .with_context(|| format!("writing {}", path.display())),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

fn label_suffix(data_dir: &Path) -> String {
    let suffix = data_dir
        .to_string_lossy()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    suffix.trim_matches('-').to_string()
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_agent_runs_the_server_at_load() {
        let agent = LaunchAgent::new(
            Path::new("/Applications/agentZ & Co.app/agentz-server"),
            None,
        );
        assert_eq!(agent.label, "dev.agentz.server");
        assert!(agent.plist.contains(
            "\t\t<string>/Applications/agentZ &amp; Co.app/agentz-server</string>\n\t\t<string>start</string>"
        ));
        assert!(agent.plist.contains("<key>RunAtLoad</key>\n\t<true/>"));
        assert!(!agent.plist.contains("KeepAlive"));
        assert!(!agent.plist.contains(DATA_DIR_ENV_VAR));
    }

    #[test]
    #[allow(
        clippy::disallowed_methods,
        reason = "a test, with nothing else to block"
    )]
    fn launch_agent_is_a_valid_property_list() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("agent.plist");
        let agent = LaunchAgent::new(
            Path::new("/Applications/agentZ <dev>.app/agentz-server"),
            Some(Path::new("/tmp/a&b")),
        );
        std::fs::write(&path, agent.plist).expect("writes the plist");
        let output = std::process::Command::new("/usr/bin/plutil")
            .arg("-lint")
            .arg(&path)
            .output()
            .expect("runs plutil");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    #[test]
    fn scratch_data_directories_get_their_own_launch_agent() {
        let agent = LaunchAgent::new(
            Path::new("/bin/agentz-server"),
            Some(Path::new("/tmp/azshot/Data")),
        );
        assert_eq!(agent.label, "dev.agentz.server.tmp-azshot-data");
        assert!(agent.plist.contains(
            "<key>AGENTZ_DATA_DIR</key>\n\t\t<string>/tmp/azshot/Data</string>\n\t</dict>\n\t<key>RunAtLoad</key>"
        ));
    }
}
