//! One client's connection: the handshake, then requests in and messages out.

use agentz_protocol::{
    ClientHello, ClientMessage, PROTOCOL_VERSION, ServerMessage, ServerWelcome, read_message,
    write_message,
};
use anyhow::{Result, anyhow};
use futures::StreamExt as _;
use futures::channel::mpsc;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::server::{ClientId, Input};

pub(crate) async fn serve(
    stream: impl AsyncRead + AsyncWrite + Send + 'static,
    client: ClientId,
    mut welcome: ServerWelcome,
    inputs: mpsc::UnboundedSender<Input>,
) -> Result<()> {
    let (mut reader, mut writer) = tokio::io::split(stream);
    let Some(hello) = read_message::<ClientHello>(&mut reader).await? else {
        return Ok(());
    };
    if hello.protocol_version != PROTOCOL_VERSION {
        let error = format!(
            "the server speaks protocol version {PROTOCOL_VERSION}, the client {}",
            hello.protocol_version
        );
        welcome.error = Some(error.clone());
        write_message(&mut writer, &welcome).await?;
        return Err(anyhow!(error));
    }
    write_message(&mut writer, &welcome).await?;
    log::info!(
        "client {client} connected: {:?} {}",
        hello.client_kind,
        hello.client_version
    );

    let (outgoing, mut outgoing_messages) = mpsc::unbounded::<ServerMessage>();
    inputs
        .unbounded_send(Input::Connected { client, outgoing })
        .map_err(|_| anyhow!("the server has stopped"))?;

    let write = async {
        while let Some(message) = outgoing_messages.next().await {
            write_message(&mut writer, &message).await?;
        }
        anyhow::Ok(())
    };
    let read = async {
        while let Some(message) = read_message::<ClientMessage>(&mut reader).await? {
            match message {
                ClientMessage::Request { id, request } => {
                    if inputs
                        .unbounded_send(Input::Request {
                            client,
                            id,
                            request,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                ClientMessage::Unknown(message) => {
                    log::warn!("client {client} sent an unknown message: {message}")
                }
            }
        }
        anyhow::Ok(())
    };
    let result = tokio::select! {
        result = write => result,
        result = read => result,
    };
    inputs.unbounded_send(Input::Disconnected(client)).ok();
    log::info!("client {client} disconnected");
    result
}
