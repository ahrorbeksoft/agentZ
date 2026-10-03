//! What `agentz-server` and its clients (the app, and later the MCP bridge and CLI) say to each
//! other, and how it's framed.
//!
//! - **Framing:** each message is JSON behind a little-endian `u32` length, as in herdr's wire
//!   protocol. It runs over any byte stream: a unix socket, or SSH's stdio through `proxy`.
//! - **Handshake:** the client sends a [`ClientHello`], the server answers with a
//!   [`ServerWelcome`].
//! - **Messages:** [`Request`]s carry ids and get one [`Response`] each. Subscriptions start
//!   with a snapshot in the response, then [`Event`]s follow.
//! - **Compatibility:** remote servers outlive client releases. New fields need serde defaults,
//!   and the enums fall back to `Unknown`, which the other side ignores or refuses. Clients turn
//!   off features the server's [`ServerWelcome::capabilities`] don't list (herdr's and t3code's
//!   rule).

pub mod agents;
pub mod thread;

use std::collections::BTreeMap;
use std::path::PathBuf;

use agent_client_protocol::schema::v1 as acp;
use anyhow::{Context as _, Result};
use projects::{ProjectIcon, ProjectId, ProjectScope, ProjectsSnapshot, ThreadId, ThreadOrder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _};

use crate::agents::{AgentId, AgentSettings, RegistrySnapshot};
use crate::thread::{ThreadUpdate, ThreadView};

/// Bumped when a change can't be made compatibly.
pub const PROTOCOL_VERSION: u32 = 1;

/// Larger frames are refused, so a bad length can't make the reader allocate without bound.
/// Long threads with big tool outputs are the largest messages.
pub const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClientHello {
    pub protocol_version: u32,
    pub client_version: String,
    pub client_kind: ClientKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientKind {
    App,
    /// `agentz-server` itself, e.g. for `stop` or `call`.
    Cli,
    /// `agentz-server mcp-bridge`, started by an agent.
    Mcp,
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ServerWelcome {
    pub protocol_version: u32,
    pub server_version: String,
    pub machine: MachineInfo,
    /// The process id, so a client can tell whether it's talking to the server it started.
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Set when the server refuses the client, e.g. for an unsupported protocol version.
    #[serde(default)]
    pub error: Option<String>,
}

/// The machine a server runs on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineInfo {
    /// Stable for the machine, whatever route reaches it (t3code's machine identity).
    pub id: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
}

/// A live agent connection on the server: a thread's, or one opened from an agent's settings
/// to log in or out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ConnectionId {
    Thread(ThreadId),
    Account(u64),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ClientMessage {
    Request {
        id: u64,
        request: Request,
    },
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ServerMessage {
    Response {
        id: u64,
        result: std::result::Result<Response, ErrorResponse>,
    },
    Event(Event),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Request {
    /// Projects, threads, the registry and agent settings: a [`Response::Session`] snapshot,
    /// then [`Event::Projects`], [`Event::Registry`] and [`Event::AgentSettings`].
    SubscribeSession,
    /// One connection's conversation: a [`Response::Thread`] snapshot, then
    /// [`Event::Thread`]s. Subscribing to a thread starts its agent if it isn't running.
    SubscribeThread(ConnectionId),
    UnsubscribeThread(ConnectionId),

    AddProject {
        path: PathBuf,
    },
    SetProjectName {
        project_id: ProjectId,
        name: String,
    },
    SetProjectIcon {
        project_id: ProjectId,
        icon: Option<ProjectIcon>,
    },
    RemoveProject(ProjectId),
    SetScope(ProjectScope),
    SetThreadOrder(ThreadOrder),
    ToggleArchivedExpanded,

    /// Answered with [`Response::ThreadCreated`].
    CreateThread {
        project_id: ProjectId,
        agent_id: AgentId,
    },
    /// The user's own title, which automatic titles no longer replace.
    RenameThread {
        thread_id: ThreadId,
        title: String,
    },
    ArchiveThread(ThreadId),
    UnarchiveThread(ThreadId),
    DeleteThread(ThreadId),

    Prompt {
        connection: ConnectionId,
        text: String,
    },
    Cancel(ConnectionId),
    RespondToPermission {
        connection: ConnectionId,
        tool_call_id: acp::ToolCallId,
        option_id: acp::PermissionOptionId,
    },
    SetConfigOption {
        connection: ConnectionId,
        config_id: acp::SessionConfigId,
        value: acp::SessionConfigOptionValue,
    },
    SetMode {
        connection: ConnectionId,
        mode_id: acp::SessionModeId,
    },
    ClearPlan(ConnectionId),
    Authenticate {
        connection: ConnectionId,
        method_id: acp::AuthMethodId,
    },
    Reauthenticate(ConnectionId),
    Logout(ConnectionId),
    RetrySession(ConnectionId),
    Reload(ConnectionId),
    /// From an account connection: checks again whether the agent is logged in.
    CheckLogin(ConnectionId),

    /// Starts an agent only to log in or out. Answered with [`Response::AccountOpened`]. It
    /// closes with [`Request::CloseAccount`], or when the client disconnects.
    OpenAccount(AgentId),
    CloseAccount(u64),

    RefreshRegistry {
        /// Only if the last refresh was over an hour ago.
        if_stale: bool,
    },
    InstallAgent(AgentId),
    UninstallAgent(AgentId),
    UpdateAgentSettings {
        agent_id: AgentId,
        change: AgentSettingsChange,
    },

    /// Ends the server, its agents and terminals.
    Shutdown,

    /// The agent-control tools, as MCP tool definitions: [`Response::Tools`].
    ListTools,
    /// Runs an agent-control tool for the caller: [`Response::ToolResult`]. Tools that wait
    /// answer when they're done.
    CallTool {
        caller: ToolCaller,
        name: String,
        arguments: serde_json::Value,
    },

    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

/// Who is calling a tool, which decides the project it may manage.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolCaller {
    /// The credential the MCP bridge got with its agent's session.
    Session(String),
    /// A thread, as the CLI knows it from `AGENTZ_THREAD_ID`.
    Thread(ThreadId),
    /// A directory inside a project, as the CLI run elsewhere knows it.
    Directory(PathBuf),
}

/// A tool's answer: its result, or for a failure `{"code", "message"}` with t3code's failure
/// codes. The MCP bridge wraps it as a `tools/call` result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub value: serde_json::Value,
    pub is_error: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AgentSettingsChange {
    SetEnv(BTreeMap<String, String>),
    SetLoginMethod(Option<String>),
    SetDefaultConfigOption {
        config_id: String,
        value: Option<acp::SessionConfigOptionValue>,
    },
    SetDefaultMode(Option<acp::SessionModeId>),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Response {
    Ok,
    Session(SessionSnapshot),
    Thread(ThreadView),
    ThreadCreated(ThreadId),
    ProjectAdded(ProjectId),
    AccountOpened(u64),
    Tools(serde_json::Value),
    ToolResult(ToolResult),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionSnapshot {
    pub projects: ProjectsSnapshot,
    pub registry: RegistrySnapshot,
    pub agent_settings: BTreeMap<AgentId, AgentSettings>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Projects(ProjectsSnapshot),
    Registry(RegistrySnapshot),
    AgentSettings(BTreeMap<AgentId, AgentSettings>),
    Thread {
        connection: ConnectionId,
        update: ThreadUpdate,
    },
    /// The connection is gone (its thread was deleted, or its account closed).
    ConnectionClosed(ConnectionId),
    /// From a newer version.
    #[serde(untagged)]
    Unknown(serde_json::Value),
}

/// Writes one length-prefixed JSON message.
pub async fn write_message(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &impl Serialize,
) -> Result<()> {
    let body = serde_json::to_vec(message).context("encoding a message")?;
    anyhow::ensure!(
        body.len() <= MAX_FRAME_SIZE,
        "message of {} bytes is over the limit",
        body.len()
    );
    let length = u32::try_from(body.len()).context("message length")?;
    writer.write_all(&length.to_le_bytes()).await?;
    writer.write_all(&body).await?;
    writer.flush().await?;
    Ok(())
}

/// Reads one length-prefixed JSON message, or `None` if the stream ended cleanly before it.
pub async fn read_message<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
) -> Result<Option<T>> {
    let mut length = [0; 4];
    match reader.read_exact(&mut length).await {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let length = u32::from_le_bytes(length) as usize;
    anyhow::ensure!(
        length <= MAX_FRAME_SIZE,
        "message of {length} bytes is over the limit"
    );
    let mut body = vec![0; length];
    reader
        .read_exact(&mut body)
        .await
        .context("reading a message")?;
    let message = serde_json::from_slice(&body).context("decoding a message")?;
    Ok(Some(message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread::Entry;

    #[tokio::test]
    async fn frames_round_trip() {
        let (mut client, mut server) = tokio::io::duplex(1024);
        let hello = ClientHello {
            protocol_version: PROTOCOL_VERSION,
            client_version: "0.1.0".into(),
            client_kind: ClientKind::App,
        };
        let request = ClientMessage::Request {
            id: 7,
            request: Request::Prompt {
                connection: ConnectionId::Thread(ThreadId(3)),
                text: "hello".into(),
            },
        };
        write_message(&mut client, &hello).await.expect("write");
        write_message(&mut client, &request).await.expect("write");
        drop(client);

        let received: Option<ClientHello> = read_message(&mut server).await.expect("read");
        assert_eq!(received, Some(hello));
        let received: Option<ClientMessage> = read_message(&mut server).await.expect("read");
        assert_eq!(received, Some(request));
        let received: Option<ClientMessage> = read_message(&mut server).await.expect("read");
        assert_eq!(received, None);
    }

    #[tokio::test]
    async fn oversized_frames_are_refused() {
        let (mut client, mut server) = tokio::io::duplex(64);
        let length = u32::try_from(MAX_FRAME_SIZE + 1).expect("fits");
        client
            .write_all(&length.to_le_bytes())
            .await
            .expect("write");
        let received = read_message::<ClientMessage>(&mut server).await;
        assert!(received.is_err());
    }

    #[test]
    fn unknown_variants_fall_back() {
        let message: ClientMessage =
            serde_json::from_str(r#"{"Request":{"id":1,"request":{"FromTheFuture":{"x":1}}}}"#)
                .expect("decodes");
        assert!(matches!(
            message,
            ClientMessage::Request {
                id: 1,
                request: Request::Unknown(_)
            }
        ));
        let message: ServerMessage =
            serde_json::from_str(r#"{"Event":{"Novel":[1]}}"#).expect("decodes");
        assert!(matches!(message, ServerMessage::Event(Event::Unknown(_))));
        let kind: ClientKind = serde_json::from_str(r#""Telepathy""#).expect("decodes");
        assert!(matches!(kind, ClientKind::Unknown(_)));
    }

    #[test]
    fn thread_events_round_trip() {
        let message = ServerMessage::Event(Event::Thread {
            connection: ConnectionId::Account(2),
            update: ThreadUpdate {
                state: None,
                entry_count: 1,
                entries: vec![(0, Entry::AgentMessage("hi".into()))],
            },
        });
        let json = serde_json::to_string(&message).expect("encodes");
        let decoded: ServerMessage = serde_json::from_str(&json).expect("decodes");
        assert_eq!(decoded, message);
    }
}
