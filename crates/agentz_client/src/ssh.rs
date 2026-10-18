//! Reaching `agentz-server` on another machine over SSH, the way herdr reaches its remote
//! hosts: OpenSSH in batch mode with a shared control master, the server binary uploaded to
//! `~/.agentz/server/`, and `agentz-server proxy` carrying the connection over the session's
//! stdin and stdout.
//!
//! Nothing secret is stored. Authentication stays with OpenSSH: keys, the agent and
//! `~/.ssh/config`. A cloud provider's machine is reached with the options its CLI's
//! `ssh-config` gives, passed as `-o`, so the user's config is never edited.

use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use agentz_protocol::ClientKind;
use agentz_protocol::cloud::{SshAddress, SshOptions};
use anyhow::{Context as _, Result, anyhow, bail};
use sha2::{Digest as _, Sha256};
use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _,
};
use tokio::process::{Child, ChildStdin, ChildStdout};

use crate::{Connection, Events};

/// Printed before a remote command's own output, so whatever the user's shell startup files
/// print can be told apart and dropped (herdr's `REMOTE_OUTPUT_READY_MARKER`).
const OUTPUT_READY_MARKER: &str = "agentz-remote-output-ready:1";
/// Runs this instead of `ssh`, for tests.
const SSH_PROGRAM_ENV_VAR: &str = "AGENTZ_SSH";
const COMMAND_TIMEOUT: Duration = Duration::from_secs(15);
/// For `ssh -G` and asking a master to stop, which answer at once when they work (herdr's
/// `SSH_CONTROL_TIMEOUT`).
const CONTROL_TIMEOUT: Duration = Duration::from_secs(3);
/// herdr's options for background commands: never prompt, check host keys strictly, and give
/// up on a dead network.
const BATCH_OPTIONS: [&str; 7] = [
    "BatchMode=yes",
    "NumberOfPasswordPrompts=0",
    "StrictHostKeyChecking=yes",
    "ConnectTimeout=10",
    "ConnectionAttempts=1",
    "ServerAliveInterval=15",
    "ServerAliveCountMax=4",
];
/// How long uploading the server may go without ssh taking more of it. A slow link takes as
/// long as it takes; one that stopped moving is given up.
const UPLOAD_STALL_TIMEOUT: Duration = Duration::from_secs(60);
const UPLOAD_CHUNK_SIZE: usize = 64 * 1024;
/// Covers starting the server on the other end, which loads the login shell's environment.
const PROXY_READY_TIMEOUT: Duration = Duration::from_secs(30);
const STDERR_LIMIT: usize = 16 * 1024;
/// Where the server is installed, quoted for the remote shell with `$HOME` left to it. One
/// path for every version, as herdr installs: a server hands off to the binary at the path it
/// runs from, so a newer version installed beside it would never take over.
const INSTALL_DIRECTORY: &str = "\"$HOME\"/.agentz/server";
/// OpenSSH's limit on a unix socket path, which the control path must fit once `%C` expands.
const MAX_CONTROL_PATH_LENGTH: usize = 103;
/// The length of `%C` once expanded, plus the `.` and 16 characters OpenSSH appends for the
/// socket it creates before renaming it into place.
const CONTROL_PATH_EXPANSION: usize = 40 + 17;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RemoteOs {
    Linux,
    Macos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RemoteArch {
    X86_64,
    Aarch64,
}

/// What `uname -sm` says about the other machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RemotePlatform {
    pub os: RemoteOs,
    pub arch: RemoteArch,
}

impl RemotePlatform {
    /// herdr's `RemotePlatform::from_uname`.
    pub fn from_uname(os: &str, arch: &str) -> Result<Self> {
        let parsed_os = match os.trim() {
            "Linux" => Some(RemoteOs::Linux),
            "Darwin" => Some(RemoteOs::Macos),
            _ => None,
        };
        let parsed_arch = match arch.trim() {
            "x86_64" | "amd64" => Some(RemoteArch::X86_64),
            "aarch64" | "arm64" => Some(RemoteArch::Aarch64),
            _ => None,
        };
        match (parsed_os, parsed_arch) {
            (Some(os), Some(arch)) => Ok(Self { os, arch }),
            _ => bail!("unsupported remote platform: {} {}", os.trim(), arch.trim()),
        }
    }

    /// The platform this binary was built for.
    pub fn current() -> Option<Self> {
        let os = match std::env::consts::OS {
            "linux" => RemoteOs::Linux,
            "macos" => RemoteOs::Macos,
            _ => return None,
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => RemoteArch::X86_64,
            "aarch64" => RemoteArch::Aarch64,
            _ => return None,
        };
        Some(Self { os, arch })
    }

    /// The Rust target that builds a server for it. Linux servers are static musl builds, so
    /// they run whatever the distribution.
    pub fn rust_target(&self) -> &'static str {
        match (self.os, self.arch) {
            (RemoteOs::Linux, RemoteArch::X86_64) => "x86_64-unknown-linux-musl",
            (RemoteOs::Linux, RemoteArch::Aarch64) => "aarch64-unknown-linux-musl",
            (RemoteOs::Macos, RemoteArch::X86_64) => "x86_64-apple-darwin",
            (RemoteOs::Macos, RemoteArch::Aarch64) => "aarch64-apple-darwin",
        }
    }
}

impl std::fmt::Display for RemotePlatform {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let os = match self.os {
            RemoteOs::Linux => "Linux",
            RemoteOs::Macos => "macOS",
        };
        let arch = match self.arch {
            RemoteArch::X86_64 => "x86_64",
            RemoteArch::Aarch64 => "aarch64",
        };
        write!(formatter, "{os} {arch}")
    }
}

/// Why connecting failed, sorted the way herdr sorts a saved machine's failures: some fix
/// themselves and are retried, others need the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshError {
    pub message: String,
    /// Retrying won't help until the user does something, such as accepting a host key.
    pub needs_attention: bool,
    /// What to do about it.
    pub hint: Option<String>,
}

impl std::fmt::Display for SshError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SshError {}

impl SshError {
    /// herdr's `saved_ssh_failure_needs_attention` and its hints. `command` is what reaches the
    /// machine from a terminal, which the hints name.
    pub fn classify(command: &str, error: &anyhow::Error) -> Self {
        let message = format!("{error:#}");
        let lowercase = message.to_lowercase();
        let host_key = [
            "host key verification failed",
            "remote host identification has changed",
            "no matching host key",
        ]
        .iter()
        .any(|needle| lowercase.contains(needle));
        let authentication = !host_key
            && ((lowercase.contains("permission denied")
                && ["(publickey", "(keyboard-interactive", "(password"]
                    .iter()
                    .any(|needle| lowercase.contains(needle)))
                || (lowercase.contains("signing failed")
                    && (lowercase.contains("sign_and_send_pubkey")
                        || lowercase.contains("agent"))));
        let hint = if host_key {
            Some(format!(
                "agentZ checks host keys strictly. Run `{command}` in a terminal to check \
                 and accept the host key, then retry."
            ))
        } else if authentication {
            Some(format!(
                "Check that `{command}` works in a terminal. If your key has a passphrase, \
                 add it to ssh-agent with `ssh-add`."
            ))
        } else if lowercase.contains("unsupported remote platform")
            || lowercase.contains("no agentz-server for")
        {
            Some("agentZ runs on Macs and on Linux, on x86_64 and aarch64.".to_string())
        } else {
            None
        };
        let needs_attention = host_key
            || authentication
            || hint.is_some()
            || [
                "permission denied",
                "protocol",
                "handshake",
                "refused the connection",
            ]
            .iter()
            .any(|needle| lowercase.contains(needle));
        Self {
            message,
            needs_attention,
            hint,
        }
    }
}

/// Rejects what ssh would read as an option, or what can't be a host (herdr's
/// `validate_remote_target`). Aliases, `user@host` and `ssh://user@host:port` all pass.
pub fn validate_target(target: &str) -> Result<()> {
    if target.is_empty() {
        bail!("the SSH host is empty");
    }
    if target.starts_with('-') {
        bail!("an SSH host can't start with “-”");
    }
    if target
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        bail!("an SSH host can't contain spaces");
    }
    Ok(())
}

/// An option's value as `ssh -o` reads it: it splits the value at spaces and takes quotes and
/// backslashes as its own, so a value with any of them is put in double quotes, escaped.
fn option_value(value: &str) -> String {
    let is_plain = !value
        .chars()
        .any(|character| character.is_whitespace() || "\"'\\#".contains(character));
    if is_plain {
        return value.to_string();
    }
    let mut quoted = String::from('"');
    for character in value.chars() {
        if matches!(character, '"' | '\\') {
            quoted.push('\\');
        }
        quoted.push(character);
    }
    quoted.push('"');
    quoted
}

/// Quotes a word for a POSIX shell (herdr's `shell_quote`).
pub fn shell_quote(value: &str) -> String {
    let is_plain = !value.is_empty()
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "@%_+=:,./-".contains(character));
    if is_plain {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

/// `ssh` to one machine, with the options every command shares.
#[derive(Clone, Debug)]
pub struct Ssh {
    program: PathBuf,
    target: String,
    /// `Key=Value` for `-o`, before the target in every command: a provider's `ssh-config`.
    options: Vec<String>,
    /// What reaches the machine from a terminal, for hints.
    terminal_command: String,
    control_path: Option<PathBuf>,
}

impl Ssh {
    pub fn new(target: &str) -> Result<Self> {
        Self::with_program(ssh_program(), target)
    }

    pub fn with_program(program: PathBuf, target: &str) -> Result<Self> {
        validate_target(target)?;
        Ok(Self {
            program,
            target: target.to_string(),
            options: Vec::new(),
            terminal_command: format!("ssh {target}"),
            control_path: shared_control_path(),
        })
    }

    /// A machine reached with `options` rather than a target in `~/.ssh/config`. `name` stands
    /// for it in messages, and `terminal_command` reaches it from a terminal
    /// (`proton ssh devbox-3`).
    pub fn with_options(name: &str, options: &SshOptions, terminal_command: &str) -> Result<Self> {
        Self::with_program_and_options(ssh_program(), name, options, terminal_command)
    }

    pub fn with_program_and_options(
        program: PathBuf,
        name: &str,
        options: &SshOptions,
        terminal_command: &str,
    ) -> Result<Self> {
        validate_target(name)?;
        validate_target(&options.host_name)?;
        let mut all = vec![("HostName".to_string(), options.host_name.clone())];
        all.extend(
            options
                .port
                .map(|port| ("Port".to_string(), port.to_string())),
        );
        all.extend(options.user.clone().map(|user| ("User".to_string(), user)));
        all.extend(options.options.iter().cloned());
        let options = all
            .into_iter()
            .map(|(key, value)| {
                anyhow::ensure!(
                    !key.is_empty()
                        && key
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric()),
                    "{key:?} isn't an SSH option"
                );
                anyhow::ensure!(
                    !value.chars().any(|character| character.is_control()),
                    "the SSH option {key} has a line break"
                );
                Ok(format!("{key}={}", option_value(&value)))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            program,
            target: name.to_string(),
            options,
            terminal_command: terminal_command.to_string(),
            control_path: shared_control_path(),
        })
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn terminal_command(&self) -> &str {
        &self.terminal_command
    }

    /// The machine's own options, then its target: what every command ends with.
    fn destination(&self, command: &mut tokio::process::Command) {
        for option in &self.options {
            command.arg("-o").arg(option);
        }
        command.arg(&self.target);
    }

    /// `ssh` with herdr's options for background commands: never prompt, check host keys
    /// strictly, give up on a dead network, and share one connection per machine.
    fn command(&self, remote_command: &str) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(&self.program);
        // Compress the first connection too: multiplexed sessions inherit the master's.
        command.arg("-C");
        if let Some(control_path) = &self.control_path {
            command
                .arg("-o")
                .arg(format!("ControlPath={}", control_path.display()))
                .args(["-o", "ControlMaster=auto", "-o", "ControlPersist=600"]);
        }
        for option in BATCH_OPTIONS {
            command.arg("-o").arg(option);
        }
        command.arg("-T");
        self.destination(&mut command);
        command
            .arg(remote_command)
            .env("SSH_ASKPASS_REQUIRE", "never")
            .env_remove("SSH_ASKPASS")
            .kill_on_drop(true);
        command
    }

    /// `ssh` through the given control socket only, never starting a master of its own, or
    /// over a connection of its own with `none` (herdr's `shared_control_command`).
    fn control_command(&self, control_path: &Path) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(&self.program);
        command
            .arg("-C")
            .arg("-o")
            .arg(format!("ControlPath={}", control_path.display()))
            .args(["-o", "ControlMaster=no", "-o", "ControlPersist=no"]);
        for option in BATCH_OPTIONS {
            command.arg("-o").arg(option);
        }
        command
            .env("SSH_ASKPASS_REQUIRE", "never")
            .env_remove("SSH_ASKPASS");
        command
    }

    /// What a local `ssh` command printed, once it ended within `timeout`.
    async fn output(
        &self,
        mut command: tokio::process::Command,
        timeout: Duration,
    ) -> Result<std::process::Output> {
        let output = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output();
        tokio::time::timeout(timeout, output)
            .await
            .map_err(|_| {
                anyhow!(
                    "ssh {} didn't answer within {} seconds",
                    self.target,
                    timeout.as_secs()
                )
            })?
            .with_context(|| format!("running {}", self.program.display()))
    }

    /// herdr's `prepare_shared_transport`. A master whose network went away, as when the Mac
    /// slept or changed networks, keeps its socket, and every command sent through it waits
    /// out its timeout. This checks the master still answers. When it doesn't but a
    /// connection of its own does, the master is asked to stop taking sessions (`-O stop`;
    /// `-O exit` would also cut those it still carries), and the next command starts a new
    /// one. True when it will.
    async fn prepare_shared_connection(&self) -> Result<bool> {
        let Some(control_path) = &self.control_path else {
            return Ok(false);
        };
        let Some(socket) = self.expanded_control_path(control_path).await else {
            return Ok(false);
        };
        if socket.parent() != control_path.parent() {
            return Ok(false);
        }
        let pinned = match PinnedControlSocket::new(&socket) {
            Ok(pinned) => pinned,
            // No master: the next command starts one.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
            Err(error) => {
                log::warn!(
                    "couldn't check the shared SSH connection to {}: {error}",
                    self.target
                );
                return Ok(false);
            }
        };
        if self.check(&pinned.alias, true).await.is_ok() {
            return Ok(false);
        }
        if !pinned.is_current()? {
            return Ok(true);
        }
        // The same allowance as the master's check, so a slow login shell doesn't count
        // against the master.
        let fresh = self.check(Path::new("none"), false).await;
        if !pinned.is_current()? {
            return Ok(true);
        }
        fresh?;
        log::info!(
            "stopping the shared SSH connection to {}, which stopped answering",
            self.target
        );
        let mut command = self.control_command(&pinned.alias);
        command.args(["-O", "stop"]);
        self.destination(&mut command);
        let output = self.output(command, CONTROL_TIMEOUT).await;
        if !pinned.is_current()? {
            return Ok(true);
        }
        let output = output?;
        if !output.status.success() {
            bail!(
                "couldn't stop the shared SSH connection to {}: {}",
                self.target,
                failure_message(&output.status, &output.stderr)
            );
        }
        Ok(true)
    }

    /// Runs `exit` on the machine through the control socket. With `master_only`, a master
    /// that refuses the session fails it, rather than ssh falling back to a connection of its
    /// own, which would prove nothing about the master.
    async fn check(&self, control_path: &Path, master_only: bool) -> Result<()> {
        let mut command = self.control_command(control_path);
        if master_only {
            command.args(["-o", "ProxyCommand=false"]);
        }
        command.arg("-T");
        self.destination(&mut command);
        command.arg("exit");
        let output = self.output(command, COMMAND_TIMEOUT).await?;
        if !output.status.success() {
            bail!("{}", failure_message(&output.status, &output.stderr));
        }
        Ok(())
    }

    /// Where `ssh` dials the machine once `~/.ssh/config` applies (`ssh -G`): what links an SSH
    /// machine to the cloud machine it is.
    pub async fn address(&self) -> Result<SshAddress> {
        let mut command = tokio::process::Command::new(&self.program);
        command.arg("-G");
        self.destination(&mut command);
        let output = self.output(command, CONTROL_TIMEOUT).await?;
        if !output.status.success() {
            bail!("{}", failure_message(&output.status, &output.stderr));
        }
        let config = String::from_utf8_lossy(&output.stdout);
        let value = |key: &str| {
            config.lines().find_map(|line| {
                let (name, value) = line.split_once(' ')?;
                (name == key).then(|| value.trim())
            })
        };
        let host = value("hostname").context("ssh -G named no hostname")?;
        let port = match value("port") {
            Some(port) => port
                .parse()
                .with_context(|| format!("ssh -G named the port {port:?}"))?,
            None => 22,
        };
        Ok(SshAddress {
            host: host.to_string(),
            port,
        })
    }

    /// Where the master's socket is once ssh expands `%C` for the machine (`ssh -G`).
    async fn expanded_control_path(&self, control_path: &Path) -> Option<PathBuf> {
        let mut command = tokio::process::Command::new(&self.program);
        command
            .arg("-o")
            .arg(format!("ControlPath={}", control_path.display()))
            .arg("-G");
        self.destination(&mut command);
        let output = match self.output(command, CONTROL_TIMEOUT).await {
            Ok(output) if output.status.success() => output,
            Ok(output) => {
                log::warn!(
                    "ssh -G {} failed: {}",
                    self.target,
                    failure_message(&output.status, &output.stderr)
                );
                return None;
            }
            Err(error) => {
                log::warn!("ssh -G {} failed: {error:#}", self.target);
                return None;
            }
        };
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| line.strip_prefix("controlpath "))
            .map(PathBuf::from)
    }

    /// Runs the script under `/bin/sh` on the machine, whatever the user's login shell, and
    /// returns what it printed.
    pub async fn run(&self, script: &str) -> Result<String> {
        let child = self
            .command(&remote_script(script))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("running {}", self.program.display()))?;
        let output = tokio::time::timeout(COMMAND_TIMEOUT, child.wait_with_output())
            .await
            .map_err(|_| {
                anyhow!(
                    "ssh {} didn't finish within {} seconds",
                    self.target,
                    COMMAND_TIMEOUT.as_secs()
                )
            })?
            .context("waiting for ssh")?;
        let output = finished_output(output)?;
        String::from_utf8(output).context("the remote command printed invalid UTF-8")
    }

    /// The machine's platform, and the hash and size of the server installed there, in one
    /// round trip.
    async fn probe(&self) -> Result<(RemotePlatform, Option<(String, u64)>)> {
        let directory = INSTALL_DIRECTORY;
        let output = self
            .run(&format!(
                "uname -s; uname -m\n\
                 cat {directory}/agentz-server.sha256 2>/dev/null || echo\n\
                 wc -c 2>/dev/null < {directory}/agentz-server || echo"
            ))
            .await?;
        let mut lines = output.lines();
        let os = lines.next().unwrap_or_default();
        let arch = lines.next().unwrap_or_default();
        let platform = RemotePlatform::from_uname(os, arch)?;
        let hash = lines.next().map(str::trim).filter(|hash| !hash.is_empty());
        let size = lines.next().and_then(|size| size.trim().parse().ok());
        let installed = hash.zip(size).map(|(hash, size)| (hash.to_string(), size));
        Ok((platform, installed))
    }

    /// Copies the binary into place through ssh's stdin, as herdr does, then records its hash.
    /// A temporary name and a rename mean a server running from the old file keeps running.
    /// Reports how much of it ssh has taken as it goes.
    async fn install(
        &self,
        binary: Vec<u8>,
        hash: &str,
        on_progress: &(dyn Fn(UploadProgress) + Send + Sync),
    ) -> Result<()> {
        let directory = INSTALL_DIRECTORY;
        // `cat` ends the same way when ssh is cut off as when the upload is done, so what
        // arrived is checked before it's put in place.
        let script = format!(
            "set -eu\n\
             dir={directory}\n\
             mkdir -p \"$dir\"\n\
             tmp=\"$dir/agentz-server.tmp.$$\"\n\
             cat > \"$tmp\"\n\
             size=$(wc -c < \"$tmp\" | tr -d ' ')\n\
             if [ \"$size\" != {size} ]; then\n\
               rm -f \"$tmp\"\n\
               echo \"the upload was cut short ($size of {size} bytes)\" >&2\n\
               exit 1\n\
             fi\n\
             if command -v sha256sum >/dev/null 2>&1; then\n\
               sum=$(sha256sum \"$tmp\" | cut -d ' ' -f 1)\n\
             elif command -v shasum >/dev/null 2>&1; then\n\
               sum=$(shasum -a 256 \"$tmp\" | cut -d ' ' -f 1)\n\
             else\n\
               sum={hash}\n\
             fi\n\
             if [ \"$sum\" != {hash} ]; then\n\
               rm -f \"$tmp\"\n\
               echo \"the upload arrived damaged\" >&2\n\
               exit 1\n\
             fi\n\
             chmod 755 \"$tmp\"\n\
             mv \"$tmp\" \"$dir/agentz-server\"\n\
             printf '%s\\n' {hash} > \"$dir/agentz-server.sha256\"\n",
            hash = shell_quote(hash),
            size = binary.len(),
        );
        let mut child = self
            .command(&remote_script(&script))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("running {}", self.program.display()))?;
        let mut stdin = child.stdin.take().context("ssh has no stdin")?;
        let output = child.wait_with_output();
        futures::pin_mut!(output);
        let total = binary.len() as u64;
        let upload = async {
            let mut sent = 0;
            on_progress(UploadProgress { sent, total });
            for chunk in binary.chunks(UPLOAD_CHUNK_SIZE) {
                tokio::time::timeout(UPLOAD_STALL_TIMEOUT, stdin.write_all(chunk))
                    .await
                    .map_err(|_| {
                        anyhow!(
                            "uploading to {} stopped for {} seconds",
                            self.target,
                            UPLOAD_STALL_TIMEOUT.as_secs()
                        )
                    })?
                    .context("sending to ssh")?;
                sent += chunk.len() as u64;
                on_progress(UploadProgress { sent, total });
            }
            stdin.shutdown().await.context("sending to ssh")?;
            drop(stdin);
            anyhow::Ok(())
        };
        // Ssh ending before it took everything has the reason in its output. Dropping it
        // stops it (`kill_on_drop`).
        let output = tokio::select! {
            uploaded = upload => {
                uploaded.context("uploading agentz-server")?;
                tokio::time::timeout(UPLOAD_STALL_TIMEOUT, &mut output)
                    .await
                    .map_err(|_| anyhow!("installing agentz-server on {} didn't finish", self.target))?
            }
            output = &mut output => output,
        };
        finished_output(output.context("waiting for ssh")?).context("uploading agentz-server")?;
        Ok(())
    }

    /// Starts `agentz-server proxy` on the machine, which starts the server if it isn't
    /// running, and returns the session as a byte stream once the proxy is about to run.
    async fn proxy(&self) -> Result<SshStream> {
        let script = format!("exec {INSTALL_DIRECTORY}/agentz-server proxy");
        let mut child = self
            .command(&remote_script(&script))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("running {}", self.program.display()))?;
        let stdin = child.stdin.take().context("ssh's stdin")?;
        let stdout = child.stdout.take().context("ssh's stdout")?;
        let mut stderr = child.stderr.take().context("ssh's stderr")?;
        let mut reader = tokio::io::BufReader::new(stdout);
        let ready = tokio::time::timeout(PROXY_READY_TIMEOUT, async {
            let mut line = Vec::new();
            loop {
                line.clear();
                if reader.read_until(b'\n', &mut line).await? == 0 {
                    return anyhow::Ok(false);
                }
                if line.strip_suffix(b"\n").map(trim_carriage_return)
                    == Some(OUTPUT_READY_MARKER.as_bytes())
                {
                    return Ok(true);
                }
            }
        })
        .await
        .map_err(|_| anyhow!("ssh {} didn't start the proxy in time", self.target))??;
        if !ready {
            let mut error = Vec::new();
            (&mut stderr)
                .take(STDERR_LIMIT as u64)
                .read_to_end(&mut error)
                .await
                .ok();
            let status = child.wait().await.context("waiting for ssh")?;
            bail!("{}", failure_message(&status, &error));
        }
        let target = self.target.clone();
        tokio::spawn(async move {
            let mut lines = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                log::warn!("ssh {target}: {line}");
            }
        });
        Ok(SshStream {
            _child: child,
            reader,
            writer: stdin,
        })
    }
}

/// A port on this machine forwarded to the same port on the machine, as `ssh -L` does. A login
/// page on the machine sends the browser back to `localhost` there, where the agent waits.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LocalForward {
    /// `localhost`, `127.0.0.1` or `::1`, as the page names it: on both ends, so the browser
    /// finds the port where it looks and reaches the agent where it listens.
    pub host: String,
    pub port: u16,
}

impl LocalForward {
    /// `-L`'s argument.
    fn specification(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("{host}:{port}:{host}:{port}", port = self.port)
    }
}

/// How long a forward stays once nobody holds it: the browser may still be loading the page the
/// agent sends it to once the login is done.
const FORWARD_LINGER: Duration = Duration::from_secs(30);

/// The forwards held through [`Ssh::hold_forward`], by machine: how many hold each, and which
/// release last let go of it, so only the latest one's cancel runs.
static HELD_FORWARDS: std::sync::LazyLock<
    parking_lot::Mutex<collections::HashMap<(String, LocalForward), (usize, u64)>>,
> = std::sync::LazyLock::new(Default::default);

/// A forward someone needs. Dropping it cancels the forward a while after the last holder
/// does.
pub struct HeldForward {
    ssh: Ssh,
    forward: LocalForward,
    runtime: tokio::runtime::Handle,
}

impl Drop for HeldForward {
    fn drop(&mut self) {
        let key = (self.ssh.target.clone(), self.forward.clone());
        let release = {
            let mut held = HELD_FORWARDS.lock();
            let Some((holders, release)) = held.get_mut(&key) else {
                return;
            };
            *holders = holders.saturating_sub(1);
            if *holders > 0 {
                return;
            }
            *release += 1;
            *release
        };
        let ssh = self.ssh.clone();
        let forward = self.forward.clone();
        self.runtime.spawn(async move {
            tokio::time::sleep(FORWARD_LINGER).await;
            let still_released = {
                let mut held = HELD_FORWARDS.lock();
                let unchanged = held.get(&key) == Some(&(0, release));
                if unchanged {
                    held.remove(&key);
                }
                unchanged
            };
            if still_released && let Err(error) = ssh.cancel_forward(&forward).await {
                log::warn!(
                    "couldn't stop forwarding port {} from {}: {error:#}",
                    forward.port,
                    ssh.target
                );
            }
        });
    }
}

impl Ssh {
    /// Forwards the port until the returned holder and every other one for it are dropped.
    /// Call it on `runtime`.
    pub async fn hold_forward(
        &self,
        forward: LocalForward,
        runtime: tokio::runtime::Handle,
    ) -> Result<HeldForward> {
        let key = (self.target.clone(), forward.clone());
        HELD_FORWARDS.lock().entry(key.clone()).or_default().0 += 1;
        let held = HeldForward {
            ssh: self.clone(),
            forward: forward.clone(),
            runtime,
        };
        // Asked again even when held: the connection may have started over since.
        self.forward(&forward).await?;
        Ok(held)
    }

    /// Adds the forward to the shared connection (`ssh -O forward`), which keeps it until it's
    /// cancelled or the connection ends. Asking for one it has already succeeds.
    pub async fn forward(&self, forward: &LocalForward) -> Result<()> {
        match self.control("forward", forward).await {
            Ok(()) => Ok(()),
            // The master couldn't listen on it here.
            Err(error) if format!("{error}").contains("Port forwarding failed") => {
                bail!("port {} is in use on this Mac", forward.port)
            }
            Err(error) => Err(error)
                .with_context(|| format!("forwarding port {} from {}", forward.port, self.target)),
        }
    }

    pub async fn cancel_forward(&self, forward: &LocalForward) -> Result<()> {
        self.control("cancel", forward).await
    }

    /// Asks the shared connection's master to forward a port, or stop.
    async fn control(&self, operation: &str, forward: &LocalForward) -> Result<()> {
        let control_path = self
            .control_path
            .as_ref()
            .context("there's no shared SSH connection to forward it through")?;
        let mut command = tokio::process::Command::new(&self.program);
        command
            .arg("-o")
            .arg(format!("ControlPath={}", control_path.display()))
            .args(["-O", operation, "-L", &forward.specification()]);
        self.destination(&mut command);
        let output = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("SSH_ASKPASS_REQUIRE", "never")
            .kill_on_drop(true)
            .output();
        let output = tokio::time::timeout(COMMAND_TIMEOUT, output)
            .await
            .map_err(|_| anyhow!("ssh {} didn't answer in time", self.target))?
            .with_context(|| format!("running {}", self.program.display()))?;
        if !output.status.success() {
            bail!("{}", failure_message(&output.status, &output.stderr));
        }
        Ok(())
    }
}

/// A session with a machine's server.
pub struct Connected {
    pub connection: Connection,
    pub events: Events,
    /// The server was already running from an older binary than the one installed now. It
    /// keeps running until someone stops it, since stopping it stops its agents.
    pub is_outdated: bool,
}

/// How much of the server ssh has taken while installing it on a machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadProgress {
    pub sent: u64,
    pub total: u64,
}

impl UploadProgress {
    pub fn percent(&self) -> u64 {
        (self.sent * 100).checked_div(self.total).unwrap_or(100)
    }
}

/// Connects to the server on the machine: checks its platform, uploads `server_binary`'s
/// choice when the installed server differs (reporting its progress), and starts the proxy.
/// Call it on the runtime.
pub async fn connect(
    ssh: &Ssh,
    version: &str,
    server_binary: impl FnOnce(RemotePlatform) -> Result<PathBuf>,
    client_kind: ClientKind,
    on_upload_progress: impl Fn(UploadProgress) + Send + Sync,
) -> Result<Connected, SshError> {
    connect_inner(
        ssh,
        version,
        server_binary,
        client_kind,
        &on_upload_progress,
    )
    .await
    .map_err(|error| SshError::classify(ssh.terminal_command(), &error))
}

async fn connect_inner(
    ssh: &Ssh,
    version: &str,
    server_binary: impl FnOnce(RemotePlatform) -> Result<PathBuf>,
    client_kind: ClientKind,
    on_upload_progress: &(dyn Fn(UploadProgress) + Send + Sync),
) -> Result<Connected> {
    // A master left from before the Mac slept would hold every command until it timed out.
    ssh.prepare_shared_connection().await?;
    let (platform, installed) = ssh.probe().await?;
    let path = server_binary(platform)?;
    let binary = tokio::fs::read(&path)
        .await
        .with_context(|| format!("reading {}", path.display()))?;
    let hash = sha256_hex(&binary);
    // The size too: an upload cut short by an older client was kept with the whole one's hash.
    if installed != Some((hash.clone(), binary.len() as u64)) {
        log::info!(
            "installing agentz-server {version} for {platform} on {}",
            ssh.target()
        );
        ssh.install(binary, &hash, on_upload_progress).await?;
    }
    let stream = ssh.proxy().await?;
    let (connection, events) = Connection::new(
        &tokio::runtime::Handle::current(),
        stream,
        client_kind,
        version.to_string(),
        crate::REMOTE_WELCOME_TIMEOUT,
    )
    .await?;
    let is_outdated = connection.welcome().build.as_deref() != Some(hash.as_str());
    if is_outdated {
        log::info!(
            "{} runs an older agentz-server (pid {})",
            ssh.target(),
            connection.welcome().pid
        );
    }
    Ok(Connected {
        connection,
        events,
        is_outdated,
    })
}

/// What a remote command printed, once it succeeded.
fn finished_output(output: std::process::Output) -> Result<Vec<u8>> {
    if !output.status.success() {
        bail!("{}", failure_message(&output.status, &output.stderr));
    }
    discard_preamble(&output.stdout)
}

/// The command ssh gives the user's login shell: `/bin/sh` running the script after the
/// marker, so the script is POSIX even when the login shell isn't (fish, xonsh).
fn remote_script(script: &str) -> String {
    let script = format!("printf '\\n%s\\n' '{OUTPUT_READY_MARKER}'\n{script}");
    format!("/bin/sh -c {}", shell_quote(&script))
}

/// Drops everything up to the marker line (herdr's `discard_remote_output_preamble`).
fn discard_preamble(output: &[u8]) -> Result<Vec<u8>> {
    let mut rest = output;
    loop {
        let (line, after) = match rest.iter().position(|byte| *byte == b'\n') {
            Some(end) => (&rest[..end], &rest[end + 1..]),
            None => (rest, &[][..]),
        };
        if trim_carriage_return(line) == OUTPUT_READY_MARKER.as_bytes() {
            return Ok(after.to_vec());
        }
        if after.is_empty() {
            bail!("the remote command exited before it started");
        }
        rest = after;
    }
}

fn trim_carriage_return(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\r").unwrap_or(line)
}

fn failure_message(status: &std::process::ExitStatus, stderr: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(&stderr[..stderr.len().min(STDERR_LIMIT)]);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        format!("ssh exited with {status}")
    } else {
        stderr.to_string()
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// `/tmp/agentz-ssh-<uid>`, private to this user, for the control sockets. It's `/tmp` rather
/// than the temp directory because macOS's is too long for a socket path (herdr's
/// `shared_ssh_control_path`).
fn ssh_program() -> PathBuf {
    std::env::var_os(SSH_PROGRAM_ENV_VAR)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("ssh"))
}

/// The control socket, `%C` being the hash of the host, port and user ssh ends up with, so
/// machines behind one gateway each get their own.
fn shared_control_path() -> Option<PathBuf> {
    match control_directory() {
        Ok(directory) => Some(directory.join("%C")),
        Err(error) => {
            log::warn!("connecting without a shared SSH connection: {error:#}");
            None
        }
    }
}

fn control_directory() -> Result<PathBuf> {
    use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, PermissionsExt as _};

    // SAFETY: geteuid has no preconditions and can't fail.
    let uid = unsafe { libc::geteuid() };
    let directory = PathBuf::from(format!("/tmp/agentz-ssh-{uid}"));
    if directory.as_os_str().len() + 1 + CONTROL_PATH_EXPANSION > MAX_CONTROL_PATH_LENGTH {
        bail!("{} is too long for a socket path", directory.display());
    }
    match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error).with_context(|| format!("creating {}", directory.display()));
        }
    }
    let metadata = std::fs::symlink_metadata(&directory)
        .with_context(|| format!("reading {}", directory.display()))?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.permissions().mode() & 0o077 != 0 {
        bail!(
            "{} isn't a private directory of this user",
            directory.display()
        );
    }
    Ok(directory)
}

/// A second name for a master's socket (herdr's `PinnedSshControlSocket`). A command sent to
/// it reaches the master that was checked even once a new master has taken the socket's
/// name, so stopping a stale master can't stop its replacement.
struct PinnedControlSocket {
    alias: PathBuf,
    original: PathBuf,
    device: u64,
    inode: u64,
}

impl PinnedControlSocket {
    fn new(path: &Path) -> std::io::Result<Self> {
        use std::os::unix::fs::{DirBuilderExt as _, FileTypeExt as _, MetadataExt as _};

        static NEXT_PIN: AtomicU64 = AtomicU64::new(0);
        let parent = path.parent().ok_or(std::io::ErrorKind::InvalidInput)?;
        let metadata = std::fs::symlink_metadata(path)?;
        // SAFETY: geteuid has no preconditions and can't fail.
        let uid = unsafe { libc::geteuid() };
        if !metadata.file_type().is_socket() || metadata.uid() != uid {
            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("{} isn't a socket of this user", path.display()),
            ));
        }
        let directory = parent.join(format!(
            "r-{}-{}",
            std::process::id(),
            NEXT_PIN.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&directory)?;
        let pinned = Self {
            alias: directory.join("ctl"),
            original: path.to_path_buf(),
            device: metadata.dev(),
            inode: metadata.ino(),
        };
        std::fs::hard_link(path, &pinned.alias)?;
        let linked = std::fs::symlink_metadata(&pinned.alias)?;
        if (linked.dev(), linked.ino()) != (pinned.device, pinned.inode) {
            return Err(std::io::ErrorKind::NotFound.into());
        }
        Ok(pinned)
    }

    /// Whether the socket's name still belongs to the master that was pinned.
    fn is_current(&self) -> std::io::Result<bool> {
        use std::os::unix::fs::MetadataExt as _;

        match std::fs::symlink_metadata(&self.original) {
            Ok(metadata) => Ok((metadata.dev(), metadata.ino()) == (self.device, self.inode)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }
}

impl Drop for PinnedControlSocket {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.alias) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => log::warn!("removing {}: {error}", self.alias.display()),
        }
        if let Some(directory) = self.alias.parent()
            && let Err(error) = std::fs::remove_dir(directory)
        {
            log::warn!("removing {}: {error}", directory.display());
        }
    }
}

/// The proxy's stdout and stdin as one stream. ssh is killed when it's dropped.
struct SshStream {
    _child: Child,
    reader: tokio::io::BufReader<ChildStdout>,
    writer: ChildStdin,
}

impl AsyncRead for SshStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.reader).poll_read(cx, buf)
    }
}

impl AsyncWrite for SshStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.writer).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.writer).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.writer).poll_shutdown(cx)
    }
}

/// Where a server binary for another machine comes from: `agentz-server-<rust target>` in
/// `directory` (next to the app, or in a bundle's resources).
pub fn bundled_server_binary(directory: &Path, platform: RemotePlatform) -> PathBuf {
    directory.join(format!("agentz-server-{}", platform.rust_target()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_uname() {
        assert_eq!(
            RemotePlatform::from_uname("Linux\n", "x86_64\n").expect("parses"),
            RemotePlatform {
                os: RemoteOs::Linux,
                arch: RemoteArch::X86_64
            }
        );
        assert_eq!(
            RemotePlatform::from_uname("Darwin", "arm64")
                .expect("parses")
                .rust_target(),
            "aarch64-apple-darwin"
        );
        assert!(RemotePlatform::from_uname("FreeBSD", "amd64").is_err());
    }

    #[test]
    fn validates_targets() {
        for target in ["devbox1", "me@host", "ssh://me@host:2222"] {
            assert!(validate_target(target).is_ok(), "{target}");
        }
        for target in ["", "-oProxyCommand=x", "a b", "a\nb"] {
            assert!(validate_target(target).is_err(), "{target:?}");
        }
    }

    #[test]
    fn quotes_for_the_shell() {
        assert_eq!(shell_quote("plain-word_1.2"), "plain-word_1.2");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote(""), "''");
    }

    #[test]
    fn drops_what_startup_files_print() {
        let output = format!("Welcome!\r\n\n{OUTPUT_READY_MARKER}\r\nLinux\nx86_64\n");
        assert_eq!(
            discard_preamble(output.as_bytes()).expect("finds the marker"),
            b"Linux\nx86_64\n"
        );
        assert!(discard_preamble(b"no marker\n").is_err());
    }

    #[test]
    fn sorts_failures() {
        let classify = |message: &str| SshError::classify("ssh devbox1", &anyhow!("{message}"));
        let refused = classify("ssh: connect to host devbox1 port 22: Connection refused");
        assert!(!refused.needs_attention);
        assert!(refused.hint.is_none());
        let timeout = classify("ssh: connect to host devbox1 port 22: Operation timed out");
        assert!(!timeout.needs_attention);
        let authentication = classify("me@devbox1: Permission denied (publickey).");
        assert!(authentication.needs_attention);
        assert!(authentication.hint.expect("hint").contains("ssh-add"));
        let host_key = classify("Host key verification failed.");
        assert!(host_key.needs_attention);
        assert!(host_key.hint.expect("hint").contains("ssh devbox1"));
        assert!(classify("unsupported remote platform: FreeBSD amd64").needs_attention);
    }

    #[test]
    fn a_providers_options_come_before_the_target() {
        let options = SshOptions {
            host_name: "ssh.proton.computer".into(),
            port: Some(22021),
            user: Some("ubuntu".into()),
            options: vec![("StrictHostKeyChecking".into(), "yes".into())],
        };
        let ssh = Ssh::with_program_and_options(
            "ssh".into(),
            "proton-devbox-3",
            &options,
            "proton ssh devbox-3",
        )
        .expect("valid options");
        let command = ssh.command("exit");
        let args: Vec<String> = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let target = args
            .iter()
            .position(|arg| arg == "proton-devbox-3")
            .expect("has the target");
        assert_eq!(
            args[target - 8..target],
            [
                "-o",
                "HostName=ssh.proton.computer",
                "-o",
                "Port=22021",
                "-o",
                "User=ubuntu",
                "-o",
                "StrictHostKeyChecking=yes",
            ]
        );
        assert_eq!(args.last().map(String::as_str), Some("exit"));
        let host_key = SshError::classify(
            ssh.terminal_command(),
            &anyhow!("Host key verification failed."),
        );
        assert!(
            host_key
                .hint
                .expect("hint")
                .contains("`proton ssh devbox-3`")
        );
    }

    /// `ssh` splits an option's value at spaces, and agentZ's own key is in "Application
    /// Support".
    #[test]
    fn an_option_with_spaces_is_quoted() {
        let options = SshOptions {
            host_name: "ssh.proton.computer".into(),
            options: vec![
                (
                    "IdentityFile".into(),
                    "/Users/me/Library/Application Support/agentZ/cloud/id_ed25519".into(),
                ),
                ("IdentityFile".into(), "~/.ssh/id_ed25519".into()),
                ("CertificateFile".into(), r#"/tmp/a "b\c"#.into()),
            ],
            ..Default::default()
        };
        let ssh = Ssh::with_program_and_options("ssh".into(), "x", &options, "x").expect("valid");
        assert_eq!(
            ssh.options,
            [
                "HostName=ssh.proton.computer",
                r#"IdentityFile="/Users/me/Library/Application Support/agentZ/cloud/id_ed25519""#,
                "IdentityFile=~/.ssh/id_ed25519",
                r#"CertificateFile="/tmp/a \"b\\c""#,
            ]
        );
    }

    #[test]
    fn rejects_options_that_could_inject_more() {
        let options = SshOptions {
            host_name: "gateway".into(),
            options: vec![("ProxyCommand".into(), "sh\nevil".into())],
            ..Default::default()
        };
        assert!(Ssh::with_program_and_options("ssh".into(), "x", &options, "x").is_err());
        let options = SshOptions {
            host_name: "-oProxyCommand=evil".into(),
            ..Default::default()
        };
        assert!(Ssh::with_program_and_options("ssh".into(), "x", &options, "x").is_err());
    }

    const ANSWERS: &str = "exit 0";
    const BROKEN_MASTER: &str =
        "echo 'mux_client_request_session: read from master failed: Broken pipe' >&2; exit 255";
    const LOCKED: &str = "echo 'me@devbox: Permission denied (publickey).' >&2; exit 255";

    /// Runs [`Ssh::prepare_shared_connection`] with a stand-in for `ssh` that records what it's
    /// asked, a master that answers its check with `master`, and a connection of its own with
    /// `fresh`.
    async fn prepare_with(
        scratch: &Path,
        name: &str,
        master: &str,
        fresh: &str,
        has_master: bool,
    ) -> (Result<bool>, Vec<String>) {
        use std::os::unix::fs::PermissionsExt as _;

        let socket = control_directory()
            .expect("control directory")
            .join(format!("test-{}-{name}", std::process::id()));
        let listener = has_master
            .then(|| std::os::unix::net::UnixListener::bind(&socket).expect("binds the socket"));
        let log = scratch.join(format!("{name}.log"));
        let program = scratch.join(format!("ssh-{name}"));
        let script = format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> '{log}'
case " $* " in
  *" -G "*) echo 'controlpath {socket}' ;;
  *" -O "*) ;;
  *ControlPath=none*) {fresh} ;;
  *ProxyCommand=false*) {master} ;;
esac
"#,
            log = log.display(),
            socket = socket.display(),
        );
        std::fs::write(&program, script).expect("writes the fake ssh");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let ssh = Ssh::with_program(program, "devbox").expect("valid target");
        let prepared = ssh.prepare_shared_connection().await;
        drop(listener);
        if has_master {
            std::fs::remove_file(&socket).expect("removes the socket");
        }
        let calls = std::fs::read_to_string(&log)
            .expect("ssh ran")
            .lines()
            .map(str::to_string)
            .collect();
        (prepared, calls)
    }

    fn control_path_of(call: &str) -> PathBuf {
        call.split_whitespace()
            .find_map(|argument| argument.strip_prefix("ControlPath="))
            .map(PathBuf::from)
            .expect("names a control path")
    }

    /// `ssh -G`'s host name and port, as an SSH machine is linked to a provider's by them.
    #[tokio::test]
    async fn reads_the_address_ssh_dials() {
        use std::os::unix::fs::PermissionsExt as _;

        let scratch = tempfile::tempdir().expect("temp dir");
        let program = scratch.path().join("ssh");
        std::fs::write(
            &program,
            "#!/bin/sh\nprintf 'user root\\nhostname ssh.proton.computer\\nport 22021\\nhostkeyalias x\\n'\n",
        )
        .expect("writes the fake ssh");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let ssh = Ssh::with_program(program, "proton").expect("valid target");
        assert_eq!(
            ssh.address().await.expect("reads the address"),
            SshAddress {
                host: "ssh.proton.computer".into(),
                port: 22021,
            }
        );
    }

    /// herdr's recovery of a stuck shared connection: a master that answers is kept, one that
    /// doesn't is stopped (never exited) once a connection of its own works, and a machine
    /// that can't be reached at all is reported as such.
    #[tokio::test]
    async fn retires_a_shared_connection_that_stopped_answering() {
        let scratch = tempfile::tempdir().expect("temp dir");
        let pins = control_directory()
            .expect("control directory")
            .join(format!("r-{}-", std::process::id()));
        let pins = pins.to_string_lossy();
        let is_pinned = |path: &Path| path.to_string_lossy().starts_with(&*pins);

        let (prepared, calls) =
            prepare_with(scratch.path(), "healthy", ANSWERS, LOCKED, true).await;
        assert!(!prepared.expect("checks"));
        assert_eq!(calls.len(), 2, "{calls:?}");
        assert!(calls[0].contains(" -G devbox"), "{calls:?}");
        assert!(calls[1].contains("ProxyCommand=false"), "{calls:?}");
        assert!(calls[1].ends_with("-T devbox exit"), "{calls:?}");
        assert!(is_pinned(&control_path_of(&calls[1])), "{calls:?}");

        let (prepared, calls) =
            prepare_with(scratch.path(), "stale", BROKEN_MASTER, ANSWERS, true).await;
        assert!(prepared.expect("recovers"));
        assert_eq!(calls.len(), 4, "{calls:?}");
        assert_eq!(control_path_of(&calls[2]), Path::new("none"));
        assert!(calls[3].ends_with("-O stop devbox"), "{calls:?}");
        let alias = control_path_of(&calls[3]);
        assert!(is_pinned(&alias), "{calls:?}");
        assert!(!alias.exists());
        assert!(!alias.parent().expect("pin directory").exists());
        assert!(!calls.iter().any(|call| call.contains("-O exit")));

        let (prepared, calls) =
            prepare_with(scratch.path(), "locked", BROKEN_MASTER, LOCKED, true).await;
        let error = prepared.expect_err("can't connect");
        assert!(
            format!("{error:#}").contains("Permission denied"),
            "{error:#}"
        );
        assert!(!calls.iter().any(|call| call.contains(" -O ")), "{calls:?}");

        let (prepared, calls) =
            prepare_with(scratch.path(), "no-master", BROKEN_MASTER, LOCKED, false).await;
        assert!(prepared.expect("checks"));
        assert_eq!(calls.len(), 1, "{calls:?}");
    }
}
