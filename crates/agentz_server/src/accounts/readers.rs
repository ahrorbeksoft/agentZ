//! How agentZ reads an account's identity and limits (plan.md › Agent descriptions, the
//! readers' kinds). Each kind comes with the first agent that needs it.

use agentz_protocol::accounts::AccountStatus;
use agentz_protocol::agents::AgentCommand;
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

use super::login_checks::run_with_account_env;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reader {
    /// A command that prints a [`Read`] as JSON, run with the account's environment, as the
    /// mock agent's `--usage` does.
    Command(ReaderCommand),
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReaderCommand {
    /// A path, or a name looked up on `PATH`. Without one, the agent's own program runs, with
    /// `args` in place of its arguments.
    pub program: Option<String>,
    pub args: Vec<String>,
}

/// What a reader found.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Read {
    /// Whether the account is logged in, when the reader can tell.
    pub logged_in: Option<bool>,
    #[serde(flatten)]
    pub status: AccountStatus,
}

impl Reader {
    /// Reads the account whose agent command is `agent`.
    pub async fn read(&self, agent: AgentCommand) -> Result<Read> {
        match self {
            Reader::Command(command) => {
                let output =
                    run_with_account_env(command.program.as_deref(), &command.args, agent).await?;
                if !output.status.success() {
                    bail!("it exited with {}", output.status);
                }
                serde_json::from_slice(&output.stdout).context("its output isn't a read")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::*;

    fn shell(script: &str) -> Reader {
        Reader::Command(ReaderCommand {
            program: Some("/bin/sh".into()),
            args: vec!["-c".into(), script.into()],
        })
    }

    #[tokio::test]
    async fn commands_print_what_they_read() {
        let agent = AgentCommand {
            env: [("PLAN".to_string(), "Pro".to_string())]
                .into_iter()
                .collect(),
            ..AgentCommand::default()
        };
        let read = shell(
            r#"echo "{\"logged_in\": true, \"email\": \"work@example.com\", \"plan\": \"$PLAN\",
                \"windows\": [{\"label\": \"5-hour\", \"used_percent\": 40,
                \"resets_at\": {\"secs_since_epoch\": 60, \"nanos_since_epoch\": 0}}]}""#,
        )
        .read(agent.clone())
        .await
        .expect("read");
        assert_eq!(read.logged_in, Some(true));
        assert_eq!(read.status.email.as_deref(), Some("work@example.com"));
        assert_eq!(read.status.plan.as_deref(), Some("Pro"));
        let [window] = &read.status.windows[..] else {
            panic!("expected a window");
        };
        assert_eq!(window.label, "5-hour");
        assert_eq!(window.used_percent, 40.0);
        assert_eq!(
            window.resets_at,
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(60))
        );

        let logged_out = shell(r#"echo '{"logged_in": false}'"#)
            .read(agent.clone())
            .await
            .expect("read");
        assert_eq!(logged_out.logged_in, Some(false));
        assert!(logged_out.status.windows.is_empty());

        assert!(
            shell("echo '{}'; exit 1")
                .read(agent.clone())
                .await
                .is_err()
        );
        assert!(shell("echo 5-hour: 40%").read(agent).await.is_err());
    }
}
