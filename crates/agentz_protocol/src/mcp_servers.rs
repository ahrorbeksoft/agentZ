//! agentZ's own MCP servers (design/accounts decisions.md §19): kept by the server in
//! `mcp-servers.json` in its data directory and given to every agent's sessions, as Zed gives its
//! context servers. agentZ's own `agentz` server is listed with them, to be switched off there.

use serde::{Deserialize, Serialize};

use crate::accounts::AgentAccount;

/// The name of the server agentZ gives every thread for its own tools.
pub const AGENTZ_SERVER_NAME: &str = "agentz";

/// The tools of the `agentz` server, by name, with their titles, so clients can name them
/// without asking the server. The server's tests keep it in step with its definitions.
pub const AGENTZ_TOOLS: [(&str, &str); 26] = [
    (
        "orchestrator_capabilities",
        "Get orchestration capabilities",
    ),
    ("agentz_thread_list", "List agentZ threads"),
    ("agentz_thread_read", "Read an agentZ thread"),
    ("agentz_thread_launch", "Launch an agentZ thread"),
    ("create_threads", "Create agentZ threads"),
    ("agentz_thread_send", "Send to an agentZ thread"),
    ("agentz_thread_wait", "Wait for an agentZ thread"),
    ("agentz_thread_interrupt", "Interrupt an agentZ thread"),
    ("agentz_thread_update", "Rename an agentZ thread"),
    ("agentz_thread_organize", "Organize an agentZ thread"),
    ("agentz_thread_diff", "Read an agentZ thread's changes"),
    ("delegate_task", "Delegate a child task"),
    ("agentz_workspace_status", "Get this thread's workspace"),
    ("agentz_workspace_list", "List branches and workspaces"),
    (
        "agentz_workspace_handoff",
        "Hand off this thread to a new workspace",
    ),
    (
        "agentz_workspace_sync",
        "Sync this pasture from the project",
    ),
    (
        "agentz_workspace_bring_back",
        "Bring this pasture's branch to the project",
    ),
    ("task_status", "Get delegated task status"),
    ("task_cancel", "Cancel delegated task"),
    ("agentz_terminal_list", "List agentZ terminals"),
    ("agentz_terminal_start", "Start an agentZ terminal"),
    ("agentz_terminal_send", "Type into an agentZ terminal"),
    ("agentz_terminal_read", "Read an agentZ terminal"),
    ("agentz_terminal_wait", "Wait for an agentZ terminal"),
    ("agentz_command_run", "Run a command"),
    ("agentz_project_add", "Add an agentZ project"),
];

/// Agents that ignore the MCP servers ACP gives them (design/accounts plan.md): their
/// sessions get none of agentZ's.
pub const IGNORES_MCP_SERVERS: [&str; 4] = ["autohand", "cline", "cortex-code", "pi-acp"];

/// One of agentZ's MCP servers, as Settings › MCP Servers lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServer {
    /// What agents know it by.
    pub name: String,
    /// Turned off with its switch, it stays listed but goes to no session.
    pub enabled: bool,
    pub transport: McpTransport,
    /// The accounts its accounts menu keeps it off, so an account added later gets it.
    #[serde(default)]
    pub kept_off: Vec<AgentAccount>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpTransport {
    /// A program each agent starts for its session (ACP's stdio server).
    Local {
        /// A path, or a program found on the `PATH` of the machine the agent runs on.
        command: String,
        args: Vec<String>,
        env: Vec<(String, String)>,
    },
    /// A server agents reach over HTTP (ACP's streamable HTTP server). Only agents that
    /// announce `mcpCapabilities.http` take it.
    Remote {
        url: String,
        headers: Vec<(String, String)>,
    },
    /// agentZ's own tools (`agentz-server mcp-bridge`), named [`AGENTZ_SERVER_NAME`]. It's
    /// always listed, first, and can be switched off and kept off accounts, but not configured
    /// or removed.
    Agentz,
}

impl McpServer {
    /// agentZ's own server as it's first listed: on for every account.
    pub fn agentz() -> Self {
        Self {
            name: AGENTZ_SERVER_NAME.into(),
            enabled: true,
            transport: McpTransport::Agentz,
            kept_off: Vec::new(),
        }
    }

    pub fn is_agentz(&self) -> bool {
        matches!(self.transport, McpTransport::Agentz)
    }

    /// Whether a session of the account gets it.
    pub fn reaches(&self, account: &AgentAccount) -> bool {
        self.enabled
            && !IGNORES_MCP_SERVERS.contains(&account.agent_id.0.as_ref())
            && !self.kept_off.contains(account)
    }

    pub fn is_remote(&self) -> bool {
        matches!(self.transport, McpTransport::Remote { .. })
    }

    /// The command line or the URL, as its row shows it.
    pub fn detail(&self) -> String {
        match &self.transport {
            McpTransport::Local { command, args, .. } => std::iter::once(command)
                .chain(args)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(" "),
            McpTransport::Remote { url, .. } => url.clone(),
            McpTransport::Agentz => {
                "agentZ's own tools: threads, tasks, workspaces, terminals and commands.".into()
            }
        }
    }

    /// Zed's checks of its form, in its words. `others` are the names of the other servers.
    pub fn validate<'a>(&self, others: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Server name is required.".into());
        }
        if name == AGENTZ_SERVER_NAME || others.into_iter().any(|other| other == name) {
            return Err(format!("A server named \"{name}\" already exists."));
        }
        match &self.transport {
            McpTransport::Local { command, env, .. } => {
                if command.trim().is_empty() {
                    return Err("Command is required.".into());
                }
                no_duplicates(env, "environment variable")
            }
            McpTransport::Remote { url, headers } => {
                if url.trim().is_empty() {
                    return Err("URL is required.".into());
                }
                if let Err(error) = url::Url::parse(url) {
                    return Err(format!("Invalid URL: {error}"));
                }
                no_duplicates(headers, "header")
            }
            McpTransport::Agentz => Err("agentZ's own server can't be configured.".into()),
        }
    }
}

fn no_duplicates(pairs: &[(String, String)], label: &str) -> Result<(), String> {
    for (index, (key, _)) in pairs.iter().enumerate() {
        if pairs[..index].iter().any(|(other, _)| other == key) {
            return Err(format!("Duplicate {label} \"{key}\"."));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(name: &str, command: &str, env: &[(&str, &str)]) -> McpServer {
        McpServer {
            name: name.into(),
            enabled: true,
            transport: McpTransport::Local {
                command: command.into(),
                args: vec!["-y".into(), "server-github".into()],
                env: env
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
            },
            kept_off: Vec::new(),
        }
    }

    fn remote(url: &str, headers: &[(&str, &str)]) -> McpServer {
        McpServer {
            name: "docs".into(),
            enabled: true,
            transport: McpTransport::Remote {
                url: url.into(),
                headers: headers
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
            },
            kept_off: Vec::new(),
        }
    }

    #[test]
    fn checks_the_form_as_zed_does() {
        assert_eq!(local("github", "npx", &[]).validate([]), Ok(()));
        assert_eq!(local("github", "npx", &[]).detail(), "npx -y server-github");
        assert_eq!(
            local(" ", "npx", &[]).validate([]),
            Err("Server name is required.".into())
        );
        assert_eq!(
            local("github", "  ", &[]).validate([]),
            Err("Command is required.".into())
        );
        assert_eq!(
            local("github", "npx", &[]).validate(["linear", "github"]),
            Err("A server named \"github\" already exists.".into())
        );
        assert_eq!(
            local("agentz", "npx", &[]).validate([]),
            Err("A server named \"agentz\" already exists.".into())
        );
        assert_eq!(
            local("github", "npx", &[("FOO", "1"), ("FOO", "2")]).validate([]),
            Err("Duplicate environment variable \"FOO\".".into())
        );
        assert_eq!(
            remote("https://mcp.linear.app/mcp", &[]).validate([]),
            Ok(())
        );
        assert_eq!(remote("", &[]).validate([]), Err("URL is required.".into()));
        assert!(
            remote("mcp.linear.app", &[])
                .validate([])
                .is_err_and(|error| error.starts_with("Invalid URL"))
        );
        assert_eq!(
            remote(
                "https://mcp.linear.app/mcp",
                &[("Authorization", "a"), ("Authorization", "b")]
            )
            .validate([]),
            Err("Duplicate header \"Authorization\".".into())
        );
    }
}
