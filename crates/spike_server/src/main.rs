//! Phase 0 spike: a GPUI-free binary with the server's real dependencies.
//!
//! - `http [url]`: an HTTPS request through `reqwest_client`.
//! - `pty`: `sh` in an `alacritty_terminal` PTY, then the screen.
//! - `acp <python> <mock_agent.py>`: the mock agent over ACP on tokio, given this binary as a
//!   stdio MCP server in `session/new`.
//! - `mcp-bridge`: a minimal stdio MCP server with one tool.

use std::{
    collections::HashMap,
    path::PathBuf,
    process::Stdio,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1 as acp;
use agent_client_protocol::{Agent, Client, ConnectionTo, Lines};
use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    event_loop::{EventLoop, Msg},
    grid::Dimensions,
    index::{Column, Line, Point},
    sync::FairMutex,
    term::{Config, Term},
    tty,
};
use anyhow::{Context as _, Result, anyhow, bail};
use futures::{AsyncReadExt as _, StreamExt as _};
use http_client::{AsyncBody, HttpClient as _};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("http") => http(args.get(1).map(String::as_str)),
        Some("pty") => pty(),
        Some("acp") => {
            let python = args.get(1).context("missing python")?;
            let script = args.get(2).context("missing mock_agent.py")?;
            runtime()?.block_on(acp_with_mcp(python.into(), script.clone()))
        }
        Some("mcp-bridge") => runtime()?.block_on(mcp_bridge()),
        _ => bail!("usage: spike_server http [url] | pty | acp <python> <script> | mcp-bridge"),
    }
}

fn runtime() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?)
}

fn http(url: Option<&str>) -> Result<()> {
    let url = url.unwrap_or("https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json");
    let client = reqwest_client::ReqwestClient::user_agent("agentZ-spike")?;
    let started = Instant::now();
    let body = futures::executor::block_on(async {
        let mut response = client.get(url, AsyncBody::empty(), true).await?;
        let status = response.status();
        let mut body = String::new();
        response.body_mut().read_to_string(&mut body).await?;
        anyhow::Ok(format!("{status}, {} bytes", body.len()))
    })?;
    println!("HTTP OK: {url}: {body} in {:?}", started.elapsed());
    Ok(())
}

struct Size {
    lines: usize,
    columns: usize,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.lines
    }

    fn screen_lines(&self) -> usize {
        self.lines
    }

    fn columns(&self) -> usize {
        self.columns
    }
}

#[derive(Clone)]
struct Listener(mpsc::Sender<Event>);

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        self.0.send(event).ok();
    }
}

fn pty() -> Result<()> {
    let size = Size {
        lines: 24,
        columns: 80,
    };
    let (events_tx, events_rx) = mpsc::channel();
    let options = tty::Options {
        shell: Some(tty::Shell::new("/bin/sh".into(), Vec::new())),
        working_directory: None,
        drain_on_exit: true,
        env: HashMap::from([("PS1".into(), "$ ".into())]),
        child_signal_mask: None,
    };
    let window_size = WindowSize {
        num_lines: size.lines as u16,
        num_cols: size.columns as u16,
        cell_width: 8,
        cell_height: 16,
    };
    let pty = tty::new(&options, window_size, 0).context("opening the PTY")?;
    let term = Arc::new(FairMutex::new(Term::new(
        Config::default(),
        &size,
        Listener(events_tx.clone()),
    )));
    let event_loop = EventLoop::new(term.clone(), Listener(events_tx), pty, true, false)?;
    let sender = event_loop.channel();
    let _io_thread = event_loop.spawn();

    let input = "echo hello-from-pty; uname -sm; printf '\\033[1;32mgreen\\033[0m\\n'; exit\n";
    sender
        .send(Msg::Input(input.as_bytes().to_vec().into()))
        .map_err(|error| anyhow!("{error:?}"))?;

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .context("timed out waiting for sh to exit")?;
        match events_rx.recv_timeout(remaining)? {
            Event::ChildExit(status) => {
                println!("child exited: {status:?}");
                break;
            }
            Event::PtyWrite(text) => sender
                .send(Msg::Input(text.into_bytes().into()))
                .map_err(|error| anyhow!("{error:?}"))?,
            _ => {}
        }
    }

    let term = term.lock();
    let screen = term.bounds_to_string(
        Point::new(Line(0), Column(0)),
        Point::new(Line(size.lines as i32 - 1), Column(size.columns - 1)),
    );
    println!("--- screen ---\n{}\n--------------", screen.trim_end());
    anyhow::ensure!(
        screen.contains("hello-from-pty"),
        "missing output on screen"
    );
    println!("PTY OK");
    Ok(())
}

async fn acp_with_mcp(python: PathBuf, script: String) -> Result<()> {
    let mut child = tokio::process::Command::new(&python)
        .arg(&script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .with_context(|| format!("starting {}", python.display()))?;
    let stdin = child.stdin.take().context("agent has no stdin")?;
    let stdout = child.stdout.take().context("agent has no stdout")?;

    let incoming_lines = futures::stream::unfold(
        BufReader::new(stdout).lines(),
        async |mut lines| match lines.next_line().await {
            Ok(Some(line)) => Some((Ok(line), lines)),
            Ok(None) => None,
            Err(error) => Some((Err(error), lines)),
        },
    )
    .boxed();
    let outgoing_lines = Box::pin(futures::sink::unfold(
        stdin,
        async move |mut writer, line: String| {
            writer.write_all(line.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
            Ok::<_, std::io::Error>(writer)
        },
    ));

    let (notification_tx, mut notification_rx) =
        futures::channel::mpsc::unbounded::<acp::SessionNotification>();
    let (connection_tx, connection_rx) = futures::channel::oneshot::channel();
    let connection_future = Client
        .builder()
        .name("agentZ-spike")
        .on_receive_notification(
            async move |notification: acp::SessionNotification, _connection| {
                notification_tx.unbounded_send(notification).ok();
                Ok(())
            },
            agent_client_protocol::on_receive_notification!(),
        )
        .connect_with(
            Lines::new(outgoing_lines, incoming_lines),
            move |connection: ConnectionTo<Agent>| async move {
                connection_tx.send(connection).ok();
                futures::future::pending::<Result<(), agent_client_protocol::Error>>().await
            },
        );
    tokio::spawn(async move {
        if let Err(error) = connection_future.await {
            eprintln!("ACP connection error: {error:?}");
        }
    });
    let connection = connection_rx.await?;

    connection
        .send_request(acp::InitializeRequest::new(ProtocolVersion::V1))
        .block_task()
        .await
        .map_err(|error| anyhow!("initialize: {error:?}"))?;

    let bridge = std::env::current_exe()?;
    let mcp_server = acp::McpServer::Stdio(
        acp::McpServerStdio::new("agentz", bridge)
            .args(vec!["mcp-bridge".into()])
            .env(vec![acp::EnvVariable::new(
                "AGENTZ_SESSION_TOKEN",
                "spike-token",
            )]),
    );
    let session = connection
        .send_request(
            acp::NewSessionRequest::new(std::env::temp_dir()).mcp_servers(vec![mcp_server]),
        )
        .block_task()
        .await
        .map_err(|error| anyhow!("session/new: {error:?}"))?;

    let prompt = connection
        .send_request(acp::PromptRequest::new(
            session.session_id,
            vec![acp::ContentBlock::Text(acp::TextContent::new("mcp"))],
        ))
        .block_task();
    let response = tokio::time::timeout(Duration::from_secs(10), prompt)
        .await
        .context("timed out waiting for the prompt")?
        .map_err(|error| anyhow!("session/prompt: {error:?}"))?;

    let mut reply = String::new();
    while let Ok(notification) = notification_rx.try_recv() {
        if let acp::SessionUpdate::AgentMessageChunk(chunk) = notification.update
            && let acp::ContentBlock::Text(text) = chunk.content
        {
            reply.push_str(&text.text);
        }
    }
    println!("agent replied: {reply:?} ({:?})", response.stop_reason);
    anyhow::ensure!(
        reply.contains("pong") && reply.contains("token: yes"),
        "the agent didn't relay the MCP tool result"
    );
    println!("ACP+MCP OK");
    Ok(())
}

/// Speaks MCP's stdio transport: newline-delimited JSON-RPC.
async fn mcp_bridge() -> Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let message: serde_json::Value = serde_json::from_str(&line)?;
        let Some(id) = message.get("id").cloned() else {
            continue;
        };
        let result = match message["method"].as_str() {
            Some("initialize") => serde_json::json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "agentz", "version": env!("CARGO_PKG_VERSION")},
            }),
            Some("tools/list") => serde_json::json!({"tools": [{
                "name": "agentz_ping",
                "description": "Replies pong, to check the bridge works.",
                "inputSchema": {"type": "object", "properties": {}},
            }]}),
            Some("tools/call") => {
                let token = std::env::var("AGENTZ_SESSION_TOKEN").is_ok();
                let text = format!(
                    "pong from pid {} on {}-{} (token: {})",
                    std::process::id(),
                    std::env::consts::OS,
                    std::env::consts::ARCH,
                    if token { "yes" } else { "no" },
                );
                serde_json::json!({"content": [{"type": "text", "text": text}]})
            }
            _ => {
                let error = serde_json::json!({"jsonrpc": "2.0", "id": id,
                    "error": {"code": -32601, "message": "method not found"}});
                stdout.write_all(format!("{error}\n").as_bytes()).await?;
                stdout.flush().await?;
                continue;
            }
        };
        let response = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": result});
        stdout.write_all(format!("{response}\n").as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}
