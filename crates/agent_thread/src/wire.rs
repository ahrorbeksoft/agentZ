//! The agent's pipes under the ACP SDK. Lines pass through unchanged, and what is still
//! unanswered either way is noted, so the connection can be paused between messages and
//! handed to another server in the middle of a turn: the requests the agent waits on are
//! received again by the new connection, and the answer to the prompt in flight is taken out
//! for the thread, since the new SDK connection never sent it.

use std::collections::{HashMap, HashSet, VecDeque};
use std::os::fd::{AsFd as _, OwnedFd};
use std::sync::{Arc, Mutex};

use anyhow::{Context as _, Result};
use futures::channel::{mpsc, oneshot};
use futures::future::BoxFuture;
use futures::{FutureExt as _, StreamExt as _};
use serde_json::Value;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::unix::pipe;
use tokio::task::JoinHandle;

/// The session of the message that marks where a pause took effect. No agent sends it; it
/// goes through the SDK after every line the agent sent before the pause.
pub(crate) const PAUSE_MARKER_SESSION: &str = "agentz/paused";
/// Marks where a taken answer arrived, so the thread ends the turn after what came before it.
pub(crate) const ANSWER_MARKER_SESSION: &str = "agentz/answered";

/// A line no agent wrote, which the SDK passes on as a session update for `session`.
pub(crate) fn marker_line(session: &str) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "session/update",
        "params": {
            "sessionId": session,
            "update": {
                "sessionUpdate": "agent_message_chunk",
                "content": {"type": "text", "text": ""},
            },
        },
    })
    .to_string()
}

/// Lines for the SDK to read, from the agent's stdout.
pub(crate) type IncomingLines = mpsc::UnboundedSender<std::io::Result<String>>;
/// Lines the SDK writes, for the agent's stdin.
pub(crate) type OutgoingLines = mpsc::UnboundedReceiver<String>;

/// What's unanswered on the connection, by request id (as JSON text).
#[derive(Default)]
struct WireState {
    /// Requests sent to the agent: their method.
    sent: HashMap<String, String>,
    /// Requests from the agent, oldest first, with their lines.
    received: Vec<(String, String)>,
    /// Requests another connection sent, whose answers are taken out rather than passed to
    /// the SDK.
    taken: HashSet<String>,
    /// Those answers, until the thread asks for them.
    answers: VecDeque<Value>,
}

impl WireState {
    fn note_sent(&mut self, line: &str) {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return;
        };
        let Some(id) = message.get("id").map(Value::to_string) else {
            return;
        };
        match message.get("method").and_then(Value::as_str) {
            Some(method) => {
                self.sent.insert(id, method.to_string());
            }
            None => self.received.retain(|(received, _)| *received != id),
        }
    }

    /// Notes a line from the agent. Returns whether the answer was taken out, rather than
    /// going to the SDK.
    fn note_received(&mut self, line: &str) -> bool {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        let Some(id) = message.get("id").map(Value::to_string) else {
            return false;
        };
        if message.get("method").is_some() {
            self.received.push((id, line.to_string()));
            return false;
        }
        self.sent.remove(&id);
        if !self.taken.remove(&id) {
            return false;
        }
        self.answers.push_back(message);
        true
    }
}

/// The agent's stdout or stderr, with what was read past the last full line.
pub(crate) struct LineReader {
    pipe: pipe::Receiver,
    rest: Vec<u8>,
}

/// The agent's stdin, with the SDK's lines not yet written to it.
pub(crate) struct LineWriter {
    pipe: pipe::Sender,
    lines: OutgoingLines,
    queued: VecDeque<String>,
}

/// The wire's ends while it doesn't run.
pub(crate) struct Parked {
    stdin: LineWriter,
    stdout: LineReader,
    stderr: LineReader,
    incoming: IncomingLines,
    on_stderr: Arc<dyn Fn(String) + Send + Sync>,
    on_exit: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl Parked {
    /// The SDK channel lines are read into, to send it a line no agent wrote.
    pub(crate) fn incoming(&self) -> IncomingLines {
        self.incoming.clone()
    }
}

/// What [`Wire::hand_off`] gives another server: the pipes, and the state to go with them.
pub struct WireHandoff {
    pub stdin: OwnedFd,
    pub stdout: OwnedFd,
    pub stderr: OwnedFd,
    pub stdout_rest: Vec<u8>,
    pub stderr_rest: Vec<u8>,
    /// The prompt the agent works on, by its JSON-RPC id.
    pub prompt_id: Option<Value>,
    /// The requests the agent waits on an answer to, as they arrived.
    pub unanswered: Vec<String>,
}

struct Running {
    stops: Vec<oneshot::Sender<()>>,
    stdin: JoinHandle<LineWriter>,
    stdout: JoinHandle<LineReader>,
    stderr: JoinHandle<LineReader>,
    incoming: IncomingLines,
    on_stderr: Arc<dyn Fn(String) + Send + Sync>,
    on_exit: Option<Arc<dyn Fn() + Send + Sync>>,
}

pub(crate) struct Wire {
    state: Arc<Mutex<WireState>>,
    running: Option<Running>,
    parked: Option<Parked>,
}

impl Wire {
    /// A wire between the agent's pipes and the SDK, paused until [`Self::resume`]. `on_exit`
    /// reports the agent's end when nothing else does: an adopted agent isn't this process's
    /// child.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        runtime: &tokio::runtime::Handle,
        stdin: OwnedFd,
        stdout: OwnedFd,
        stderr: OwnedFd,
        rests: (Vec<u8>, Vec<u8>),
        incoming: IncomingLines,
        outgoing: OutgoingLines,
        on_stderr: Arc<dyn Fn(String) + Send + Sync>,
        on_exit: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self> {
        // Pipes are made in the runtime's context, which registers them with its reactor.
        let _guard = runtime.enter();
        let parked = Parked {
            stdin: LineWriter {
                pipe: pipe::Sender::from_owned_fd(stdin).context("opening the agent's stdin")?,
                lines: outgoing,
                queued: VecDeque::new(),
            },
            stdout: LineReader {
                pipe: pipe::Receiver::from_owned_fd(stdout)
                    .context("opening the agent's stdout")?,
                rest: rests.0,
            },
            stderr: LineReader {
                pipe: pipe::Receiver::from_owned_fd(stderr)
                    .context("opening the agent's stderr")?,
                rest: rests.1,
            },
            incoming,
            on_stderr,
            on_exit,
        };
        Ok(Self {
            state: Arc::default(),
            running: None,
            parked: Some(parked),
        })
    }

    /// Takes out the answer to a prompt sent before this connection was made. A marker
    /// (see [`ANSWER_MARKER_SESSION`]) follows it through the SDK; then it's
    /// [`Self::taken_answer`].
    pub(crate) fn take_answer(&self, id: &Value) {
        let mut state = lock(&self.state);
        state
            .sent
            .insert(id.to_string(), "session/prompt".to_string());
        state.taken.insert(id.to_string());
    }

    pub(crate) fn taken_answer(&self) -> Option<Value> {
        lock(&self.state).answers.pop_front()
    }

    /// Passes requests the agent sent an earlier connection to the SDK, which answers them.
    pub(crate) fn receive_again(&self, lines: Vec<String>) {
        let Some(incoming) = self
            .running
            .as_ref()
            .map(|running| running.incoming.clone())
            .or_else(|| self.parked.as_ref().map(|parked| parked.incoming.clone()))
        else {
            return;
        };
        let mut state = lock(&self.state);
        for line in lines {
            if !state.note_received(&line) {
                incoming.unbounded_send(Ok(line)).ok();
            }
        }
    }

    pub(crate) fn is_paused(&self) -> bool {
        self.parked.is_some()
    }

    /// Whether the connection could be handed over as it is: no request is in flight but at
    /// most the prompt being worked on.
    pub(crate) fn prompt_in_flight(&self) -> Result<Option<Value>, &'static str> {
        let state = lock(&self.state);
        let mut prompt = None;
        for (id, method) in &state.sent {
            if method != "session/prompt" || prompt.is_some() {
                return Err("a request to the agent is in flight");
            }
            prompt = Some(id.clone());
        }
        Ok(prompt.and_then(|id| serde_json::from_str(&id).ok()))
    }

    /// Stops passing lines between messages. Resolves once every task has stopped; pass the
    /// result to [`Self::park`].
    pub(crate) fn pause(&mut self) -> BoxFuture<'static, Option<Parked>> {
        let Some(running) = self.running.take() else {
            return futures::future::ready(None).boxed();
        };
        for stop in running.stops {
            stop.send(()).ok();
        }
        async move {
            let stdin = running.stdin.await.ok()?;
            let stdout = running.stdout.await.ok()?;
            let stderr = running.stderr.await.ok()?;
            Some(Parked {
                stdin,
                stdout,
                stderr,
                incoming: running.incoming,
                on_stderr: running.on_stderr,
                on_exit: running.on_exit,
            })
        }
        .boxed()
    }

    pub(crate) fn park(&mut self, parked: Parked) {
        self.parked = Some(parked);
    }

    pub(crate) fn resume(&mut self, runtime: &tokio::runtime::Handle) {
        let Some(parked) = self.parked.take() else {
            return;
        };
        let (stop_stdin, stdin_stopped) = oneshot::channel();
        let (stop_stdout, stdout_stopped) = oneshot::channel();
        let (stop_stderr, stderr_stopped) = oneshot::channel();
        let stdin = runtime.spawn(write_lines(parked.stdin, self.state.clone(), stdin_stopped));
        let stdout = runtime.spawn(read_agent_lines(
            parked.stdout,
            self.state.clone(),
            parked.incoming.clone(),
            parked.on_exit.clone(),
            stdout_stopped,
        ));
        let on_stderr = parked.on_stderr.clone();
        let stderr = runtime.spawn(async move {
            let mut stderr = parked.stderr;
            let mut stopped = stderr_stopped;
            read_lines(&mut stderr, &mut stopped, |line| on_stderr(line)).await;
            stderr
        });
        self.running = Some(Running {
            stops: vec![stop_stdin, stop_stdout, stop_stderr],
            stdin,
            stdout,
            stderr,
            incoming: parked.incoming,
            on_stderr: parked.on_stderr,
            on_exit: parked.on_exit,
        });
    }

    /// Copies what a paused connection is handed over with. Fails when the SDK wrote anything
    /// since it paused, or a request other than the prompt is in flight: then the connection
    /// should resume here.
    pub(crate) fn hand_off(&mut self) -> Result<WireHandoff> {
        let parked = self
            .parked
            .as_mut()
            .context("the connection isn't paused")?;
        while let Ok(line) = parked.stdin.lines.try_recv() {
            parked.stdin.queued.push_back(line);
        }
        anyhow::ensure!(
            parked.stdin.queued.is_empty(),
            "the agent was sent something while pausing"
        );
        let prompt_id = self.prompt_in_flight().map_err(anyhow::Error::msg)?;
        let parked = self
            .parked
            .as_ref()
            .context("the connection isn't paused")?;
        let unanswered = lock(&self.state)
            .received
            .iter()
            .map(|(_, line)| line.clone())
            .collect();
        Ok(WireHandoff {
            stdin: parked.stdin.pipe.as_fd().try_clone_to_owned()?,
            stdout: parked.stdout.pipe.as_fd().try_clone_to_owned()?,
            stderr: parked.stderr.pipe.as_fd().try_clone_to_owned()?,
            stdout_rest: parked.stdout.rest.clone(),
            stderr_rest: parked.stderr.rest.clone(),
            prompt_id,
            unanswered,
        })
    }
}

impl Drop for Wire {
    fn drop(&mut self) {
        if let Some(running) = self.running.take() {
            running.stdin.abort();
            running.stdout.abort();
            running.stderr.abort();
        }
    }
}

fn lock(state: &Mutex<WireState>) -> std::sync::MutexGuard<'_, WireState> {
    // The state stays consistent line by line, so a panic elsewhere can't leave it half done.
    state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Writes the SDK's lines to the agent, one whole line at a time, until stopped.
async fn write_lines(
    mut writer: LineWriter,
    state: Arc<Mutex<WireState>>,
    mut stopped: oneshot::Receiver<()>,
) -> LineWriter {
    loop {
        let line = match writer.queued.pop_front() {
            Some(line) => line,
            None => {
                tokio::select! {
                    biased;
                    _ = &mut stopped => return writer,
                    line = writer.lines.next() => match line {
                        Some(line) => line,
                        // The SDK is gone.
                        None => return writer,
                    },
                }
            }
        };
        lock(&state).note_sent(&line);
        let mut bytes = line.into_bytes();
        bytes.push(b'\n');
        if let Err(error) = writer.pipe.write_all(&bytes).await {
            log::warn!("failed to write to the agent: {error}");
            return writer;
        }
    }
}

/// Passes the agent's lines to the SDK, taking out answers meant for the thread.
async fn read_agent_lines(
    mut reader: LineReader,
    state: Arc<Mutex<WireState>>,
    incoming: IncomingLines,
    on_exit: Option<Arc<dyn Fn() + Send + Sync>>,
    mut stopped: oneshot::Receiver<()>,
) -> LineReader {
    let closed = read_lines(&mut reader, &mut stopped, |line| {
        let taken = lock(&state).note_received(&line);
        let line = if taken {
            marker_line(ANSWER_MARKER_SESSION)
        } else {
            line
        };
        incoming.unbounded_send(Ok(line)).ok();
    })
    .await;
    if closed && let Some(on_exit) = on_exit {
        on_exit();
    }
    reader
}

/// Reads lines until stopped or the pipe closes, which it returns. A partial line stays in
/// the reader, so stopping loses nothing.
async fn read_lines(
    reader: &mut LineReader,
    stopped: &mut oneshot::Receiver<()>,
    mut on_line: impl FnMut(String),
) -> bool {
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        while let Some(end) = reader.rest.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = reader.rest.drain(..=end).collect();
            let line = String::from_utf8_lossy(&line[..end]);
            on_line(line.trim_end_matches('\r').to_string());
        }
        tokio::select! {
            biased;
            _ = &mut *stopped => return false,
            // Reading is cancel safe: bytes are only taken when the read completes.
            read = reader.pipe.read(&mut chunk) => match read {
                Ok(0) => return true,
                Ok(count) => reader.rest.extend_from_slice(&chunk[..count]),
                Err(error) => {
                    log::warn!("failed to read from the agent: {error}");
                    return true;
                }
            },
        }
    }
}
