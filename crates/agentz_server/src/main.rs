//! `agentz-server`: the background process that runs agents for agentZ. It listens on a socket
//! in the data directory; the app starts it with `start`, and clients on other machines reach it
//! through `proxy` over SSH.

use std::io::IsTerminal as _;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use agentz_protocol::{
    ClientHello, ClientKind, ClientMessage, PROTOCOL_VERSION, Request, ServerMessage,
    ServerWelcome, read_message, write_message,
};
use agentz_server::ServerConfig;
use anyhow::{Context as _, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use futures::FutureExt as _;
use reqwest_client::ReqwestClient;
use tokio::net::{UnixListener, UnixStream};
use util::ResultExt as _;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const START_TIMEOUT: Duration = Duration::from_secs(10);
/// A log over this size is cleared when the server next starts in the background.
const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

#[derive(Parser)]
#[command(
    name = "agentz-server",
    version,
    about = "Runs agents for agentZ in the background"
)]
struct Arguments {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Serves in the foreground. The default.
    Run,
    /// Starts the server in the background unless it's running, and returns once it accepts
    /// connections.
    Start,
    /// Connects stdin and stdout to the server, starting it if needed.
    Proxy,
    /// Asks the running server to stop its agents and exit.
    Stop,
}

fn main() {
    // The login-shell environment capture re-runs this binary with `--printenv`.
    if std::env::args().any(|argument| argument == "--printenv") {
        util::shell_env::print_env();
        return;
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let arguments = Arguments::parse();
    let result = match arguments.command.unwrap_or(Command::Run) {
        Command::Run => run(),
        Command::Start => start(),
        Command::Proxy => start().and_then(|()| block_on(proxy())),
        Command::Stop => block_on(stop()),
    };
    if let Err(error) = result {
        log::error!("{error:#}");
        std::process::exit(1);
    }
}

fn block_on(future: impl Future<Output = Result<()>>) -> Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the runtime")?
        .block_on(future)
}

fn run() -> Result<()> {
    let socket = paths::server_socket();
    prepare_socket_path(&socket)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the runtime")?;
    runtime.block_on(serve(&socket))
}

async fn serve(socket: &Path) -> Result<()> {
    let listener =
        UnixListener::bind(socket).with_context(|| format!("listening on {}", socket.display()))?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("restricting {}", socket.display()))?;
    let pid_file = paths::server_pid_file();
    std::fs::write(&pid_file, std::process::id().to_string())
        .with_context(|| format!("writing {}", pid_file.display()))?;

    // Started from the app or by SSH, the environment lacks the user's `PATH`.
    let started_from_terminal = std::io::stdout().is_terminal();
    let shell_environment_ready = tokio::spawn(async move {
        if !started_from_terminal && let Err(error) = util::load_login_shell_environment().await {
            log::error!("failed to load the login shell environment: {error:#}");
        }
    })
    .map(|_| ())
    .boxed()
    .shared();
    let http_client = match ReqwestClient::user_agent(&format!("agentZ/{VERSION}")) {
        Ok(client) => Arc::new(client),
        Err(error) => {
            log::error!("failed to create the HTTP client: {error:#}");
            Arc::new(ReqwestClient::new())
        }
    };
    let server = agentz_server::start(
        tokio::runtime::Handle::current(),
        ServerConfig {
            data_dir: paths::data_dir().clone(),
            version: VERSION.to_string(),
            http_client,
            shell_environment_ready,
            custom_agents: Default::default(),
        },
    )?;
    log::info!(
        "agentz-server {VERSION} listening on {} (pid {})",
        socket.display(),
        std::process::id()
    );

    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .context("handling SIGTERM")?;
    loop {
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _)) => server.serve(stream),
                Err(error) => log::error!("failed to accept a connection: {error}"),
            },
            _ = server.stopped() => break,
            _ = tokio::signal::ctrl_c() => server.shut_down(),
            _ = terminate.recv() => server.shut_down(),
        }
    }

    std::fs::remove_file(socket).log_err();
    if std::fs::read_to_string(&pid_file).ok().as_deref() == Some(&std::process::id().to_string()) {
        std::fs::remove_file(&pid_file).log_err();
    }
    log::info!("stopped");
    Ok(())
}

/// Removes a socket left behind by a server that died, and refuses to start next to a live one
/// (herdr's `prepare_socket_path`).
fn prepare_socket_path(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    if !path.exists() {
        return Ok(());
    }
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => bail!("a server is already listening on {}", path.display()),
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionRefused
                    | std::io::ErrorKind::NotFound
                    | std::io::ErrorKind::TimedOut
            ) => {}
        Err(error) => {
            return Err(error).with_context(|| format!("connecting to {}", path.display()));
        }
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("removing {}", path.display())),
    }
}

/// Starts the server in the background unless one is listening, and waits until it is.
#[allow(
    clippy::disallowed_methods,
    reason = "runs before any async runtime, so there's nothing to block"
)]
fn start() -> Result<()> {
    let socket = paths::server_socket();
    if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
        return Ok(());
    }
    let log_path = paths::server_log_file();
    let log = open_log(&log_path)?;
    let executable = std::env::current_exe().context("finding this executable")?;
    let mut child = std::process::Command::new(executable)
        .arg("run")
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().context("opening the log")?)
        .stderr(log)
        // Its own process group, so signals for the app's or the terminal's don't reach it.
        .process_group(0)
        .spawn()
        .context("starting the server")?;

    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
            return Ok(());
        }
        if let Some(status) = child.try_wait().context("waiting for the server")? {
            bail!("the server exited ({status}); see {}", log_path.display());
        }
        if Instant::now() >= deadline {
            bail!(
                "the server didn't start listening within {} seconds; see {}",
                START_TIMEOUT.as_secs(),
                log_path.display()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn open_log(path: &Path) -> Result<std::fs::File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    let too_large = std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > MAX_LOG_SIZE);
    std::fs::OpenOptions::new()
        .create(true)
        .append(!too_large)
        .write(true)
        .truncate(too_large)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))
}

/// Copies stdin to the server and the server's messages to stdout.
async fn proxy() -> Result<()> {
    let socket = paths::server_socket();
    let stream = UnixStream::connect(&socket)
        .await
        .with_context(|| format!("connecting to {}", socket.display()))?;
    let (mut from_server, mut to_server) = stream.into_split();
    let mut stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    tokio::select! {
        copied = tokio::io::copy(&mut stdin, &mut to_server) => copied?,
        copied = tokio::io::copy(&mut from_server, &mut stdout) => copied?,
    };
    Ok(())
}

async fn stop() -> Result<()> {
    let socket = paths::server_socket();
    let Ok(stream) = UnixStream::connect(&socket).await else {
        log::info!("no server is running");
        return Ok(());
    };
    let (mut reader, mut writer) = stream.into_split();
    let hello = ClientHello {
        protocol_version: PROTOCOL_VERSION,
        client_version: VERSION.to_string(),
        client_kind: ClientKind::Cli,
    };
    write_message(&mut writer, &hello).await?;
    let welcome: ServerWelcome = read_message(&mut reader)
        .await?
        .context("the server closed the connection")?;
    if let Some(error) = welcome.error {
        bail!("the server refused the connection: {error}");
    }
    let request = ClientMessage::Request {
        id: 1,
        request: Request::Shutdown,
    };
    write_message(&mut writer, &request).await?;
    // The server closes the connection once it has stopped.
    while let Some(message) = read_message::<ServerMessage>(&mut reader).await? {
        if let ServerMessage::Response { id: 1, result } = message {
            result.map_err(|error| anyhow!("the server refused to stop: {}", error.message))?;
        }
    }
    // It removes its socket just after.
    let deadline = Instant::now() + START_TIMEOUT;
    while socket.exists() {
        if Instant::now() >= deadline {
            bail!("the server didn't exit (pid {})", welcome.pid);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    log::info!("stopped the server (pid {})", welcome.pid);
    Ok(())
}
