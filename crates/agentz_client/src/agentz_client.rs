//! A connection to `agentz-server`, for the app and other clients. Requests are sent as soon as
//! they're made and answered through futures. Events arrive in order on [`Events`], which also
//! delivers the answers, so keep polling it: an answer comes only after the events the server
//! sent before it were taken, which means a client that applies events as it takes them is up
//! to date when it hears back.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agentz_protocol::{
    ClientHello, ClientKind, ClientMessage, ErrorResponse, Event, PROTOCOL_VERSION, Request,
    Response, ServerMessage, ServerWelcome, read_message, write_message,
};
use anyhow::{Context as _, Result, anyhow, bail};
use collections::HashMap;
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt as _, StreamExt as _};
use parking_lot::Mutex;
use tokio::io::{AsyncRead, AsyncWrite};

enum Incoming {
    Event(Event),
    Response {
        id: u64,
        result: Result<Response, ErrorResponse>,
    },
}

/// Ends when the connection does.
pub struct Events {
    incoming: mpsc::UnboundedReceiver<Incoming>,
    pending: PendingResponses,
}

impl Events {
    pub async fn next(&mut self) -> Option<Event> {
        loop {
            match self.incoming.next().await {
                Some(Incoming::Event(event)) => return Some(event),
                Some(Incoming::Response { id, result }) => {
                    let responder = self
                        .pending
                        .lock()
                        .as_mut()
                        .and_then(|pending| pending.remove(&id));
                    if let Some(responder) = responder {
                        responder.send(result).ok();
                    }
                }
                None => {
                    // Dropping the responders fails the requests still waiting.
                    self.pending.lock().take();
                    return None;
                }
            }
        }
    }
}

type PendingResponses =
    Arc<Mutex<Option<HashMap<u64, oneshot::Sender<Result<Response, ErrorResponse>>>>>>;

/// Cloning shares the connection.
#[derive(Clone)]
pub struct Connection {
    welcome: Arc<ServerWelcome>,
    outgoing: mpsc::UnboundedSender<ClientMessage>,
    /// `None` once the connection has closed.
    pending: PendingResponses,
    next_id: Arc<AtomicU64>,
}

impl Connection {
    /// Says hello over the stream and, if the server accepts, starts reading and writing on the
    /// runtime.
    pub async fn new(
        runtime: &tokio::runtime::Handle,
        stream: impl AsyncRead + AsyncWrite + Send + 'static,
        client_kind: ClientKind,
        client_version: String,
    ) -> Result<(Self, Events)> {
        let (mut reader, mut writer) = tokio::io::split(stream);
        let hello = ClientHello {
            protocol_version: PROTOCOL_VERSION,
            client_version,
            client_kind,
        };
        let welcome = runtime
            .spawn(async move {
                write_message(&mut writer, &hello).await?;
                let welcome: ServerWelcome = read_message(&mut reader)
                    .await?
                    .context("the server closed the connection")?;
                anyhow::Ok((welcome, reader, writer))
            })
            .await
            .context("the connection task ended")?;
        let (welcome, mut reader, mut writer) = welcome?;
        if let Some(error) = &welcome.error {
            bail!("the server refused the connection: {error}");
        }

        let (outgoing, mut outgoing_messages) = mpsc::unbounded::<ClientMessage>();
        let (incoming, incoming_receiver) = mpsc::unbounded();
        let pending: PendingResponses = Arc::new(Mutex::new(Some(HashMap::default())));
        runtime.spawn(async move {
            while let Some(message) = outgoing_messages.next().await {
                if let Err(error) = write_message(&mut writer, &message).await {
                    log::warn!("failed to write to the server: {error:#}");
                    break;
                }
            }
        });
        runtime.spawn(async move {
            loop {
                let message = match read_message::<ServerMessage>(&mut reader).await {
                    Ok(Some(ServerMessage::Response { id, result })) => {
                        Incoming::Response { id, result }
                    }
                    Ok(Some(ServerMessage::Event(event))) => Incoming::Event(event),
                    Ok(Some(ServerMessage::Unknown(message))) => {
                        log::warn!("the server sent an unknown message: {message}");
                        continue;
                    }
                    Ok(None) => break,
                    Err(error) => {
                        log::warn!("failed to read from the server: {error:#}");
                        break;
                    }
                };
                if incoming.unbounded_send(message).is_err() {
                    break;
                }
            }
        });

        let events = Events {
            incoming: incoming_receiver,
            pending: pending.clone(),
        };
        let connection = Self {
            welcome: Arc::new(welcome),
            outgoing,
            pending,
            next_id: Arc::new(AtomicU64::new(1)),
        };
        Ok((connection, events))
    }

    pub fn welcome(&self) -> &ServerWelcome {
        &self.welcome
    }

    pub fn is_closed(&self) -> bool {
        self.pending.lock().is_none()
    }

    /// Sends the request now. The future only waits for the answer, so dropping it doesn't
    /// take the request back.
    pub fn request(
        &self,
        request: Request,
    ) -> impl Future<Output = Result<Response>> + Send + 'static {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (responder, response) = oneshot::channel();
        let sent = match self.pending.lock().as_mut() {
            Some(pending) => {
                pending.insert(id, responder);
                self.outgoing
                    .unbounded_send(ClientMessage::Request { id, request })
                    .is_ok()
            }
            None => false,
        };
        async move {
            if !sent {
                bail!("not connected to the server");
            }
            match response.await {
                Ok(Ok(response)) => Ok(response),
                Ok(Err(error)) => Err(anyhow!(error.message)),
                Err(_) => Err(anyhow!("the connection to the server closed")),
            }
        }
        .boxed()
    }
}

/// Connects to the server listening on `socket`.
pub async fn connect_local(
    runtime: &tokio::runtime::Handle,
    socket: &Path,
    client_kind: ClientKind,
    client_version: String,
) -> Result<(Connection, Events)> {
    let path = socket.to_path_buf();
    let stream = runtime
        .spawn(async move { tokio::net::UnixStream::connect(&path).await })
        .await
        .context("the connection task ended")?
        .with_context(|| format!("connecting to {}", socket.display()))?;
    Connection::new(runtime, stream, client_kind, client_version).await
}

/// Runs `agentz-server start`, which returns once a server is listening.
pub async fn start_local_server(runtime: &tokio::runtime::Handle, binary: &Path) -> Result<()> {
    let mut command = tokio::process::Command::new(binary);
    command.arg("start").stdin(std::process::Stdio::null());
    let output = runtime
        .spawn(async move { command.output().await })
        .await
        .context("the start task ended")?
        .with_context(|| format!("running {} start", binary.display()))?;
    if !output.status.success() {
        bail!(
            "{} start failed ({}): {}",
            binary.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Waits for the answer, keeping the events that come first.
    async fn request(
        connection: &Connection,
        events: &mut Events,
        received: &mut Vec<Event>,
        request: Request,
    ) -> Result<Response> {
        let response = connection.request(request);
        futures::pin_mut!(response);
        loop {
            futures::select_biased! {
                response = response.as_mut().fuse() => return response,
                event = events.next().fuse() => match event {
                    Some(event) => received.push(event),
                    None => return response.await,
                },
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn requests_and_events() {
        let data_dir = tempfile::tempdir().expect("temp dir");
        let project_dir = tempfile::tempdir().expect("temp dir");
        let server = agentz_server::start(
            tokio::runtime::Handle::current(),
            agentz_server::ServerConfig {
                data_dir: data_dir.path().to_path_buf(),
                version: "0.0.0-test".into(),
                http_client: Arc::new(http_client::BlockedHttpClient),
                shell_environment_ready: futures::future::ready(()).boxed().shared(),
                custom_agents: Default::default(),
            },
        )
        .expect("server starts");
        let (client_stream, server_stream) = tokio::io::duplex(1 << 16);
        server.serve(server_stream);
        let (connection, mut events) = Connection::new(
            &tokio::runtime::Handle::current(),
            client_stream,
            ClientKind::App,
            "0.0.0-test".into(),
        )
        .await
        .expect("connects");
        assert_eq!(connection.welcome().pid, std::process::id());
        let mut received = Vec::new();

        let session = request(
            &connection,
            &mut events,
            &mut received,
            Request::SubscribeSession,
        )
        .await;
        assert!(matches!(session, Ok(Response::Session(_))));
        let added = request(
            &connection,
            &mut events,
            &mut received,
            Request::AddProject {
                path: project_dir.path().to_path_buf(),
            },
        )
        .await;
        assert!(matches!(added, Ok(Response::ProjectAdded(_))));
        // The change came before the answer.
        assert!(
            received.iter().any(
                |event| matches!(event, Event::Projects(projects) if projects.projects.len() == 1)
            ),
            "{received:?}"
        );
        let refused = request(
            &connection,
            &mut events,
            &mut received,
            Request::DeleteThread(projects::ThreadId(99)),
        )
        .await;
        assert!(refused.is_err());

        request(&connection, &mut events, &mut received, Request::Shutdown)
            .await
            .expect("shuts down");
        assert!(events.next().await.is_none());
        assert!(connection.request(Request::SubscribeSession).await.is_err());
        assert!(connection.is_closed());
    }
}
