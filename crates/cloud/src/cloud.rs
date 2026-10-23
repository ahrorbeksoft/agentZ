//! Cloud providers' CLIs (design/cloud): finding each one, and running it to list, make, start
//! and stop machines and to say how SSH reaches them. The user installs each CLI and signs in
//! with it; agentZ only runs it, with the login shell's environment the server already has, and
//! never reads its login.

mod own_key;
mod proton;
mod ssh_config;

pub use own_key::OwnKey;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use agentz_protocol::cloud::{
    CloudMachine, CloudUsage, CreateMachine, Lifecycle, MachineAction, MachineDetails, Provider,
    ProviderStatus, SshOptions,
};
use anyhow::{Context as _, Result, anyhow};

/// Reading: who's signed in, the machines, usage.
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Making or changing a machine. Proton's CLI waits on its own operations, for up to 20
/// minutes.
const CHANGE_TIMEOUT: Duration = Duration::from_secs(21 * 60);

/// A provider's CLI.
#[derive(Clone, Debug)]
pub struct Cli {
    provider: Provider,
    program: PathBuf,
    /// The `PATH` it's looked for on and runs with; `None` is the server's own.
    path: Option<OsString>,
    /// agentZ's own key, which machines are made and reached with; `None` leaves the key to
    /// the CLI.
    own_key: Option<OwnKey>,
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
            own_key: None,
        }
    }

    /// The provider's CLI, found on `path` rather than the server's own `PATH`.
    pub fn on_path(provider: Provider, path: Option<OsString>) -> Self {
        Self {
            path,
            ..Self::new(provider)
        }
    }

    pub fn with_own_key(self, own_key: OwnKey) -> Self {
        Self {
            own_key: Some(own_key),
            ..self
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
            Provider::Proton => (&["list", "--json"], proton::parse_machines),
        };
        let stdout = self.run(args, READ_TIMEOUT).await?;
        parse(&stdout).with_context(|| format!("reading {}'s machines", self.name()))
    }

    /// Makes a machine and waits until it's made, then gives it as the CLI lists it: its
    /// `create` doesn't print it.
    pub async fn create(&self, machine: &CreateMachine) -> Result<CloudMachine> {
        let key = match &self.own_key {
            Some(own_key) => Some(own_key.public_key().await?),
            None => None,
        };
        let args = match self.provider {
            Provider::Proton => proton::create_args(machine, key.as_deref())?,
        };
        self.run(&args, CHANGE_TIMEOUT).await?;
        self.machines()
            .await?
            .into_iter()
            .find(|listed| listed.name == machine.name)
            .with_context(|| format!("{} doesn't list {} once made", self.name(), machine.name))
    }

    /// Changes a machine, by its id, and waits until it's done.
    pub async fn change(&self, machine: &str, change: Lifecycle) -> Result<()> {
        let args = match self.provider {
            Provider::Proton => proton::change_args(machine, change),
        };
        self.run(&args, CHANGE_TIMEOUT).await.map(|_| ())
    }

    /// How SSH reaches a running machine, from the CLI's `ssh-config`, with agentZ's own key.
    pub async fn ssh_options(&self, machine: &str) -> Result<SshOptions> {
        let stdout = self.run(&["ssh-config", machine], READ_TIMEOUT).await?;
        let mut options = ssh_config::parse(&stdout)?;
        if let Some(own_key) = &self.own_key {
            options.options.extend(own_key.identity_options().await?);
        }
        Ok(options)
    }

    /// What a machine's page shows beyond its listing (topic 7 A), by the machine's id.
    pub async fn details(&self, machine: &str) -> Result<MachineDetails> {
        let (args, parse): (&[&str], fn(&str) -> Result<MachineDetails>) = match self.provider {
            Provider::Proton => (&["show", machine, "--json"], proton::parse_details),
        };
        let stdout = self.run(args, READ_TIMEOUT).await?;
        parse(&stdout)
    }

    /// Does a page's action to a machine, by its id, and waits until it's done. Replacing its
    /// keys keeps agentZ's own among them.
    pub async fn act(&self, machine: &str, action: &MachineAction) -> Result<()> {
        let action = match (action, &self.own_key) {
            (MachineAction::ReplaceSshKeys(keys), Some(own_key)) => {
                let own = own_key.public_key().await?;
                let mut keys = keys.clone();
                if !keys.contains(&own) {
                    keys.push(own);
                }
                MachineAction::ReplaceSshKeys(keys)
            }
            (action, _) => action.clone(),
        };
        let args = match self.provider {
            Provider::Proton => proton::action_args(machine, &action)?,
        };
        self.run(&args, CHANGE_TIMEOUT).await.map(|_| ())
    }

    pub async fn usage(&self) -> Result<CloudUsage> {
        let (args, parse): (&[&str], fn(&str) -> Result<CloudUsage>) = match self.provider {
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
        tokio::time::timeout(timeout, child_process::output(&mut command))
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
        for (index, (args, stdout, stderr, status)) in cases.iter().enumerate() {
            let stdout_file = directory.path().join(format!("out-{index}"));
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
    }

    #[tokio::test]
    async fn creating_gives_the_machine_as_listed() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[
                ("create scratch --yes", "Creating scratch…\n", "", 0),
                (
                    "list --json",
                    r#"[{"id": "c2", "name": "scratch", "state": "running"}]"#,
                    "",
                    0,
                ),
            ],
        );
        let cli = Cli::with_program(Provider::Proton, program);
        let machine = cli
            .create(&CreateMachine {
                name: "scratch".into(),
            })
            .await
            .expect("creates");
        assert_eq!(machine.id, "c2");
        assert_eq!(
            commands_run(&directory),
            ["create scratch --yes", "list --json"]
        );
    }

    /// With agentZ's own key, a machine is made with it, reached with it, and keeps it when its
    /// keys are replaced.
    #[tokio::test]
    async fn machines_are_made_and_reached_with_agentzs_own_key() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let data = tempfile::tempdir().expect("makes a folder");
        let own_key = OwnKey::new(data.path().join("cloud").join("id_ed25519"));
        let public = data.path().join("cloud").join("id_ed25519.pub");
        let create = format!("create scratch --yes --key {}", public.display());
        let keys = format!("keys c2 --key /tmp/a.pub --key {}", public.display());
        let program = fake_cli(
            &directory,
            &[
                (&create, "Creating scratch…\n", "", 0),
                (
                    "list --json",
                    r#"[{"id": "c2", "name": "scratch", "state": "running"}]"#,
                    "",
                    0,
                ),
                (
                    "ssh-config c2",
                    include_str!("proton_outputs/ssh-config.txt"),
                    "",
                    0,
                ),
                (&keys, "", "", 0),
            ],
        );
        let cli = Cli::with_program(Provider::Proton, program).with_own_key(own_key.clone());
        cli.create(&CreateMachine {
            name: "scratch".into(),
        })
        .await
        .expect("creates");
        assert!(public.is_file(), "the key was made");
        let options = cli.ssh_options("c2").await.expect("has options");
        assert!(
            options.options.contains(&(
                "IdentityFile".to_string(),
                own_key.path().display().to_string()
            )),
            "{options:?}"
        );
        cli.act(
            "c2",
            &MachineAction::ReplaceSshKeys(vec!["/tmp/a.pub".into()]),
        )
        .await
        .expect("replaces");
        assert_eq!(
            commands_run(&directory),
            [
                create.as_str(),
                "list --json",
                "ssh-config c2",
                keys.as_str()
            ]
        );
    }

    #[tokio::test]
    async fn a_proton_computers_page_reads_show() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[
                (
                    "show devbox-3 --json",
                    include_str!("proton_outputs/show.json"),
                    "",
                    0,
                ),
                ("keys devbox-3 --key /tmp/a.pub", "", "", 0),
            ],
        );
        let cli = Cli::with_program(Provider::Proton, program);
        let details = cli.details("devbox-3").await.expect("reads it");
        assert!(details.started_at.is_some());
        let keys = MachineAction::ReplaceSshKeys(vec!["/tmp/a.pub".into()]);
        cli.act("devbox-3", &keys).await.expect("replaces");
        assert_eq!(
            commands_run(&directory),
            ["show devbox-3 --json", "keys devbox-3 --key /tmp/a.pub"]
        );
    }
}
