//! `agentz-server`: the background process that runs agents for agentZ. It listens on a socket
//! in the data directory; the app starts it with `start`, and clients on other machines reach it
//! through `proxy` over SSH.

use std::io::IsTerminal as _;
use std::os::fd::{AsFd as _, AsRawFd as _};
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::CommandExt as _;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use agentz_protocol::{
    ClientHello, ClientKind, ClientMessage, PROTOCOL_VERSION, Request, Response, ServerMessage,
    ServerWelcome, ToolCaller, read_message, write_message,
};
use agentz_server::handoff::{HandedOver, Takeover};
use agentz_server::{AgentControl, ServerConfig};
use anyhow::{Context as _, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use futures::FutureExt as _;
use projects::ThreadId;
use reqwest_client::ReqwestClient;
use tokio::net::{UnixListener, UnixStream};
use util::ResultExt as _;

mod mcp_bridge;

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
    Run {
        /// Takes over from the server that started this one, which hands over its socket and
        /// terminals on stdin.
        #[arg(long, hide = true)]
        handoff: bool,
    },
    /// Starts the server in the background unless it's running, and returns once it accepts
    /// connections.
    Start,
    /// Connects stdin and stdout to the server, starting it if needed.
    Proxy,
    /// Asks the running server to stop its agents and exit.
    Stop,
    /// Serves the agent-control tools to an agent over MCP's stdio transport. agentZ gives it
    /// to every agent session, with its credential in `AGENTZ_MCP_TOKEN`.
    McpBridge,
    /// Prints the agent-control tools, as MCP tool definitions in JSON.
    Tools,
    /// Calls an agent-control tool and prints its JSON result, for agents without MCP and for
    /// scripts. The tools manage the threads of the caller's project: the thread in
    /// `AGENTZ_THREAD_ID` (set for agentZ's agents), or else the project containing the current
    /// directory.
    Call {
        /// A tool name from `tools`, such as `agentz_thread_list`.
        tool: String,
        /// The tool's arguments as a JSON object.
        arguments: Option<String>,
        /// Call as this thread instead of `AGENTZ_THREAD_ID`.
        #[arg(long)]
        thread: Option<u64>,
    },
    /// Stands in for `program` (`xdg-open`, …) for agents on a machine reached over SSH: while
    /// the agent logs in, the page goes to agentZ's clients, to open where the user is.
    /// Otherwise it runs the real program, the next one on `PATH` outside `skip`.
    #[command(hide = true)]
    OpenUrl {
        #[arg(long)]
        program: String,
        #[arg(long)]
        skip: std::path::PathBuf,
        #[arg(last = true)]
        arguments: Vec<String>,
    },
}

fn main() {
    // The login-shell environment capture re-runs this binary with `--printenv`.
    if std::env::args().any(|argument| argument == "--printenv") {
        util::shell_env::print_env();
        return;
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let arguments = Arguments::parse();
    let result = match arguments.command.unwrap_or(Command::Run { handoff: false }) {
        Command::Run { handoff } => run(handoff),
        Command::Start => start(),
        Command::Proxy => start().and_then(|()| run_proxy()),
        Command::Stop => block_on(stop()),
        Command::McpBridge => block_on(mcp_bridge::run(VERSION)),
        Command::Tools => block_on(tools()),
        Command::Call {
            tool,
            arguments,
            thread,
        } => block_on(call(tool, arguments, thread)),
        Command::OpenUrl {
            program,
            skip,
            arguments,
        } => open_url(&program, &skip, arguments),
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

/// What the server before this one handed over.
struct Handoff {
    listener: std::os::unix::net::UnixListener,
    handed_over: HandedOver,
    takeover: Takeover,
}

fn run(handoff: bool) -> Result<()> {
    let socket = paths::server_socket();
    let handoff = if handoff {
        let connection = std::io::stdin()
            .as_fd()
            .try_clone_to_owned()
            .context("reading the handoff")?;
        let (listener, handed_over, takeover) =
            agentz_server::handoff::receive(std::os::unix::net::UnixStream::from(connection))?;
        Some(Handoff {
            listener,
            handed_over,
            takeover,
        })
    } else {
        prepare_socket_path(&socket)?;
        None
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("starting the runtime")?;
    runtime.block_on(serve(&socket, handoff))
}

async fn serve(socket: &Path, handoff: Option<Handoff>) -> Result<()> {
    let (listener, handed_over, takeover) = match handoff {
        Some(handoff) => {
            handoff
                .listener
                .set_nonblocking(true)
                .context("taking over the socket")?;
            let listener =
                UnixListener::from_std(handoff.listener).context("taking over the socket")?;
            (listener, Some(handoff.handed_over), Some(handoff.takeover))
        }
        None => {
            let listener = UnixListener::bind(socket)
                .with_context(|| format!("listening on {}", socket.display()))?;
            std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("restricting {}", socket.display()))?;
            write_pid_file()?;
            (listener, None, None)
        }
    };

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
            custom_agents: agentz_server::load_custom_agents(paths::data_dir())
                .log_err()
                .unwrap_or_default(),
            agent_control: Some(AgentControl {
                executable: std::env::current_exe().context("finding this executable")?,
                socket: socket.to_path_buf(),
            }),
            hands_pages_to_clients: agentz_server::browser::hands_pages_to_clients(),
            terminal_shell: None,
            listener: Some(listener.as_raw_fd()),
            handed_over,
        },
    )?;
    if let Some(takeover) = takeover {
        let ready = tokio::task::spawn_blocking(move || takeover.ready())
            .await
            .map_err(anyhow::Error::from)
            .and_then(|ready| ready);
        if let Err(error) = ready {
            log::error!("failed to take over from the server before: {error:#}");
            // At once, without dropping the terminals: their processes are the old server's
            // again.
            std::process::exit(1);
        }
        log::info!("took over from the server before");
        write_pid_file()?;
    }
    let pid_file = paths::server_pid_file();
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

    // A server that took over listens on it now.
    if !server.handed_off() {
        std::fs::remove_file(socket).log_err();
    }
    if std::fs::read_to_string(&pid_file).ok().as_deref() == Some(&std::process::id().to_string()) {
        std::fs::remove_file(&pid_file).log_err();
    }
    log::info!("stopped");
    Ok(())
}

/// Written as soon as clients can connect, which `start` waits for.
fn write_pid_file() -> Result<()> {
    let pid_file = paths::server_pid_file();
    std::fs::write(&pid_file, std::process::id().to_string())
        .with_context(|| format!("writing {}", pid_file.display()))
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

fn run_proxy() -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the runtime")?;
    let result = runtime.block_on(proxy());
    // Stdin is read on a blocking thread that only returns when the client sends more or
    // hangs up. Waiting for it would keep the session open after the server has gone.
    runtime.shutdown_background();
    result
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

/// The socket of the server the tools come from: the one that started the calling agent, or
/// else this user's.
fn control_socket() -> std::path::PathBuf {
    std::env::var_os("AGENTZ_SOCKET")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(paths::server_socket)
}

pub(crate) async fn connect_for_tools(
    client_kind: ClientKind,
) -> Result<agentz_client::Connection> {
    let socket = control_socket();
    let (connection, mut events) = agentz_client::connect_local(
        &tokio::runtime::Handle::current(),
        &socket,
        client_kind,
        VERSION.to_string(),
    )
    .await
    .context("agentZ's server isn't running")?;
    // Answers are delivered while the events are taken.
    tokio::spawn(async move { while events.next().await.is_some() {} });
    Ok(connection)
}

async fn tools() -> Result<()> {
    let connection = connect_for_tools(ClientKind::Cli).await?;
    match connection.request(Request::ListTools).await? {
        Response::Tools(tools) => print_json(&tools),
        response => bail!("unexpected response: {response:?}"),
    }
}

async fn call(tool: String, arguments: Option<String>, thread: Option<u64>) -> Result<()> {
    let arguments = match arguments {
        Some(arguments) => {
            serde_json::from_str(&arguments).context("the arguments must be a JSON object")?
        }
        None => serde_json::Value::Object(Default::default()),
    };
    let thread = match thread {
        Some(thread) => Some(thread),
        None => match std::env::var("AGENTZ_THREAD_ID") {
            Ok(thread) => Some(
                thread
                    .parse()
                    .context("AGENTZ_THREAD_ID isn't a thread id")?,
            ),
            Err(_) => None,
        },
    };
    let caller = match thread {
        Some(thread) => ToolCaller::Thread(ThreadId(thread)),
        None => {
            ToolCaller::Directory(std::env::current_dir().context("finding the current directory")?)
        }
    };
    let connection = connect_for_tools(ClientKind::Cli).await?;
    let response = connection
        .request(Request::CallTool {
            caller,
            name: tool,
            arguments,
        })
        .await?;
    match response {
        Response::ToolResult(result) => {
            print_json(&result.value)?;
            if result.is_error {
                std::process::exit(1);
            }
            Ok(())
        }
        response => bail!("unexpected response: {response:?}"),
    }
}

fn open_url(program: &str, skip: &Path, arguments: Vec<String>) -> Result<()> {
    if let Some(url) = agentz_server::browser::page_to_hand_over(&arguments) {
        let connection = std::env::var(agentz_server::browser::CONNECTION_ENV_VAR)
            .ok()
            .and_then(|text| agentz_server::browser::connection_from_string(&text));
        if let Some(connection) = connection {
            let url = url.to_string();
            let handed_over = block_on(async move {
                let response = connect_for_tools(ClientKind::Cli)
                    .await?
                    .request(Request::OpenLoginPage { connection, url })
                    .await?;
                match response {
                    Response::Ok => Ok(()),
                    response => bail!("unexpected response: {response:?}"),
                }
            });
            match handed_over {
                Ok(()) => return Ok(()),
                // Not logging in: the page is the real program's to open.
                Err(error) => log::debug!("not handing the page to agentZ: {error:#}"),
            }
        }
    }
    let path = std::env::var_os("PATH").unwrap_or_default();
    let Some(real) = agentz_server::browser::real_program(program, skip, &path) else {
        eprintln!("{program}: no program to open {}", arguments.join(" "));
        // xdg-open's code for a missing tool.
        std::process::exit(3);
    };
    let error = std::process::Command::new(&real)
        .arg0(program)
        .args(&arguments)
        .exec();
    Err(error).with_context(|| format!("running {}", real.display()))
}

fn print_json(value: &serde_json::Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value).context("encoding the result")?;
    println!("{text}");
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
