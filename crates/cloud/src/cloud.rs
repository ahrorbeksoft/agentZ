//! Cloud providers' CLIs (design/cloud): finding each one, and running it to list, make, start
//! and stop machines and to say how SSH reaches them. The user installs each CLI and signs in
//! with it; agentZ only runs it, with the login shell's environment the server already has, and
//! never reads its login.

mod instabox;
mod instabox_api;
mod proton;
mod ssh_config;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use agentz_protocol::cloud::{
    AccountRules, CloudMachine, CloudSources, CloudUsage, CreateMachine, Lifecycle, MachineAction,
    MachineDetails, Provider, ProviderStatus, SshOptions,
};
use anyhow::{Context as _, Result, anyhow};

pub use instabox_api::InstaboxApi;

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
        let whoami: &[&str] = match self.provider {
            Provider::Instabox => &["whoami", "--json"],
            Provider::Proton => &["whoami"],
        };
        match self.run(whoami, READ_TIMEOUT).await {
            Ok(stdout) => match self.account(&stdout) {
                Some((account, rules)) => ProviderStatus::SignedIn {
                    version,
                    account,
                    rules,
                },
                None => ProviderStatus::SignedOut { version },
            },
            Err(error) if is_not_found(&error) => ProviderStatus::NotInstalled,
            Err(_) => ProviderStatus::SignedOut { version },
        }
    }

    /// Who `whoami` names, and the account's rules where it gives them.
    fn account(&self, stdout: &str) -> Option<(String, AccountRules)> {
        if self.provider == Provider::Instabox {
            match instabox::parse_whoami(stdout) {
                Ok(account) => return Some(account),
                Err(error) => log::warn!("reading instabox whoami --json: {error:#}"),
            }
        }
        first_line(stdout).map(|account| (account.to_string(), AccountRules::default()))
    }

    pub async fn machines(&self) -> Result<Vec<CloudMachine>> {
        let (args, parse): (&[&str], fn(&str) -> Result<Vec<CloudMachine>>) = match self.provider {
            Provider::Instabox => (&["ls", "--json"], instabox::parse_machines),
            Provider::Proton => (&["list", "--json"], proton::parse_machines),
        };
        let stdout = self.run(args, READ_TIMEOUT).await?;
        parse(&stdout).with_context(|| format!("reading {}'s machines", self.name()))
    }

    /// What new machines can start from: the provider's templates and the account's snapshots.
    pub async fn sources(&self) -> Result<CloudSources> {
        match self.provider {
            Provider::Instabox => {
                let templates = self.run(&["templates", "--json"], READ_TIMEOUT).await?;
                let snapshots = self
                    .run(&["snapshot", "ls", "--json"], READ_TIMEOUT)
                    .await?;
                Ok(CloudSources {
                    templates: instabox::parse_templates(&templates)?,
                    snapshots: instabox::parse_snapshots(&snapshots)?,
                })
            }
            Provider::Proton => Ok(CloudSources::default()),
        }
    }

    /// Makes a machine and waits until it's made, then gives it as the CLI lists it: neither
    /// CLI's `create` prints it.
    pub async fn create(&self, machine: &CreateMachine) -> Result<CloudMachine> {
        let args = match self.provider {
            Provider::Instabox => instabox::create_args(machine),
            Provider::Proton => proton::create_args(machine)?,
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

    /// What a machine's page shows beyond its listing (topic 7 A), by the machine's id. What
    /// can't be read is named in its `errors`, and the rest is still given.
    pub async fn details(&self, machine: &str, api: &InstaboxApi) -> Result<MachineDetails> {
        if self.provider == Provider::Proton {
            let stdout = self.run(&["show", machine, "--json"], READ_TIMEOUT).await?;
            return proton::parse_details(&stdout);
        }
        let stdout = self.run(&["get", machine, "--json"], READ_TIMEOUT).await?;
        let (mut details, running) = instabox::parse_details(&stdout)?;
        let vault = async {
            let stdout = self
                .run(&["vault", "show", machine, "--json"], READ_TIMEOUT)
                .await?;
            instabox::parse_vault(&stdout)
        };
        let ports = async {
            let stdout = self
                .run(&["port", "ls", machine, "--json"], READ_TIMEOUT)
                .await?;
            instabox::parse_ports(&stdout)
        };
        let snapshots = async {
            let stdout = self
                .run(&["snapshot", "ls", "--json"], READ_TIMEOUT)
                .await?;
            instabox::parse_snapshots(&stdout)
        };
        // A machine that isn't running has no samples.
        let metrics = async {
            if running {
                api.metrics(self, machine).await
            } else {
                Ok(Vec::new())
            }
        };
        let (vault, ports, snapshots, metrics, running_time) = futures::join!(
            vault,
            ports,
            snapshots,
            metrics,
            api.running_time(self, machine)
        );
        let mut errors = Vec::new();
        let mut keep = |what: &str, error: anyhow::Error| errors.push(format!("{what}: {error:#}"));
        details.vault = vault.map_err(|error| keep("Vault", error)).ok();
        details.ports = ports
            .map_err(|error| keep("Ports", error))
            .unwrap_or_default();
        let mut snapshots: Vec<_> = snapshots
            .map_err(|error| keep("Snapshots", error))
            .unwrap_or_default()
            .into_iter()
            .filter(|snapshot| snapshot.source_machine.as_deref() == Some(machine))
            .collect();
        snapshots.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.created_at));
        details.snapshots = snapshots;
        details.metrics = metrics
            .map_err(|error| keep("CPU and memory", error))
            .unwrap_or_default();
        details.running = running_time
            .map_err(|error| keep("Running time", error))
            .ok();
        details.errors = errors;
        Ok(details)
    }

    /// Does a page's action to a machine, by its id, and waits until it's done. Gives what's for
    /// the user to copy or open: an SSH command, a desktop link, a published port's URL.
    pub async fn act(
        &self,
        machine: &str,
        action: &MachineAction,
        api: &InstaboxApi,
    ) -> Result<Option<String>> {
        if let (Provider::Instabox, MachineAction::DesktopLink { view_only }) =
            (self.provider, action)
        {
            return api.desktop_link(self, machine, *view_only).await.map(Some);
        }
        let args = match self.provider {
            Provider::Instabox => instabox::action_args(machine, action)?,
            Provider::Proton => proton::action_args(machine, action)?,
        };
        let stdout = self.run(&args, CHANGE_TIMEOUT).await?;
        Ok(match action {
            MachineAction::SshCommand => Some(instabox::parse_ssh_command(&stdout)?),
            MachineAction::PublishPort { .. } => instabox::published_url(&stdout),
            _ => None,
        })
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
    use std::collections::{HashMap, VecDeque};
    use std::os::unix::fs::PermissionsExt as _;
    use std::sync::{Arc, Mutex};

    use agentz_protocol::cloud::CloudState;
    use futures::AsyncReadExt as _;
    use http_client::{AsyncBody, BlockedHttpClient, FakeHttpClient, HttpClient, Response};

    use super::*;

    const MACHINE: &str = "m_q4mz7tk2bx5rn3wd6vp2hj4sye";
    const MAKE_KEY: &str =
        "keys create --name agentZ --scope machines:read --scope machines:write --json";

    type Requests = Arc<Mutex<Vec<String>>>;

    /// A fake Instabox API. A request is answered with the next of the answers for its method,
    /// path and `action` (the last again once they run out) while it has the key `ibk_good`,
    /// and refused with any other. Each is logged as "METHOD path [action] key", with the body
    /// of a desktop link's.
    fn fake_api(answers: &[(&str, &[&str])]) -> (Arc<dyn HttpClient>, Requests) {
        let answers: HashMap<String, VecDeque<String>> = answers
            .iter()
            .map(|(name, answers)| {
                let answers = answers.iter().map(|answer| answer.to_string()).collect();
                (name.to_string(), answers)
            })
            .collect();
        let answers = Arc::new(Mutex::new(answers));
        let requests = Requests::default();
        let http = FakeHttpClient::create({
            let requests = requests.clone();
            move |mut request| {
                let answers = answers.clone();
                let requests = requests.clone();
                async move {
                    let mut body = String::new();
                    request.body_mut().read_to_string(&mut body).await?;
                    let body: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
                    let key = request
                        .headers()
                        .get("X-API-Key")
                        .and_then(|key| key.to_str().ok())
                        .unwrap_or_default()
                        .to_string();
                    let mut name = format!("{} {}", request.method(), request.uri().path());
                    if let Some(action) = body["action"].as_str() {
                        name = format!("{name} {action}");
                    }
                    let mut logged = format!("{name} {key}");
                    if name.ends_with("/stream") {
                        logged = format!("{logged} {body}");
                    }
                    requests.lock().expect("lock").push(logged);
                    let (status, answer) = if key == "ibk_good" {
                        match answers.lock().expect("lock").get_mut(&name) {
                            Some(queue) if queue.len() > 1 => {
                                (200, queue.pop_front().unwrap_or_default())
                            }
                            Some(queue) => (200, queue.front().cloned().unwrap_or_default()),
                            None => (404, r#"{"error": {"message": "not found"}}"#.into()),
                        }
                    } else {
                        (
                            401,
                            r#"{"error": {"code": "unauthorized", "message": "invalid API key"}}"#
                                .into(),
                        )
                    };
                    Ok(Response::builder()
                        .status(status)
                        .body(AsyncBody::from(answer))?)
                }
            }
        });
        (http, requests)
    }

    fn blocked_api(directory: &tempfile::TempDir) -> InstaboxApi {
        InstaboxApi::new(Arc::new(BlockedHttpClient::new()), directory.path())
    }

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
                rules: AccountRules::default(),
            }
        );
    }

    #[tokio::test]
    async fn instabox_names_the_account_and_its_rules() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[
                ("--version", "instabox cbecf34a\n", "", 0),
                (
                    "whoami --json",
                    include_str!("instabox_outputs/whoami.json"),
                    "",
                    0,
                ),
            ],
        );
        let cli = Cli::with_program(Provider::Instabox, program);
        let ProviderStatus::SignedIn {
            version,
            account,
            rules,
        } = cli.status().await
        else {
            panic!("signed in");
        };
        assert_eq!(version, "cbecf34a");
        assert_eq!(account, "me@example.com");
        assert_eq!(rules.fixed_idle_pause, Some(Duration::from_secs(1800)));
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
                ..Default::default()
            })
            .await
            .expect("creates");
        assert_eq!(machine.id, "c2");
        assert_eq!(
            commands_run(&directory),
            ["create scratch --yes", "list --json"]
        );
    }

    #[tokio::test]
    async fn instabox_starts_from_templates_and_snapshots() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[
                (
                    "templates --json",
                    include_str!("instabox_outputs/templates.json"),
                    "",
                    0,
                ),
                (
                    "snapshot ls --json",
                    include_str!("instabox_outputs/snapshots.json"),
                    "",
                    0,
                ),
            ],
        );
        let sources = Cli::with_program(Provider::Instabox, program)
            .sources()
            .await
            .expect("reads them");
        assert_eq!(sources.templates.len(), 4);
        assert_eq!(sources.snapshots.len(), 2);
        let proton = Cli::with_program(Provider::Proton, "/nonexistent/agentz-test-proton");
        assert_eq!(
            proton.sources().await.expect("has none"),
            CloudSources::default()
        );
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
                    include_str!("instabox_outputs/ssh-config.txt"),
                    "",
                    0,
                ),
            ],
        );
        let cli = Cli::with_program(Provider::Instabox, program);
        let options = cli.ssh_options("agents-1").await.expect("gives options");
        assert_eq!(options.host_name, "api.instabox.sh");
        assert_eq!(options.port, Some(2222));
        assert_eq!(
            options.user.as_deref(),
            Some("m_q4mz7tk2bx5rn3wd6vp2hj4sye")
        );
        assert!(
            options
                .options
                .iter()
                .any(|(key, value)| key == "CertificateFile" && value.ends_with("-cert.pub"))
        );
        assert_eq!(
            commands_run(&directory),
            ["ssh-cert agents-1", "ssh-config agents-1"]
        );
    }

    #[tokio::test]
    async fn an_instabox_machines_page_reads_its_cli_and_api() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let data = tempfile::tempdir().expect("makes a folder");
        let running = include_str!("instabox_outputs/get.json").replacen(
            r#""state": "suspended""#,
            r#""state": "running""#,
            1,
        );
        let program = fake_cli(
            &directory,
            &[
                (&format!("get {MACHINE} --json"), &running, "", 0),
                (
                    &format!("vault show {MACHINE} --json"),
                    include_str!("instabox_outputs/vault-show.json"),
                    "",
                    0,
                ),
                (
                    &format!("port ls {MACHINE} --json"),
                    "",
                    "instabox: port ls: the server is busy\n",
                    1,
                ),
                (
                    "snapshot ls --json",
                    include_str!("instabox_outputs/snapshots.json"),
                    "",
                    0,
                ),
                (
                    MAKE_KEY,
                    r#"{"id": "key_1", "name": "agentZ", "key": "ibk_good"}"#,
                    "",
                    0,
                ),
            ],
        );
        let metrics = format!("GET /v1/machines/{MACHINE}/metrics");
        let usage = format!("GET /v1/machines/{MACHINE}/usage");
        let (http, _) = fake_api(&[
            (
                &metrics,
                &[r#"{"metrics": [
                    {"timestamp": "2026-10-18T10:00:30Z", "cpuUsedPct": 12.5, "cpuCount": 4,
                     "memUsed": 2147483648, "memTotal": 8589934592, "diskTotal": 21474836480},
                    {"timestamp": "2026-10-18T10:00:00Z", "cpuUsedPct": 3, "cpuCount": 4,
                     "memUsed": 1073741824, "memTotal": 8589934592, "diskTotal": 21474836480}]}"#],
            ),
            (
                &usage,
                &[
                    r#"{"from": "2026-10-01T00:00:00Z", "to": "2026-10-18T10:01:00Z",
                      "runningSeconds": 7200, "vcpuSeconds": 28800, "memGiBSeconds": 0,
                      "diskGiBSeconds": 0}"#,
                ],
            ),
        ]);
        let api = InstaboxApi::new(http, data.path());
        let cli = Cli::with_program(Provider::Instabox, program);
        let details = cli.details(MACHINE, &api).await.expect("reads it");

        assert_eq!(
            details.template.as_deref(),
            Some("agent-browser 2026-10-15.4")
        );
        assert_eq!(details.idle_pause, Some(Duration::from_secs(1800)));
        assert!(details.vault.is_some_and(|vault| vault.enabled));
        let snapshots: Vec<&str> = details
            .snapshots
            .iter()
            .map(|snapshot| snapshot.name.as_str())
            .collect();
        assert_eq!(snapshots, ["dev-base"]);
        let cpu: Vec<f64> = details
            .metrics
            .iter()
            .map(|sample| sample.cpu_percent)
            .collect();
        assert_eq!(cpu, [3., 12.5]);
        assert_eq!(details.metrics[1].memory_used_bytes, 2147483648);
        let running = details.running.expect("has its running time");
        assert_eq!((running.seconds, running.vcpu_seconds), (7200, Some(28800)));
        assert_eq!(details.ports, []);
        assert_eq!(details.errors, ["Ports: port ls: the server is busy"]);

        // One key for both requests, which only the user can read.
        let made = commands_run(&directory)
            .iter()
            .filter(|command| *command == MAKE_KEY)
            .count();
        assert_eq!(made, 1);
        let key_file = data.path().join("cloud/instabox/api-key");
        let mode = std::fs::metadata(&key_file)
            .expect("is kept")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert!(
            std::fs::read_to_string(&key_file)
                .expect("reads")
                .contains("ibk_good")
        );
    }

    #[tokio::test]
    async fn a_desktop_link_installs_the_desktop_and_replaces_a_refused_key() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let data = tempfile::tempdir().expect("makes a folder");
        let key_file = data.path().join("cloud/instabox/api-key");
        std::fs::create_dir_all(key_file.parent().expect("has a folder")).expect("makes it");
        std::fs::write(&key_file, r#"{"id": "key_old", "secret": "ibk_old"}"#).expect("writes");
        let program = fake_cli(
            &directory,
            &[
                (MAKE_KEY, r#"{"id": "key_new", "key": "ibk_good"}"#, "", 0),
                ("keys rm key_old", "", "", 0),
            ],
        );
        let desktop = "POST /v1/machines/m_1/desktop";
        let (http, requests) = fake_api(&[
            (
                &format!("{desktop} start"),
                &[
                    r#"{"installed": false, "running": false}"#,
                    r#"{"installed": true, "running": true}"#,
                ],
            ),
            (
                &format!("{desktop} install"),
                &[r#"{"installed": false, "running": false, "installing": true}"#],
            ),
            (
                &format!("{desktop} status"),
                &[
                    r#"{"installed": false, "running": false, "installing": true}"#,
                    r#"{"installed": true, "running": false}"#,
                ],
            ),
            (
                &format!("{desktop}/stream"),
                &[
                    r#"{"url": "https://desktop.instabox.sh/s/abc", "expiresAt": "2026-10-18T11:00:00Z"}"#,
                ],
            ),
        ]);
        let api = InstaboxApi::new(http, data.path()).polling_every(Duration::from_millis(1));
        let cli = Cli::with_program(Provider::Instabox, program);
        let link = cli
            .act("m_1", &MachineAction::DesktopLink { view_only: true }, &api)
            .await
            .expect("links it");

        assert_eq!(link.as_deref(), Some("https://desktop.instabox.sh/s/abc"));
        assert_eq!(
            *requests.lock().expect("lock"),
            [
                format!("{desktop} start ibk_old"),
                format!("{desktop} start ibk_good"),
                format!("{desktop} install ibk_good"),
                format!("{desktop} status ibk_good"),
                format!("{desktop} status ibk_good"),
                format!("{desktop} start ibk_good"),
                format!(r#"{desktop}/stream ibk_good {{"viewOnly":true,"ttlSeconds":3600}}"#),
            ]
        );
        assert_eq!(commands_run(&directory), [MAKE_KEY, "keys rm key_old"]);
        assert!(
            std::fs::read_to_string(&key_file)
                .expect("reads")
                .contains("ibk_good")
        );
    }

    #[tokio::test]
    async fn actions_run_the_cli_and_give_what_it_printed() {
        let directory = tempfile::tempdir().expect("makes a folder");
        let program = fake_cli(
            &directory,
            &[
                (
                    "ssh-token m_1 --json",
                    r#"{"command": "ssh -p 2222 t_abc@api.instabox.sh"}"#,
                    "",
                    0,
                ),
                ("snapshot save m_1 --name base --json", "{}", "", 0),
            ],
        );
        let api = blocked_api(&directory);
        let cli = Cli::with_program(Provider::Instabox, program);
        assert_eq!(
            cli.act("m_1", &MachineAction::SshCommand, &api)
                .await
                .expect("makes one")
                .as_deref(),
            Some("ssh -p 2222 t_abc@api.instabox.sh")
        );
        let save = MachineAction::SaveSnapshot {
            name: "base".into(),
            software_only: false,
        };
        assert_eq!(cli.act("m_1", &save, &api).await.expect("saves"), None);
        assert_eq!(
            commands_run(&directory),
            [
                "ssh-token m_1 --json",
                "snapshot save m_1 --name base --json"
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
        let api = blocked_api(&directory);
        let cli = Cli::with_program(Provider::Proton, program);
        let details = cli.details("devbox-3", &api).await.expect("reads it");
        assert!(details.started_at.is_some());
        assert_eq!(details.errors, Vec::<String>::new());
        let keys = MachineAction::ReplaceSshKeys(vec!["/tmp/a.pub".into()]);
        assert_eq!(
            cli.act("devbox-3", &keys, &api).await.expect("replaces"),
            None
        );
        assert_eq!(
            commands_run(&directory),
            ["show devbox-3 --json", "keys devbox-3 --key /tmp/a.pub"]
        );
    }
}
