//! How agentZ tells whether an account is logged in.

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

/// A status or usage command still running by then is taken to have hung.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginCheck {
    /// An empty session opens only while logged in: `session/new` fails with "authentication
    /// required" otherwise, as most agents' does.
    #[default]
    Session,
    /// The agent's sessions open while it's logged out too (Claude's, Devin's), so its own
    /// status command tells.
    Command(StatusCommand),
}

/// A command that says whether the agent is logged in, run with the account's environment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusCommand {
    /// A path, or a name looked up on `PATH`. Without one, the agent's own program runs, with
    /// `args` in place of its arguments, or after them with `after_agent_args`.
    pub program: Option<String>,
    pub args: Vec<String>,
    /// Whether `args` follow the agent's own arguments, as they must where the agent runs
    /// through Node, its script first (Claude's adapter, whose `--cli` runs Claude Code).
    pub after_agent_args: bool,
    /// A JSON pointer to where its output says `true` while it's logged in (`/loggedIn`).
    /// Without one, it exits with 0 only while logged in.
    pub logged_in: Option<String>,
}

impl StatusCommand {
    /// Runs it with the environment `agent`, the account's agent command, has.
    pub async fn run(&self, agent: AgentCommand) -> Result<bool> {
        let args = if self.program.is_none() && self.after_agent_args {
            agent.args.iter().chain(&self.args).cloned().collect()
        } else {
            self.args.clone()
        };
        let output = run_with_account_env(self.program.as_deref(), &args, agent).await?;
        self.logged_in(&output)
    }

    fn logged_in(&self, output: &Output) -> Result<bool> {
        let Some(pointer) = &self.logged_in else {
            return Ok(output.status.success());
        };
        let json: serde_json::Value =
            serde_json::from_slice(&output.stdout).context("its output isn't JSON")?;
        match json.pointer(pointer) {
            Some(serde_json::Value::Bool(logged_in)) => Ok(*logged_in),
            found => bail!("its output has {found:?} at {pointer}"),
        }
    }
}

/// Runs `program` (else the agent's own) with `args` and the environment `agent`, the account's
/// agent command, has.
pub(super) async fn run_with_account_env(
    program: Option<&str>,
    args: &[String],
    agent: AgentCommand,
) -> Result<Output> {
    let program = program.map(PathBuf::from).unwrap_or(agent.path.clone());
    let mut command = account_command(&program, args, &agent);
    command
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::null());
    tokio::time::timeout(COMMAND_TIMEOUT, command.output())
        .await
        .with_context(|| format!("{} didn't finish", program.display()))?
        .with_context(|| format!("running {}", program.display()))
}

/// `program` with `args` and the environment `agent`, the account's agent command, has, its
/// output read and its errors dropped. It's killed if it's dropped before it ends.
pub(super) fn account_command(
    program: &Path,
    args: &[String],
    agent: &AgentCommand,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(program);
    command
        .args(args)
        .envs(&agent.env)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    for variable in &agent.env_remove {
        command.env_remove(variable);
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(script: &str) -> StatusCommand {
        StatusCommand {
            program: Some("/bin/sh".into()),
            args: vec!["-c".into(), script.into()],
            after_agent_args: false,
            logged_in: None,
        }
    }

    #[tokio::test]
    async fn status_commands_say_whether_the_account_is_logged_in() {
        let agent = AgentCommand {
            env: [("ACCOUNT".to_string(), "work".to_string())]
                .into_iter()
                .collect(),
            ..AgentCommand::default()
        };
        assert!(shell("true").run(agent.clone()).await.expect("run"));
        assert!(!shell("exit 1").run(agent.clone()).await.expect("run"));

        let json = StatusCommand {
            logged_in: Some("/account/loggedIn".into()),
            ..shell(
                r#"echo "{\"account\": {\"loggedIn\": $([ "$ACCOUNT" = work ] && echo true || echo false)}}"; exit 1"#,
            )
        };
        assert!(json.run(agent.clone()).await.expect("run"));
        let elsewhere = AgentCommand {
            env_remove: vec!["ACCOUNT".into()],
            ..agent.clone()
        };
        assert!(!json.run(elsewhere).await.expect("run"));

        let not_json = StatusCommand {
            logged_in: Some("/loggedIn".into()),
            ..shell("echo Logged in")
        };
        assert!(not_json.run(agent.clone()).await.is_err());
        let missing = StatusCommand {
            logged_in: Some("/loggedIn".into()),
            ..shell("echo '{}'")
        };
        assert!(missing.run(agent.clone()).await.is_err());

        // After the agent's own arguments, as its script's.
        let script = AgentCommand {
            path: "/bin/sh".into(),
            args: vec![
                "-c".into(),
                r#"[ "$1 $2" = "--cli auth" ] && [ "$ACCOUNT" = work ]"#.into(),
                "sh".into(),
            ],
            ..agent
        };
        let after = StatusCommand {
            program: None,
            args: vec!["--cli".into(), "auth".into()],
            after_agent_args: true,
            logged_in: None,
        };
        assert!(after.run(script.clone()).await.expect("run"));
        let in_place = StatusCommand {
            after_agent_args: false,
            ..after
        };
        assert!(!in_place.run(script).await.expect("run"));
    }

    #[test]
    fn descriptions_name_their_check() {
        let check: LoginCheck = serde_json::from_str(
            r#"{"command": {"args": ["auth", "status", "--json"], "logged_in": "/loggedIn"}}"#,
        )
        .expect("parse");
        let LoginCheck::Command(command) = check else {
            panic!("expected a status command");
        };
        assert_eq!(command.program, None);
        assert_eq!(command.args, ["auth", "status", "--json"]);
        let check: LoginCheck = serde_json::from_str(r#""session""#).expect("parse");
        assert_eq!(check, LoginCheck::Session);
    }
}
