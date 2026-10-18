//! Cloud providers' CLIs (design/cloud): finding each one, and running it to list, make, start
//! and stop machines and to say how SSH reaches them. The user installs each CLI and signs in
//! with it; agentZ only runs it, with the login shell's environment the server already has, and
//! never reads its login.

mod instabox;
mod proton;
mod ssh_config;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use agentz_protocol::cloud::{
    CloudMachine, CloudUsage, CreateMachine, Lifecycle, Provider, ProviderStatus, SshOptions,
};
use anyhow::{Context as _, Result, anyhow};

/// Reading: who's signed in, the machines, usage.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Making or changing a machine. Both CLIs wait on their own operations, Proton's for up to 20
/// minutes.
const CHANGE_TIMEOUT: Duration = Duration::from_secs(21 * 60);

/// A provider's CLI.
#[derive(Clone, Debug)]
pub struct Cli {
    provider: Provider,
    program: PathBuf,
    /// The `PATH` it's looked for on and runs with; `None` is the server's own.
    path: Option<OsString>,
}

impl Cli {
    /// The provider's CLI, found on `PATH`.
    pub fn new(provider: Provider) -> Self {
        Self::with_program(provider, provider.description().program)
    }

    pub fn with_program(provider: Provider, program: impl Into<PathBuf>) -> Self {
        Self {
            provider,
            program: program.into(),
            path: None,
        }
    }

    /// The provider's CLI, found on `path` rather than the server's own `PATH`.
    pub fn on_path(provider: Provider, path: Option<OsString>) -> Self {
        Self {
            path,
            ..Self::new(provider)
        }
    }

    pub fn provider(&self) -> Provider {
        self.provider
    }

    /// Whether the CLI is installed, and who it's signed in as.
    pub async fn status(&self) -> ProviderStatus {
        let version = match self.output(&["--version"], READ_TIMEOUT).await {
            Err(error) if is_not_found(&error) => return ProviderStatus::NotInstalled,
            Err(error) => {
                log::warn!("{} --version failed: {error:#}", self.name());
                String::new()
            }
            Ok(output) => version_of(&output.stdout, self.provider.description().program),
        };
        match self.run(&["whoami"], READ_TIMEOUT).await {
            Ok(stdout) => match first_line(&stdout) {
                Some(account) => ProviderStatus::SignedIn {
                    version,
                    account: account.to_string(),
                },
                None => ProviderStatus::SignedOut { version },
            },
            Err(error) if is_not_found(&error) => ProviderStatus::NotInstalled,
            Err(_) => ProviderStatus::SignedOut { version },
        }
    }

    pub async fn machines(&self) -> Result<Vec<CloudMachine>> {
        let (args, parse): (&[&str], fn(&str) -> Result<Vec<CloudMachine>>) = match self.provider {
            Provider::Instabox => (&["ls", "--json"], instabox::parse_machines),
            Provider::Proton => (&["list", "--json"], proton::parse_machines),
        };
        let stdout = self.run(args, READ_TIMEOUT).await?;
        parse(&stdout).with_context(|| format!("reading {}'s machines", self.name()))
    }

    /// Makes a machine and waits until it's made.
    pub async fn create(&self, machine: &CreateMachine) -> Result<()> {
        let args = match self.provider {
            Provider::Instabox => instabox::create_args(machine),
            Provider::Proton => proton::create_args(machine)?,
        };
        self.run(&args, CHANGE_TIMEOUT).await.map(|_| ())
    }

    /// Changes a machine, by its id, and waits until it's done.
    pub async fn change(&self, machine: &str, change: Lifecycle) -> Result<()> {
        let args = match self.provider {
            Provider::Instabox => instabox::change_args(machine, change),
            Provider::Proton => proton::change_args(machine, change)?,
        };
        self.run(&args, CHANGE_TIMEOUT).await.map(|_| ())
    }

    /// How SSH reaches a running machine, from the CLI's `ssh-config`. Instabox's certificate
    /// lasts a short time, so it's renewed first.
    pub async fn ssh_options(&self, machine: &str) -> Result<SshOptions> {
        if self.provider == Provider::Instabox {
            self.run(&["ssh-cert", machine], READ_TIMEOUT)
                .await
                .context("renewing the SSH certificate")?;
        }
        let stdout = self.run(&["ssh-config", machine], READ_TIMEOUT).await?;
        ssh_config::parse(&stdout)
    }

    pub async fn usage(&self) -> Result<CloudUsage> {
        let (args, parse): (&[&str], fn(&str) -> Result<CloudUsage>) = match self.provider {
            Provider::Instabox => (&["usage", "--json"], instabox::parse_usage),
            Provider::Proton => (&["balance"], proton::parse_balance),
        };
        let stdout = self.run(args, READ_TIMEOUT).await?;
        parse(&stdout).with_context(|| format!("reading {}'s usage", self.name()))
    }

    /// Runs the CLI with `args`, and gives what it printed. A command that fails gives what it
    /// said on stderr.
    async fn run(&self, args: &[impl AsRef<str>], timeout: Duration) -> Result<String> {
        let output = self.output(args, timeout).await?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let message = last_line(&stderr)
                .map(|line| {
                    line.strip_prefix(&format!("{}: ", self.provider.description().program))
                        .unwrap_or(line)
                        .to_string()
                })
                .unwrap_or_else(|| format!("{} exited with {}", self.name(), output.status));
            return Err(anyhow!(message));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    async fn output(
        &self,
        args: &[impl AsRef<str>],
        timeout: Duration,
    ) -> Result<std::process::Output> {
        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(args.iter().map(AsRef::as_ref))
            .current_dir(std::env::temp_dir())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(path) = &self.path {
            command.env("PATH", path);
        }
        let command_line = args
            .first()
            .map(|arg| format!("{} {}", self.program.display(), arg.as_ref()))
            .unwrap_or_else(|| self.program.display().to_string());
        tokio::time::timeout(timeout, command.output())
            .await
            .with_context(|| format!("{command_line} didn't finish"))?
            .with_context(|| format!("running {command_line}"))
    }

    fn name(&self) -> &'static str {
        self.provider.description().name
    }
}

fn is_not_found(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|error| error.kind() == std::io::ErrorKind::NotFound)
}

/// `proton 0.1.2` is version `0.1.2`; a CLI that prints only its version keeps it as it is.
fn version_of(stdout: &[u8], program: &str) -> String {
    let stdout = String::from_utf8_lossy(stdout);
    let line = first_line(&stdout).unwrap_or_default();
    line.strip_prefix(program)
        .map(str::trim)
        .unwrap_or(line)
        .trim_start_matches("version ")
        .to_string()
}

fn first_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).find(|line| !line.is_empty())
}

fn last_line(text: &str) -> Option<&str> {
    text.lines().map(str::trim).rfind(|line| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use agentz_protocol::cloud::CloudState;

    use super::*;

    /// A fake CLI: a script that prints `stdout` (and `stderr`) for the commands it knows,
    /// and logs every command it's run with.
    fn fake_cli(directory: &tempfile::TempDir, cases: &[(&str, &str, &str, i32)]) -> PathBuf {
        let mut script =
            String::from("#!/bin/sh\necho \"$*\" >> \"$(dirname \"$0\")/log\"\ncase \"$*\" in\n");
        for (args, stdout, stderr, status) in cases {
            let stdout_file = directory
                .path()
                .join(format!("out-{}", args.replace(' ', "_")));
            std::fs::write(&stdout_file, stdout).expect("writes stdout");
            script.push_str(&format!(
                "  \"{args}\") cat '{}'; printf '%s' '{stderr}' >&2; exit {status} ;;\n",
                stdout_file.display()
            ));
        }
        script.push_str("  *) echo \"unknown: $*\" >&2; exit 2 ;;\nesac\n");
        let path = directory.path().join("cli");
        std::fs::write(&path, script).expect("writes the script");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("makes it runnable");
        path
    }

    fn commands_run(directory: &tempfile::TempDir) -> Vec<String> {
        std::fs::read_to_string(directory.path().join("log"))
            .unwrap_or_default()
            .lines()
            .map(String::from)
            .collect()
    }

    #[tokio::test]
    async fn a_missing_cli_is_not_installed() {
        let cli = Cli::with_program(Provider::Proton, "/nonexistent/agentz-test-proton");
        assert_eq!(cli.status().await, ProviderStatus::NotInstalled);
    }

    #[tokio::test]
    async fn whoami_says_who_is_signed_in() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[
                (
                    "--version",
                    include_str!("proton_outputs/version.txt"),
                    "",
                    0,
                ),
                ("whoami", include_str!("proton_outputs/whoami.txt"), "", 0),
            ],
        );
        let cli = Cli::with_program(Provider::Proton, program);
        assert_eq!(
            cli.status().await,
            ProviderStatus::SignedIn {
                version: "0.1.2".into(),
                account: "alex@example.com".into(),
            }
        );
    }

    #[tokio::test]
    async fn a_failing_whoami_is_signed_out() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[
                (
                    "--version",
                    include_str!("proton_outputs/version.txt"),
                    "",
                    0,
                ),
                (
                    "whoami",
                    "",
                    include_str!("proton_outputs/whoami-signed-out.txt"),
                    1,
                ),
            ],
        );
        let cli = Cli::with_program(Provider::Proton, program);
        assert_eq!(
            cli.status().await,
            ProviderStatus::SignedOut {
                version: "0.1.2".into()
            }
        );
    }

    #[tokio::test]
    async fn a_failing_command_says_what_the_cli_said() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[(
                "list --json",
                "",
                "proton: not signed in; run: proton login\n",
                1,
            )],
        );
        let cli = Cli::with_program(Provider::Proton, program);
        let error = cli.machines().await.expect_err("fails");
        assert_eq!(error.to_string(), "not signed in; run: proton login");
    }

    #[tokio::test]
    async fn proton_lists_its_computers() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[(
                "list --json",
                include_str!("proton_outputs/list.json"),
                "",
                0,
            )],
        );
        let cli = Cli::with_program(Provider::Proton, program);
        let machines = cli.machines().await.expect("lists");
        assert_eq!(machines.len(), 1);
        assert_eq!(machines[0].name, "devbox-3");
        assert_eq!(machines[0].state, CloudState::Running);
    }

    #[tokio::test]
    async fn starting_a_proton_computer_skips_its_price_question() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[
                ("start devbox-3 --yes", "", "  start: succeeded\n", 0),
                ("stop devbox-3", "", "  stop: succeeded\n", 0),
            ],
        );
        let cli = Cli::with_program(Provider::Proton, program);
        cli.change("devbox-3", Lifecycle::Start)
            .await
            .expect("starts");
        cli.change("devbox-3", Lifecycle::Stop)
            .await
            .expect("stops");
        assert_eq!(
            commands_run(&directory),
            ["start devbox-3 --yes", "stop devbox-3"]
        );
        let error = cli
            .change("devbox-3", Lifecycle::Pause)
            .await
            .expect_err("can't pause");
        assert_eq!(error.to_string(), "Proton computers can't be paused");
    }

    #[tokio::test]
    async fn instabox_renews_the_certificate_before_ssh_options() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[
                ("ssh-cert agents-1", "", "", 0),
                (
                    "ssh-config agents-1",
                    "Host instabox-agents-1\n  HostName ssh.instabox.sh\n  Port 2222\n  User m_q4mz7tk2bx5rn3wd6vp2hj4sye\n  CertificateFile /Users/alex/.config/instabox/ssh/m_q4mz-cert.pub\n",
                    "",
                    0,
                ),
            ],
        );
        let cli = Cli::with_program(Provider::Instabox, program);
        let options = cli.ssh_options("agents-1").await.expect("gives options");
        assert_eq!(options.host_name, "ssh.instabox.sh");
        assert_eq!(options.port, Some(2222));
        assert_eq!(
            commands_run(&directory),
            ["ssh-cert agents-1", "ssh-config agents-1"]
        );
    }
}
