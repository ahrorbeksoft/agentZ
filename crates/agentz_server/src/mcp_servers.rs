//! agentZ's own MCP servers (design/accounts decisions.md §19), kept in `mcp-servers.json` in
//! the data directory. Every session's agent gets the enabled ones beside agentZ's `agentz`
//! server, as Zed gives agents its context servers; the agent's thread leaves out remote ones
//! its agent doesn't take. `agentz` is kept in the list too, for its switch and accounts menu.

use std::path::{Path, PathBuf};

use agent_client_protocol::schema::v1 as acp;
use agentz_protocol::accounts::AgentAccount;
use agentz_protocol::mcp_servers::{AGENTZ_SERVER_NAME, McpServer, McpTransport};
use anyhow::{Result, anyhow};

use crate::agent_settings::{read_json, write_json};

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("mcp-servers.json")
}

/// The servers, with agentZ's own first, on until its switch turns it off.
pub fn load(data_dir: &Path) -> Result<Vec<McpServer>> {
    let mut servers: Vec<McpServer> = read_json(&path(data_dir))?.unwrap_or_default();
    if !servers.iter().any(McpServer::is_agentz) {
        servers.insert(0, McpServer::agentz());
    }
    Ok(servers)
}

fn save(data_dir: &Path, servers: &[McpServer]) -> Result<()> {
    write_json(&path(data_dir), &servers)
}

/// Adds `server`, or puts it in place of the one named `replacing`, and saves them.
pub fn save_server(
    data_dir: &Path,
    servers: &mut Vec<McpServer>,
    replacing: Option<&str>,
    mut server: McpServer,
) -> Result<()> {
    server.name = server.name.trim().to_string();
    if replacing == Some(AGENTZ_SERVER_NAME) {
        return Err(anyhow!("agentZ's own server can't be configured."));
    }
    let index = match replacing {
        Some(name) => Some(
            servers
                .iter()
                .position(|existing| existing.name == name)
                .ok_or_else(|| anyhow!("There's no MCP server named \"{name}\"."))?,
        ),
        None => None,
    };
    let others = servers
        .iter()
        .enumerate()
        .filter(|(other, _)| Some(*other) != index)
        .map(|(_, other)| other.name.as_str());
    server.validate(others).map_err(|error| anyhow!(error))?;
    match index {
        Some(index) => servers[index] = server,
        None => servers.push(server),
    }
    save(data_dir, servers)
}

pub fn delete_server(data_dir: &Path, servers: &mut Vec<McpServer>, name: &str) -> Result<()> {
    if name == AGENTZ_SERVER_NAME {
        return Err(anyhow!("agentZ's own server can't be removed."));
    }
    let count = servers.len();
    servers.retain(|server| server.name != name);
    if servers.len() == count {
        return Err(anyhow!("There's no MCP server named \"{name}\"."));
    }
    save(data_dir, servers)
}

/// Changes the server named `name` (its switch, its accounts menu) and saves them.
pub fn change(
    data_dir: &Path,
    servers: &mut [McpServer],
    name: &str,
    change: impl FnOnce(&mut McpServer),
) -> Result<()> {
    let server = servers
        .iter_mut()
        .find(|server| server.name == name)
        .ok_or_else(|| anyhow!("There's no MCP server named \"{name}\"."))?;
    change(server);
    save(data_dir, servers)
}

/// Whether a session on `account` gets agentZ's own server.
pub fn gives_agentz(servers: &[McpServer], account: &AgentAccount) -> bool {
    servers
        .iter()
        .find(|server| server.is_agentz())
        .is_none_or(|server| server.reaches(account))
}

/// What a session on `account` is given of the user's servers. agentZ's own is added by the
/// thread, with its credential ([`gives_agentz`]).
pub fn for_session(servers: &[McpServer], account: &AgentAccount) -> Vec<acp::McpServer> {
    servers
        .iter()
        .filter(|server| server.reaches(account))
        .filter_map(|server| match &server.transport {
            McpTransport::Local { command, args, env } => Some(acp::McpServer::Stdio(
                acp::McpServerStdio::new(server.name.clone(), find_program(command))
                    .args(args.clone())
                    .env(
                        env.iter()
                            .map(|(name, value)| acp::EnvVariable::new(name, value))
                            .collect(),
                    ),
            )),
            McpTransport::Remote { url, headers } => Some(acp::McpServer::Http(
                acp::McpServerHttp::new(server.name.clone(), url.clone()).headers(
                    headers
                        .iter()
                        .map(|(name, value)| acp::HttpHeader::new(name, value))
                        .collect(),
                ),
            )),
            McpTransport::Agentz => None,
        })
        .collect()
}

/// ACP asks for the program's absolute path. A bare name is looked up on the server's `PATH`,
/// the user's once the login shell's environment is in; one that isn't there goes as it is,
/// for the agent to find on its own `PATH`.
fn find_program(command: &str) -> PathBuf {
    let command = command.trim();
    if command.contains('/') {
        return PathBuf::from(command);
    }
    std::env::var_os("PATH")
        .and_then(|path| {
            std::env::split_paths(&path)
                .map(|folder| folder.join(command))
                .find(|candidate| candidate.is_file())
        })
        .unwrap_or_else(|| PathBuf::from(command))
}

#[cfg(test)]
mod tests {
    use agentz_protocol::accounts::AccountId;
    use agentz_protocol::agents::AgentId;

    use super::*;

    fn server(name: &str, transport: McpTransport) -> McpServer {
        McpServer {
            name: name.into(),
            enabled: true,
            transport,
            kept_off: Vec::new(),
        }
    }

    #[test]
    fn saves_renames_and_deletes_servers() -> Result<()> {
        let data_dir = tempfile::tempdir()?;
        let mut servers = load(data_dir.path())?;
        assert_eq!(servers, [McpServer::agentz()]);
        let local = |command: &str| McpTransport::Local {
            command: command.into(),
            args: vec!["-y".into(), "server-github".into()],
            env: vec![("GITHUB_TOKEN".into(), "secret".into())],
        };
        save_server(
            data_dir.path(),
            &mut servers,
            None,
            server(" github ", local("npx")),
        )?;
        save_server(
            data_dir.path(),
            &mut servers,
            None,
            server(
                "linear",
                McpTransport::Remote {
                    url: "https://mcp.linear.app/mcp".into(),
                    headers: Vec::new(),
                },
            ),
        )?;
        let error = save_server(
            data_dir.path(),
            &mut servers,
            None,
            server("github", local("uvx")),
        )
        .expect_err("a second github");
        assert_eq!(
            error.to_string(),
            "A server named \"github\" already exists."
        );
        // Saved under its own name, it isn't a duplicate of itself.
        save_server(
            data_dir.path(),
            &mut servers,
            Some("github"),
            server("gh", local("/usr/bin/env")),
        )?;
        change(data_dir.path(), &mut servers, "linear", |server| {
            server.enabled = false
        })?;
        assert_eq!(load(data_dir.path())?, servers);
        assert_eq!(
            servers
                .iter()
                .map(|server| &server.name)
                .collect::<Vec<_>>(),
            ["agentz", "gh", "linear"]
        );

        let account = |agent_id: &str, account: Option<u64>| AgentAccount {
            agent_id: AgentId::new(agent_id.to_string()),
            account: account.map(AccountId),
        };
        let given = for_session(&servers, &account("claude", None));
        assert_eq!(
            given,
            [acp::McpServer::Stdio(
                acp::McpServerStdio::new("gh", "/usr/bin/env")
                    .args(vec!["-y".into(), "server-github".into()])
                    .env(vec![acp::EnvVariable::new("GITHUB_TOKEN", "secret")])
            )]
        );
        assert!(for_session(&servers, &account("cline", None)).is_empty());

        // Kept off one account, the others still get it.
        let kept_off = vec![account("claude", Some(2))];
        change(data_dir.path(), &mut servers, "gh", |server| {
            server.kept_off = kept_off.clone()
        })?;
        assert_eq!(load(data_dir.path())?[1].kept_off, kept_off);
        assert!(for_session(&servers, &account("claude", Some(2))).is_empty());
        assert_eq!(for_session(&servers, &account("claude", Some(3))), given);

        delete_server(data_dir.path(), &mut servers, "gh")?;
        assert_eq!(load(data_dir.path())?.len(), 2);
        Ok(())
    }

    #[test]
    fn agentz_is_switched_but_not_configured_or_removed() -> Result<()> {
        let data_dir = tempfile::tempdir()?;
        let mut servers = load(data_dir.path())?;
        let claude = AgentAccount {
            agent_id: AgentId::new("claude".to_string()),
            account: None,
        };
        assert!(gives_agentz(&servers, &claude));
        assert!(for_session(&servers, &claude).is_empty());

        let replaced = save_server(
            data_dir.path(),
            &mut servers,
            Some(AGENTZ_SERVER_NAME),
            server("other", McpTransport::Agentz),
        );
        assert_eq!(
            replaced.map_err(|error| error.to_string()),
            Err("agentZ's own server can't be configured.".into())
        );
        let deleted = delete_server(data_dir.path(), &mut servers, AGENTZ_SERVER_NAME);
        assert_eq!(
            deleted.map_err(|error| error.to_string()),
            Err("agentZ's own server can't be removed.".into())
        );

        change(
            data_dir.path(),
            &mut servers,
            AGENTZ_SERVER_NAME,
            |server| server.enabled = false,
        )?;
        let servers = load(data_dir.path())?;
        assert_eq!(servers.len(), 1);
        assert!(!gives_agentz(&servers, &claude));
        Ok(())
    }
}
