//! Handing the terminals and agents to a newly installed server, so updating the server keeps
//! them running (herdr's live handoff, extended to ACP agents). The old server starts the new
//! binary with one end of a socket pair as its stdin, sends it a manifest, then the listening
//! socket, each terminal's PTY and each agent's pipes as file descriptors. The new server takes
//! them over and says it's ready; the old one says to commit, and exits without ending the
//! terminals' or agents' processes. Until the commit either side can give up, and the old
//! server carries on as before.

use std::io::{IoSlice, IoSliceMut, Read as _, Write as _};
use std::os::fd::{AsFd as _, AsRawFd as _, BorrowedFd, FromRawFd as _, OwnedFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_thread::AgentSnapshot;
use agentz_protocol::terminal::TerminalKey;
use anyhow::{Context as _, Result, anyhow, bail};
use nix::sys::socket::{ControlMessage, ControlMessageOwned, MsgFlags, recvmsg, sendmsg};
use projects::ThreadId;
use serde::{Deserialize, Serialize};

use crate::terminals::TerminalHandoff;

const READY: u8 = b'R';
/// Ready, having taken over the agents too. A server from before agents were handed off answers
/// [`READY`] and ignores them, so they end with the old server, as they did then.
const READY_WITH_AGENTS: u8 = b'A';
const COMMIT: u8 = b'C';
/// The new server loads the state before it's ready, which may take a while on a busy machine.
const READY_TIMEOUT: Duration = Duration::from_secs(60);
const COMMIT_TIMEOUT: Duration = Duration::from_secs(10);
/// Screens with their scrollback are the bulk of it.
const MAX_MANIFEST_SIZE: u64 = 1 << 30;

#[derive(Serialize, Deserialize)]
pub(crate) struct Manifest {
    /// In the order their PTYs follow the listening socket.
    pub terminals: Vec<HandedOffTerminal>,
    #[serde(default)]
    pub palette: Option<Vec<[u8; 3]>>,
    /// In the order their pipes follow the PTYs.
    #[serde(default)]
    pub agents: Vec<HandedOffThread>,
    /// Messages for threads to send their agents when their turns end.
    #[serde(default)]
    pub follow_ups: Vec<(ThreadId, Vec<crate::server::FollowUp>)>,
}

/// A thread whose agent the new server goes on talking to.
#[derive(Serialize, Deserialize)]
pub(crate) struct HandedOffThread {
    pub thread_id: ThreadId,
    pub agent: AgentSnapshot,
    /// The credential its agent's MCP bridge calls the server's tools with.
    #[serde(default)]
    pub token: Option<String>,
}

/// An agent's stdin, stdout and stderr.
pub(crate) struct AgentPipes {
    pub stdin: OwnedFd,
    pub stdout: OwnedFd,
    pub stderr: OwnedFd,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct HandedOffTerminal {
    pub key: TerminalKey,
    pub terminal: TerminalHandoff,
    #[serde(default)]
    pub output_byte_limit: Option<usize>,
    #[serde(default)]
    pub released: bool,
    #[serde(default)]
    pub folder: Option<PathBuf>,
}

/// The terminals and agents a new server takes over, each with its PTY or pipes.
pub struct HandedOver {
    pub(crate) manifest: Manifest,
    pub(crate) ptys: Vec<OwnedFd>,
    pub(crate) agent_pipes: Vec<AgentPipes>,
}

/// The new server's side of the handoff, to say when it's ready.
pub struct Takeover {
    connection: UnixStream,
}

/// The old server's side, once the new one is ready.
pub(crate) struct Handover {
    connection: UnixStream,
}

/// Reads what the old server sends on `connection`: the listening socket and the terminals.
pub fn receive(connection: UnixStream) -> Result<(UnixListener, HandedOver, Takeover)> {
    let mut length = [0u8; 8];
    (&connection)
        .read_exact(&mut length)
        .context("reading the handoff's manifest")?;
    let length = u64::from_be_bytes(length);
    if length > MAX_MANIFEST_SIZE {
        bail!("the handoff's manifest is too large ({length} bytes)");
    }
    let mut manifest = vec![0; length as usize];
    (&connection)
        .read_exact(&mut manifest)
        .context("reading the handoff's manifest")?;
    let manifest: Manifest =
        serde_json::from_slice(&manifest).context("parsing the handoff's manifest")?;
    let listener = UnixListener::from(receive_fd(&connection).context("receiving the socket")?);
    let ptys = manifest
        .terminals
        .iter()
        .map(|_| receive_fd(&connection).context("receiving a terminal"))
        .collect::<Result<Vec<_>>>()?;
    let agent_pipes = manifest
        .agents
        .iter()
        .map(|_| {
            Ok(AgentPipes {
                stdin: receive_fd(&connection).context("receiving an agent")?,
                stdout: receive_fd(&connection).context("receiving an agent")?,
                stderr: receive_fd(&connection).context("receiving an agent")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok((
        listener,
        HandedOver {
            manifest,
            ptys,
            agent_pipes,
        },
        Takeover { connection },
    ))
}

impl Takeover {
    /// Tells the old server the terminals and agents are taken over, and waits for it to let
    /// them go.
    pub fn ready(self) -> Result<()> {
        (&self.connection)
            .write_all(&[READY_WITH_AGENTS])
            .context("telling the old server")?;
        self.connection
            .set_read_timeout(Some(COMMIT_TIMEOUT))
            .context("waiting for the old server")?;
        let mut answer = [0u8; 1];
        (&self.connection)
            .read_exact(&mut answer)
            .context("the old server didn't let the terminals go")?;
        if answer[0] != COMMIT {
            bail!("the old server answered {:?}", answer[0]);
        }
        Ok(())
    }
}

impl Handover {
    /// Tells the new server it runs the terminals now.
    pub(crate) fn commit(self) -> Result<()> {
        (&self.connection)
            .write_all(&[COMMIT])
            .context("telling the new server to take over")
    }
}

/// Starts `executable` to take over, and sends it the listening socket, the terminals and the
/// agents. Returns once it's ready, with whether it took the agents. A new server that isn't
/// ready is stopped.
#[allow(
    clippy::disallowed_methods,
    reason = "runs on a blocking thread, and the new server's stdin must be the socket"
)]
pub(crate) fn send(
    executable: &Path,
    listener: OwnedFd,
    manifest: &Manifest,
    ptys: Vec<OwnedFd>,
    agent_pipes: Vec<AgentPipes>,
) -> Result<(Handover, bool)> {
    let manifest = serde_json::to_vec(manifest).context("encoding the handoff's manifest")?;
    let (connection, theirs) = UnixStream::pair().context("connecting to the new server")?;
    let mut child = std::process::Command::new(executable)
        .args(["run", "--handoff"])
        .stdin(OwnedFd::from(theirs))
        // Its own process group, as `agentz-server start` gives the server.
        .process_group(0)
        .spawn()
        .with_context(|| format!("starting {}", executable.display()))?;
    let result = (|| {
        (&connection)
            .write_all(&(manifest.len() as u64).to_be_bytes())
            .and_then(|()| (&connection).write_all(&manifest))
            .context("sending the handoff's manifest")?;
        send_fd(&connection, listener.as_fd()).context("sending the socket")?;
        for pty in &ptys {
            send_fd(&connection, pty.as_fd()).context("sending a terminal")?;
        }
        for pipes in &agent_pipes {
            for pipe in [&pipes.stdin, &pipes.stdout, &pipes.stderr] {
                send_fd(&connection, pipe.as_fd()).context("sending an agent")?;
            }
        }
        connection
            .set_read_timeout(Some(READY_TIMEOUT))
            .context("waiting for the new server")?;
        let mut answer = [0u8; 1];
        (&connection)
            .read_exact(&mut answer)
            .context("the new server didn't take over; see its log")?;
        match answer[0] {
            READY => Ok(false),
            READY_WITH_AGENTS => Ok(true),
            answer => bail!("the new server answered {answer:?}"),
        }
    })();
    match result {
        Ok(took_agents) => Ok((Handover { connection }, took_agents)),
        Err(error) => {
            if child.try_wait().ok().flatten().is_none() {
                child.kill().ok();
            }
            child.wait().ok();
            Err(error)
        }
    }
}

fn send_fd(connection: &UnixStream, fd: BorrowedFd<'_>) -> Result<()> {
    let fds = [fd.as_raw_fd()];
    let byte = [0u8];
    sendmsg::<()>(
        connection.as_raw_fd(),
        &[IoSlice::new(&byte)],
        &[ControlMessage::ScmRights(&fds)],
        MsgFlags::empty(),
        None,
    )?;
    Ok(())
}

fn receive_fd(connection: &UnixStream) -> Result<OwnedFd> {
    let mut byte = [0u8];
    let mut buffers = [IoSliceMut::new(&mut byte)];
    let mut space = nix::cmsg_space!(RawFd);
    let message = recvmsg::<()>(
        connection.as_raw_fd(),
        &mut buffers,
        Some(&mut space),
        MsgFlags::empty(),
    )?;
    let mut received = None;
    for message in message.cmsgs()? {
        if let ControlMessageOwned::ScmRights(fds) = message {
            for fd in fds {
                // SAFETY: the kernel just gave this process the descriptor, and nothing else
                // holds it.
                let fd = unsafe { OwnedFd::from_raw_fd(fd) };
                if received.is_none() {
                    received = Some(fd);
                }
            }
        }
    }
    let fd = received.ok_or_else(|| anyhow!("the old server sent no file descriptor"))?;
    // Not for the terminals and agents this server starts. macOS has no `MSG_CMSG_CLOEXEC`.
    // SAFETY: only sets the descriptor's flag.
    if unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return Err(std::io::Error::last_os_error()).context("receiving a file descriptor");
    }
    Ok(fd)
}
