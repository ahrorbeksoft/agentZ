//! The pages server (design/artifacts): serves the artifacts of every machine the app reaches
//! to the user's browser. It listens on `127.0.0.1` only, takes the token from
//! `artifacts/server.json` on every shell and API link, and runs agents' pages in sandboxed
//! frames with a per-artifact key instead, so a page's scripts can't use the token
//! (`design/artifacts` backlog: an agent's scripts run in the user's browser). The port and
//! token are kept across restarts and server handoffs, so open pages and links keep working.
//!
//! Another machine's artifacts come through the app: a request naming one is passed to the
//! server actor ([`PageAsk`]), which relays it as [`Event::RelayArtifact`]. Pages hear of
//! publishes, deletions and theme changes over `/events` (SSE).

use std::path::Path;
use std::sync::{Arc, RwLock};

use agentz_protocol::Event;
use agentz_protocol::artifacts::{
    ArtifactId, ArtifactKind, ArtifactReply, ArtifactRequest, PageTheme,
};
use anyhow::{Context as _, Result};
use futures::channel::{mpsc, oneshot};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt as _, AsyncReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::TcpListener;

use crate::artifacts::{self, PagesConfig};
use crate::server::Input;

/// How long binding the kept port is retried at start: a server this one handed off from lets
/// go of it as it exits.
const BIND_ATTEMPTS: u32 = 50;
const BIND_RETRY: std::time::Duration = std::time::Duration::from_millis(100);
/// The most a request's head or a Send to thread body may hold.
const MAX_HEAD: usize = 16 * 1024;
const MAX_BODY: usize = 1024 * 1024;
/// A relayed read through the app may take a moment on a slow link.
const RELAY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// What a page asked the server for: a machine's answer to an [`ArtifactRequest`], `None`
/// being this machine.
pub(crate) struct PageAsk {
    pub machine: Option<String>,
    pub request: ArtifactRequest,
    pub answer: oneshot::Sender<Result<ArtifactReply, String>>,
}

/// Something an open page should hear of, over `/events`.
#[derive(Clone, Debug)]
pub(crate) enum PageEvent {
    /// Published again; `machine` is `None` for this one.
    Artifact {
        machine: Option<String>,
        id: ArtifactId,
    },
    Deleted {
        machine: Option<String>,
        id: ArtifactId,
    },
    Theme,
}

/// The running pages server, cheap to clone. It ends with the runtime.
#[derive(Clone)]
pub(crate) struct Pages {
    pub address: String,
    pub token: String,
    theme: Arc<RwLock<PageTheme>>,
    events: tokio::sync::broadcast::Sender<PageEvent>,
}

impl Pages {
    pub fn pages(&self) -> agentz_protocol::artifacts::ArtifactPages {
        agentz_protocol::artifacts::ArtifactPages {
            address: self.address.clone(),
            token: self.token.clone(),
        }
    }

    pub fn theme(&self) -> PageTheme {
        self.theme
            .read()
            .map(|theme| theme.clone())
            .unwrap_or_default()
    }

    pub fn set_theme(&self, theme: PageTheme) {
        if let Ok(mut current) = self.theme.write() {
            *current = theme;
        }
        self.broadcast(PageEvent::Theme);
    }

    pub fn broadcast(&self, event: PageEvent) {
        // Nobody listening is fine.
        self.events.send(event).ok();
    }
}

/// Starts the pages server. `None` (logged) when the port can't be had at all: artifacts
/// still work in the app, only the browser pages don't.
pub(crate) fn start(
    runtime: &tokio::runtime::Handle,
    data_dir: &Path,
    inputs: mpsc::UnboundedSender<Input>,
) -> Option<Pages> {
    let config = match PagesConfig::load(data_dir) {
        Ok(config) => config,
        Err(error) => {
            log::error!("couldn't read the pages server file: {error:#}");
            return None;
        }
    };
    let mut listener = None;
    if config.port != 0 {
        for attempt in 0..BIND_ATTEMPTS {
            match std::net::TcpListener::bind(("127.0.0.1", config.port)) {
                Ok(bound) => {
                    listener = Some(bound);
                    break;
                }
                Err(error) => {
                    if attempt + 1 == BIND_ATTEMPTS {
                        log::warn!("couldn't bind the pages port {}: {error}", config.port);
                    } else {
                        std::thread::sleep(BIND_RETRY);
                    }
                }
            }
        }
    }
    let listener = listener.or_else(|| std::net::TcpListener::bind(("127.0.0.1", 0)).ok())?;
    let address = match listener.local_addr() {
        Ok(address) => address.to_string(),
        Err(error) => {
            log::error!("the pages listener has no address: {error}");
            return None;
        }
    };
    if let Err(error) = listener.set_nonblocking(true) {
        log::error!("couldn't ready the pages listener: {error}");
        return None;
    }
    let listener = match TcpListener::from_std(listener) {
        Ok(listener) => listener,
        Err(error) => {
            log::error!("couldn't ready the pages listener: {error}");
            return None;
        }
    };
    if config.port.to_string() != address.rsplit(':').next().unwrap_or_default() {
        let config = PagesConfig {
            port: address
                .rsplit(':')
                .next()
                .and_then(|port| port.parse().ok())
                .unwrap_or_default(),
            token: config.token.clone(),
        };
        if let Err(error) = config.save(data_dir) {
            log::error!("couldn't save the pages server file: {error:#}");
        }
    }
    let (events, _) = tokio::sync::broadcast::channel(64);
    let pages = Pages {
        address,
        token: config.token,
        theme: Arc::new(RwLock::new(artifacts::load_theme(data_dir))),
        events,
    };
    let task = {
        let pages = pages.clone();
        async move {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        log::warn!("a pages connection failed: {error}");
                        continue;
                    }
                };
                let pages = pages.clone();
                let inputs = inputs.clone();
                tokio::spawn(async move {
                    if let Err(error) = serve(stream, pages, inputs).await {
                        log::debug!("a pages request failed: {error:#}");
                    }
                });
            }
        }
    };
    runtime.spawn(task);
    Some(pages)
}

async fn serve(
    stream: tokio::net::TcpStream,
    pages: Pages,
    inputs: mpsc::UnboundedSender<Input>,
) -> Result<()> {
    let (reading, mut writing) = stream.into_split();
    let mut reader = BufReader::new(reading);
    // The request line and headers, capped; only Send carries a body.
    let mut head = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            return Ok(());
        }
        if head.len() + line.len() > MAX_HEAD {
            return reply(
                &mut writing,
                "431 Request Header Fields Too Large",
                "text/plain",
                b"too large".to_vec(),
                &[],
            )
            .await;
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        head.push_str(&line);
    }
    let mut lines = head.lines();
    let request_line = lines.next().context("an empty request")?;
    let mut parts = request_line.split_whitespace();
    let (method, target) = (
        parts.next().context("a request without a method")?,
        parts.next().context("a request without a path")?,
    );
    let mut content_length = 0usize;
    for line in lines {
        if let Some((name, value)) = line.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query: Vec<(String, String)> = url::form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let query = Query(&query);
    let given = query.get("t");
    // The frame key opens only a version's content and files; the token opens everything else.
    let key = query.get("key");
    let segments: Vec<String> = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(percent_decode)
        .collect();
    let at = |expected: &[&str]| {
        segments.len() == expected.len()
            && segments
                .iter()
                .zip(expected)
                .all(|(segment, expected)| segment == expected)
    };

    if at(&["events"]) {
        return events(&mut writing, &pages, given.as_deref()).await;
    }

    // Everything below needs the token, except a version's own content and files (the frame's
    // key) and its raw source for Copy Source.
    match segments.first().map(String::as_str) {
        Some("a") | Some("m") => {}
        _ if given.as_deref() != Some(pages.token.as_str()) => {
            return reply(
                &mut writing,
                "403 Forbidden",
                "text/plain",
                b"forbidden".to_vec(),
                &[],
            )
            .await;
        }
        _ => {}
    }

    if at(&[]) {
        return reply(
            &mut writing,
            "200 OK",
            "text/html; charset=utf-8",
            gallery(&pages).into_bytes(),
            &[],
        )
        .await;
    }
    if at(&["api", "artifacts"]) {
        let listings = gather(&inputs, None).await;
        let mut machines = vec![MachineArtifacts {
            machine: None,
            artifacts: listings,
        }];
        for machine in machine_names(&inputs).await {
            let artifacts = gather(&inputs, Some(&machine)).await;
            machines.push(MachineArtifacts {
                machine: Some(machine),
                artifacts,
            });
        }
        return json_reply(&mut writing, &machines).await;
    }
    if segments.len() == 3 && segments[0] == "api" && segments[1] == "artifact" {
        let id = ArtifactId(segments[2].clone());
        let machine = query.get("machine");
        return match ask(&inputs, machine, ArtifactRequest::Listing(id)).await {
            Ok(ArtifactReply::Listing(listing)) => json_reply(&mut writing, &listing).await,
            Ok(_) => {
                reply(
                    &mut writing,
                    "404 Not Found",
                    "text/plain",
                    b"no such artifact".to_vec(),
                    &[],
                )
                .await
            }
            Err(error) => {
                reply(
                    &mut writing,
                    "404 Not Found",
                    "text/plain",
                    error.into_bytes(),
                    &[],
                )
                .await
            }
        };
    }
    if at(&["api", "send"]) && method == "POST" {
        let body = read_body(&mut reader, &mut writing, content_length).await?;
        let Some(body) = body else { return Ok(()) };
        let request: Value = serde_json::from_slice(&body).unwrap_or_default();
        let machine = request["machine"]
            .as_str()
            .filter(|machine| !machine.is_empty())
            .map(str::to_string);
        let send = ArtifactRequest::Send {
            id: ArtifactId(request["id"].as_str().unwrap_or_default().to_string()),
            version: request["version"].as_u64().unwrap_or_default() as u32,
            text: request["text"].as_str().unwrap_or_default().to_string(),
            note: request["note"].as_str().unwrap_or_default().to_string(),
        };
        return match ask(&inputs, machine, send).await {
            Ok(_) => {
                reply(
                    &mut writing,
                    "200 OK",
                    "application/json",
                    b"{}".to_vec(),
                    &[],
                )
                .await
            }
            Err(error) => {
                reply(
                    &mut writing,
                    "400 Bad Request",
                    "application/json",
                    json!({"error": error}).to_string().into_bytes(),
                    &[],
                )
                .await
            }
        };
    }
    if at(&["api", "show-thread"]) && method == "POST" {
        let body = read_body(&mut reader, &mut writing, content_length).await?;
        let Some(body) = body else { return Ok(()) };
        let request: Value = serde_json::from_slice(&body).unwrap_or_default();
        let machine = request["machine"]
            .as_str()
            .filter(|machine| !machine.is_empty())
            .map(str::to_string);
        let thread = request["thread"].as_u64().unwrap_or_default();
        let event = Event::ShowThread {
            machine,
            thread_id: projects::ThreadId(thread),
        };
        let tell = Input::Run(Box::new(move |server: &mut crate::server::Server| {
            server.tell_clients(event);
        }));
        inputs.unbounded_send(tell).ok();
        return reply(
            &mut writing,
            "200 OK",
            "application/json",
            b"{}".to_vec(),
            &[],
        )
        .await;
    }

    // /a/<id>, with the machine's /m/<machine>/a/<id>.
    let (machine, rest) = match segments.first().map(String::as_str) {
        Some("m") if segments.len() >= 3 => (Some(segments[1].clone()), &segments[2..]),
        Some("a") => (None, &segments[..]),
        _ => {
            return reply(
                &mut writing,
                "404 Not Found",
                "text/plain",
                b"not found".to_vec(),
                &[],
            )
            .await;
        }
    };
    if rest.len() >= 2 && rest[0] == "a" {
        let id = ArtifactId(rest[1].clone());
        if rest.len() == 2 {
            if given.as_deref() != Some(pages.token.as_str()) {
                return reply(
                    &mut writing,
                    "403 Forbidden",
                    "text/plain",
                    b"forbidden".to_vec(),
                    &[],
                )
                .await;
            }
            let version = query
                .get("v")
                .and_then(|version| version.parse::<u32>().ok())
                .filter(|version| *version > 0);
            let page = shell(&pages, machine.as_deref(), &id, version);
            return reply(
                &mut writing,
                "200 OK",
                "text/html; charset=utf-8",
                page.into_bytes(),
                &[],
            )
            .await;
        }
        let Ok(version) = rest
            .get(3)
            .map(String::as_str)
            .unwrap_or_default()
            .parse::<u32>()
        else {
            return reply(
                &mut writing,
                "404 Not Found",
                "text/plain",
                b"not found".to_vec(),
                &[],
            )
            .await;
        };
        match rest[2].as_str() {
            "content" => {
                let read = ArtifactRequest::Read {
                    id: id.clone(),
                    version: Some(version),
                };
                let listing = match ask(
                    &inputs,
                    machine.clone(),
                    ArtifactRequest::Listing(id.clone()),
                )
                .await
                {
                    Ok(ArtifactReply::Listing(listing)) => listing,
                    _ => {
                        return reply(
                            &mut writing,
                            "404 Not Found",
                            "text/plain",
                            b"no such artifact".to_vec(),
                            &[],
                        )
                        .await;
                    }
                };
                if key.as_deref() != Some(listing.artifact.frame_key.as_str()) {
                    return reply(
                        &mut writing,
                        "403 Forbidden",
                        "text/plain",
                        b"forbidden".to_vec(),
                        &[],
                    )
                    .await;
                }
                let source = match ask(&inputs, machine, read).await {
                    Ok(ArtifactReply::Page { source, .. }) => source,
                    _ => {
                        return reply(
                            &mut writing,
                            "404 Not Found",
                            "text/plain",
                            b"no such version".to_vec(),
                            &[],
                        )
                        .await;
                    }
                };
                let print = query.get("print").is_some();
                let body = match listing.artifact.kind {
                    ArtifactKind::Page => html_page(&source, &pages.theme(), print),
                    ArtifactKind::Document => {
                        document_page(&listing.artifact.title, &source, &pages.theme(), print)
                    }
                };
                return reply(
                    &mut writing,
                    "200 OK",
                    "text/html; charset=utf-8",
                    body.into_bytes(),
                    &[],
                )
                .await;
            }
            "files" => {
                let Some(name) = rest.get(4) else {
                    return reply(
                        &mut writing,
                        "404 Not Found",
                        "text/plain",
                        b"not found".to_vec(),
                        &[],
                    )
                    .await;
                };
                let listing = match ask(
                    &inputs,
                    machine.clone(),
                    ArtifactRequest::Listing(id.clone()),
                )
                .await
                {
                    Ok(ArtifactReply::Listing(listing)) => listing,
                    _ => {
                        return reply(
                            &mut writing,
                            "404 Not Found",
                            "text/plain",
                            b"no such artifact".to_vec(),
                            &[],
                        )
                        .await;
                    }
                };
                if key.as_deref() != Some(listing.artifact.frame_key.as_str()) {
                    return reply(
                        &mut writing,
                        "403 Forbidden",
                        "text/plain",
                        b"forbidden".to_vec(),
                        &[],
                    )
                    .await;
                }
                let file = ArtifactRequest::File {
                    id,
                    version,
                    name: name.clone(),
                };
                let data = match ask(&inputs, machine, file).await {
                    Ok(ArtifactReply::File(data)) => data,
                    _ => {
                        return reply(
                            &mut writing,
                            "404 Not Found",
                            "text/plain",
                            b"no such file".to_vec(),
                            &[],
                        )
                        .await;
                    }
                };
                use base64::Engine as _;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap_or_default();
                let download = (
                    "content-disposition",
                    format!("attachment; filename=\"{}\"", name.replace('"', "_")),
                );
                return reply(
                    &mut writing,
                    "200 OK",
                    "application/octet-stream",
                    bytes,
                    &[download],
                )
                .await;
            }
            "raw" => {
                if given.as_deref() != Some(pages.token.as_str()) {
                    return reply(
                        &mut writing,
                        "403 Forbidden",
                        "text/plain",
                        b"forbidden".to_vec(),
                        &[],
                    )
                    .await;
                }
                let (source, kind) = match ask(
                    &inputs,
                    machine.clone(),
                    ArtifactRequest::Listing(id.clone()),
                )
                .await
                {
                    Ok(ArtifactReply::Listing(listing)) => {
                        let read = ArtifactRequest::Read {
                            id,
                            version: Some(version),
                        };
                        match ask(&inputs, machine, read).await {
                            Ok(ArtifactReply::Page { source, .. }) => {
                                (source, listing.artifact.kind)
                            }
                            _ => {
                                return reply(
                                    &mut writing,
                                    "404 Not Found",
                                    "text/plain",
                                    b"no such version".to_vec(),
                                    &[],
                                )
                                .await;
                            }
                        }
                    }
                    _ => {
                        return reply(
                            &mut writing,
                            "404 Not Found",
                            "text/plain",
                            b"no such artifact".to_vec(),
                            &[],
                        )
                        .await;
                    }
                };
                let content_type = match kind {
                    ArtifactKind::Page => "text/plain; charset=utf-8",
                    ArtifactKind::Document => "text/markdown; charset=utf-8",
                };
                return reply(
                    &mut writing,
                    "200 OK",
                    content_type,
                    source.into_bytes(),
                    &[],
                )
                .await;
            }
            _ => {}
        }
    }
    reply(
        &mut writing,
        "404 Not Found",
        "text/plain",
        b"not found".to_vec(),
        &[],
    )
    .await
}

struct Query<'a>(&'a [(String, String)]);

impl Query<'_> {
    fn get(&self, key: &str) -> Option<String> {
        self.0
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.clone())
    }
}

#[derive(Serialize)]
struct MachineArtifacts {
    machine: Option<String>,
    artifacts: Vec<agentz_protocol::artifacts::ArtifactListing>,
}

/// The machine's own artifact listings, as the gallery shows them.
async fn gather(
    inputs: &mpsc::UnboundedSender<Input>,
    machine: Option<&str>,
) -> Vec<agentz_protocol::artifacts::ArtifactListing> {
    match ask(inputs, machine.map(str::to_string), ArtifactRequest::List).await {
        Ok(ArtifactReply::List(listings)) => listings,
        _ => Vec::new(),
    }
}

/// The other machines the app says it reaches, so the gallery can list their artifacts.
async fn machine_names(inputs: &mpsc::UnboundedSender<Input>) -> Vec<String> {
    let (answer, receive) = oneshot::channel();
    let ask = Input::Run(Box::new(move |server: &mut crate::server::Server| {
        server.answer_machine_names(answer);
    }));
    if inputs.unbounded_send(ask).is_err() {
        return Vec::new();
    }
    receive.await.unwrap_or_default()
}

async fn ask(
    inputs: &mpsc::UnboundedSender<Input>,
    machine: Option<String>,
    request: ArtifactRequest,
) -> Result<ArtifactReply, String> {
    let (answer, receive) = oneshot::channel();
    inputs
        .unbounded_send(Input::PageAsk(PageAsk {
            machine,
            request,
            answer,
        }))
        .map_err(|_| "the server is stopping".to_string())?;
    match tokio::time::timeout(RELAY_TIMEOUT, receive).await {
        Ok(Ok(reply)) => reply,
        Ok(Err(_)) => Err("the server is stopping".to_string()),
        Err(_) => Err("that took too long; is agentZ open?".to_string()),
    }
}

async fn read_body(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
    writing: &mut tokio::net::tcp::OwnedWriteHalf,
    content_length: usize,
) -> Result<Option<Vec<u8>>> {
    if content_length > MAX_BODY {
        reply(
            writing,
            "413 Content Too Large",
            "text/plain",
            b"too large".to_vec(),
            &[],
        )
        .await?;
        return Ok(None);
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).await?;
    Ok(Some(body))
}

/// The events stream: one `data:` line per [`PageEvent`], until the browser goes.
async fn events(
    writing: &mut tokio::net::tcp::OwnedWriteHalf,
    pages: &Pages,
    given: Option<&str>,
) -> Result<()> {
    if given != Some(pages.token.as_str()) {
        return reply(
            writing,
            "403 Forbidden",
            "text/plain",
            b"forbidden".to_vec(),
            &[],
        )
        .await;
    }
    let headers = b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncache-control: no-store\r\nconnection: close\r\n\r\n";
    writing.write_all(headers).await?;
    writing.flush().await?;
    let mut events = pages.events.subscribe();
    loop {
        let event = match events.recv().await {
            Ok(event) => event,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => return Ok(()),
        };
        let data = match event {
            PageEvent::Artifact { machine, id } => {
                json!({"type": "artifact", "machine": machine, "id": id.0})
            }
            PageEvent::Deleted { machine, id } => {
                json!({"type": "deleted", "machine": machine, "id": id.0})
            }
            PageEvent::Theme => {
                let theme = pages.theme();
                json!({"type": "theme", "dark": theme.dark, "variables": theme.variables})
            }
        };
        if writing
            .write_all(format!("data: {data}\n\n").as_bytes())
            .await
            .is_err()
        {
            return Ok(());
        }
        if writing.flush().await.is_err() {
            return Ok(());
        }
    }
}

async fn json_reply(
    writing: &mut tokio::net::tcp::OwnedWriteHalf,
    value: &impl Serialize,
) -> Result<()> {
    let body = serde_json::to_vec(value).context("encoding a reply")?;
    reply(writing, "200 OK", "application/json", body, &[]).await
}

async fn reply(
    writing: &mut tokio::net::tcp::OwnedWriteHalf,
    status: &str,
    content_type: &str,
    body: Vec<u8>,
    extra: &[(&str, String)],
) -> Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\ncache-control: no-store\r\nconnection: close\r\n",
        body.len()
    );
    for (name, value) in extra {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    writing.write_all(head.as_bytes()).await?;
    writing.write_all(&body).await?;
    writing.flush().await?;
    Ok(())
}

fn percent_decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(hex) = u8::from_str_radix(&segment[index + 1..index + 3], 16)
        {
            decoded.push(hex);
            index += 3;
            continue;
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The theme as a `:root` rule, before the first paint as t3code does it.
fn vars_style(theme: &PageTheme) -> String {
    let mut css = String::from(":root{color-scheme:");
    css.push_str(if theme.dark { "dark" } else { "light" });
    css.push_str(";");
    for (name, value) in &theme.variables {
        if safe(name) && safe(value) {
            css.push_str(name);
            css.push(':');
            css.push_str(value);
            css.push(';');
        }
    }
    css.push('}');
    css
}

fn safe(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_#.,%() ".contains(c))
}

/// The script every page gets (topic 10 and 11): theme changes reach it as messages, and
/// Send to thread asks it for what it gathered through `window.agentzPicks()`.
fn bridge_script(print: bool) -> String {
    let print = if print {
        "try{window.print()}catch(_){}"
    } else {
        ""
    };
    format!(
        r#"<script>(function(){{
function apply(t){{var r=document.documentElement;if(t.dark!==undefined){{r.classList.toggle('agentz-dark',!!t.dark);r.style.colorScheme=t.dark?'dark':'light';}}var v=t.variables||{{}};for(var k in v)r.style.setProperty(k,v[k]);}}
window.addEventListener('message',function(e){{var d=e.data||{{}};if(d.agentzTheme)apply(d.agentzTheme);if(d.agentzCollect){{var text='';try{{if(typeof window.agentzPicks==='function')text=String(window.agentzPicks()||'');else text=String(window.getSelection()||'');}}catch(_){{}}parent.postMessage({{agentzPicks:text}},'*');}}}});
{print}
}})();</script>"#
    )
}

/// A published HTML page as served: its own file, with the theme and the bridge added.
fn html_page(source: &str, theme: &PageTheme, print: bool) -> String {
    let addition = format!(
        "<style>{}</style>{}",
        vars_style(theme),
        bridge_script(print)
    );
    match source.rfind("</body>") {
        Some(at) => format!("{}{}{}", &source[..at], addition, &source[at..]),
        None => format!("{source}{addition}"),
    }
}

const HOUSE: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8"/><path d="M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>"#;
const DOWNLOAD: &str = r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" x2="12" y1="15" y2="3"/></svg>"#;
const SEND: &str = r#"<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14.54 21.69a.5.5 0 0 0 .94-.03l6.5-19a.5.5 0 0 0-.64-.64l-19 6.5a.5.5 0 0 0-.03.94l7.93 3.18a2 2 0 0 1 1.11 1.11z"/><path d="m21.85 2.15-10.94 10.94"/></svg>"#;
const PAGE_ICON: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="20" height="16" rx="2"/><path d="M2 8h20"/></svg>"#;
const DOC_ICON: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/></svg>"#;

/// The shell around a page (topic 7 A): the 44px bar with everything on it, the version's
/// banner (topic 8 A) and the republish bar (topic 12 B) above the sandboxed page.
fn shell(pages: &Pages, machine: Option<&str>, id: &ArtifactId, version: Option<u32>) -> String {
    let data = serde_json::to_string(&json!({
        "id": id.0,
        "machine": machine,
        "version": version,
        "token": pages.token,
    }))
    .unwrap_or_default()
    .replace('<', "\\u003c");
    SHELL
        .replace("__DATA__", &format!("const ART = {data};"))
        .replace("__VARS__", &vars_style(&pages.theme()))
        .replace("__HOUSE__", HOUSE)
        .replace("__DOWNLOAD__", DOWNLOAD)
        .replace("__SEND__", SEND)
}

/// The gallery (topic 5 D): every machine's artifacts, filtered and searched.
fn gallery(pages: &Pages) -> String {
    let data = serde_json::to_string(&json!({"token": pages.token}))
        .unwrap_or_default()
        .replace('<', "\\u003c");
    GALLERY
        .replace("__DATA__", &format!("const GALLERY = {data};"))
        .replace("__VARS__", &vars_style(&pages.theme()))
        .replace("__PAGE_ICON__", PAGE_ICON)
        .replace("__DOC_ICON__", DOC_ICON)
}

const SHELL: &str = r##"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Artifact</title>
<style>
__VARS__
* { box-sizing: border-box; }
html, body { margin: 0; height: 100%; background: var(--background, #1e1e1e); color: var(--text, #ccc); font: 13px var(--font-sans, system-ui, sans-serif); }
#bar { height: 44px; display: flex; align-items: center; gap: 8px; padding: 0 10px; border-bottom: 1px solid var(--border, #333); background: var(--surface, var(--panel, #252526)); overflow: visible; position: relative; z-index: 3; }
#bar a, #bar button { font: inherit; color: inherit; }
.iconbtn { display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; border: none; border-radius: 6px; background: none; cursor: pointer; color: var(--muted, #888); text-decoration: none; }
.iconbtn:hover { background: var(--hover, rgba(255,255,255,.06)); color: var(--text, #ccc); }
.slash { color: var(--muted, #888); }
.title { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 320px; }
.vtag { border: 1px solid var(--border, #333); background: var(--background, #1e1e1e); border-radius: 6px; padding: 2px 8px; cursor: pointer; white-space: nowrap; }
.vtag.accent { color: var(--accent, #548af7); border-color: var(--accent, #548af7); }
.where { display: flex; align-items: center; gap: 5px; color: var(--muted, #888); font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
.where a { color: inherit; cursor: pointer; text-decoration: none; overflow: hidden; text-overflow: ellipsis; }
.where a:hover { color: var(--text, #ccc); text-decoration: underline; }
.where svg { flex: none; }
.grow { flex: 1; }
.btn { display: inline-flex; align-items: center; gap: 5px; border: none; border-radius: 6px; padding: 5px 10px; background: var(--hover, rgba(255,255,255,.06)); cursor: pointer; white-space: nowrap; }
.btn:hover { filter: brightness(1.2); }
.btn.primary { background: var(--accent, #548af7); color: var(--accent-foreground, #fff); }
.menu { position: absolute; top: 40px; background: var(--surface, var(--panel, #252526)); border: 1px solid var(--border, #333); border-radius: 8px; box-shadow: 0 8px 24px rgba(0,0,0,.4); padding: 4px; min-width: 200px; z-index: 5; }
.menu a, .menu button { display: flex; width: 100%; align-items: center; gap: 8px; padding: 6px 10px; border: none; border-radius: 5px; background: none; color: inherit; text-decoration: none; cursor: pointer; text-align: left; white-space: nowrap; }
.menu a:hover, .menu button:hover { background: var(--hover, rgba(255,255,255,.06)); }
.menu .when { margin-left: auto; color: var(--muted, #888); font-size: 11px; padding-left: 16px; }
#banner { display: flex; align-items: center; gap: 8px; padding: 7px 12px; font-size: 12px; background: var(--surface, var(--panel, #252526)); border-bottom: 1px solid var(--border, #333); color: var(--muted, #888); }
#banner b { color: var(--text, #ccc); font-weight: 600; }
#banner a { color: var(--accent, #548af7); cursor: pointer; }
#updatebar { position: absolute; left: 50%; transform: translateX(-50%); top: 52px; z-index: 4; display: flex; gap: 8px; align-items: center; padding: 7px 14px; border-radius: 8px; background: var(--surface, var(--panel, #252526)); border: 1px solid var(--border, #333); box-shadow: 0 8px 24px rgba(0,0,0,.4); font-size: 12px; }
#updatebar a { color: var(--accent, #548af7); cursor: pointer; font-weight: 600; }
#frame { position: absolute; inset: 44px 0 0 0; width: 100%; height: calc(100% - 44px); border: none; }
body.banner #frame { top: 76px; height: calc(100% - 76px); }
#sent { font-size: 11px; color: var(--muted, #888); white-space: nowrap; }
#overlay { position: fixed; inset: 0; background: rgba(0,0,0,.45); display: flex; align-items: flex-start; justify-content: center; padding-top: 90px; z-index: 10; }
#dialog { width: 520px; max-width: 92vw; background: var(--background, #1e1e1e); border: 1px solid var(--border, #333); border-radius: 10px; padding: 14px; box-shadow: 0 16px 48px rgba(0,0,0,.5); }
#dialog h2 { margin: 0 0 10px; font-size: 14px; }
#picks { max-height: 200px; overflow: auto; white-space: pre-wrap; word-break: break-word; background: var(--surface, var(--panel, #252526)); border: 1px solid var(--border, #333); border-radius: 6px; padding: 8px 10px; font: 12px var(--font-mono, ui-monospace, monospace); color: var(--muted, #bbb); margin: 0 0 10px; }
#note { width: 100%; min-height: 64px; resize: vertical; border: 1px solid var(--border, #333); border-radius: 6px; background: var(--surface, var(--panel, #252526)); color: var(--text, #ccc); font: 13px var(--font-sans, system-ui, sans-serif); padding: 8px 10px; }
#dialog .row { display: flex; justify-content: flex-end; gap: 8px; margin-top: 10px; }
#error { position: fixed; inset: 0; display: flex; align-items: center; justify-content: center; flex-direction: column; gap: 8px; color: var(--muted, #888); }
[hidden] { display: none !important; }
@media print { #bar, #banner, #updatebar { display: none !important; } #frame { position: static; height: auto; } }
</style>
</head>
<body>
<div id="bar">
  <a class="iconbtn" id="home" title="All artifacts">__HOUSE__</a>
  <span class="slash">/</span>
  <span class="title" id="title"></span>
  <button class="vtag" id="version"></button>
  <span class="where" id="where"></span>
  <span class="grow"></span>
  <span id="sent" hidden></span>
  <button class="btn" id="files" hidden></button>
  <button class="btn" id="export">__DOWNLOAD__Export</button>
  <button class="btn primary" id="send">__SEND__Send to thread</button>
</div>
<div id="banner" hidden></div>
<div id="updatebar" hidden></div>
<iframe id="frame" sandbox="allow-scripts allow-downloads allow-modals"></iframe>
<div id="overlay" hidden><div id="dialog">
  <h2>Send to thread</h2>
  <pre id="picks"></pre>
  <textarea id="note" placeholder="A note to go with it…"></textarea>
  <div class="row"><button class="btn" id="cancel">Cancel</button><button class="btn primary" id="sendit">__SEND__Send</button></div>
</div></div>
<div id="error" hidden></div>
<div id="menus"></div>
<script>__DATA__</script>
<script>
const $ = (id) => document.getElementById(id);
const base = ART.machine ? '/m/' + encodeURIComponent(ART.machine) + '/a/' + ART.id : '/a/' + ART.id;
const api = (path) => path + (path.includes('?') ? '&' : '?') + 't=' + encodeURIComponent(ART.token);
const pageUrl = (v) => base + '?t=' + encodeURIComponent(ART.token) + (v ? '&v=' + v : '');
const contentUrl = (v, key, print) => base + '/content/' + v + '?key=' + encodeURIComponent(key) + (print ? '&print=1' : '');
const rawUrl = (v) => base + '/raw/' + v + '?t=' + encodeURIComponent(ART.token);
let listing = null, version = ART.version, closing = null;

function ago(ms) {
  if (!ms) return '';
  const s = Math.max(1, Math.round((Date.now() - ms) / 1000));
  if (s < 60) return s + 's ago';
  const m = Math.round(s / 60); if (m < 60) return m + 'm ago';
  const h = Math.round(m / 60); if (h < 24) return h + 'h ago';
  const d = Math.round(h / 24); if (d < 30) return d + 'd ago';
  return new Date(ms).toLocaleDateString();
}
function menu(items, anchor) {
  closeMenus();
  const el = document.createElement('div');
  el.className = 'menu';
  const rect = anchor.getBoundingClientRect();
  el.style.right = Math.max(8, window.innerWidth - rect.right) + 'px';
  if (anchor.id === 'version') { el.style.right = 'auto'; el.style.left = rect.left + 'px'; }
  for (const item of items) {
    const row = document.createElement(item.href ? 'a' : 'button');
    row.innerHTML = item.label;
    if (item.href) { row.href = item.href; if (item.download) row.download = item.download; }
    if (item.click) row.onclick = (e) => { e.preventDefault(); closeMenus(); item.click(); };
    if (item.when) { const w = document.createElement('span'); w.className = 'when'; w.textContent = item.when; row.appendChild(w); }
    el.appendChild(row);
  }
  $('menus').appendChild(el);
  closing = () => { el.remove(); closing = null; };
  setTimeout(() => window.addEventListener('mousedown', function h(e) { if (!el.contains(e.target)) { closeMenus(); } }), 0);
}
function closeMenus() { if (closing) closing(); }

function render() {
  const a = listing.artifact, latest = a.versions.length;
  version = version || latest;
  document.title = a.title + ' · agentZ';
  $('title').textContent = a.title;
  $('home').href = '/?t=' + encodeURIComponent(ART.token);
  const vtag = $('version');
  vtag.textContent = version === latest ? 'v' + version : 'v' + version + ' of ' + latest;
  vtag.classList.toggle('accent', version !== latest);
  vtag.onclick = () => menu(a.versions.map((v, i) => ({ label: 'v' + (i + 1) + (i + 1 === latest ? ' (latest)' : ''), when: ago(v.published_at), click: () => { location.href = pageUrl(i + 1); } })).reverse(), vtag);
  const where = $('where');
  where.innerHTML = '';
  if (listing.agent_icon) { const icon = document.createElement('span'); icon.innerHTML = listing.agent_icon; icon.style.display = 'inline-flex'; icon.style.width = '14px'; icon.style.height = '14px'; where.appendChild(icon); }
  const place = document.createElement(listing.has_thread ? 'a' : 'span');
  place.textContent = listing.place;
  if (listing.has_thread) place.onclick = () => fetch(api('/api/show-thread'), { method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify({ machine: ART.machine || null, thread: a.thread_id ? a.thread_id : 0 }) });
  where.appendChild(place);
  const files = (a.versions[version - 1] || {}).files || [];
  const filesBtn = $('files');
  filesBtn.hidden = files.length === 0;
  filesBtn.textContent = 'Files ' + files.length;
  filesBtn.onclick = () => menu(files.map((f) => ({ label: f.name, when: f.bytes < 1048576 ? Math.round(f.bytes / 1024) + ' KB' : (f.bytes / 1048576).toFixed(1) + ' MB', href: base + '/files/' + version + '/' + encodeURIComponent(f.name) + '?key=' + encodeURIComponent(a.frame_key), download: f.name })), filesBtn);
  $('export').onclick = () => menu([
    { label: 'Save as ' + (a.kind === 'document' ? 'Markdown' : 'HTML'), href: rawUrl(version), download: a.title.replace(/[^\w.-]+/g, ' ').trim().replace(/\s+/g, '-') + '-v' + version + '.' + (a.kind === 'document' ? 'md' : 'html') },
    { label: 'Print or Save as PDF…', click: () => window.open(contentUrl(version, a.frame_key, true), '_blank') },
    { label: 'Copy Source', click: async () => { const text = await (await fetch(rawUrl(version))).text(); await navigator.clipboard.writeText(text); flash('Copied'); } },
  ], $('export'));
  $('send').hidden = !listing.has_thread;
  const sent = $('sent');
  sent.hidden = !a.sent_at;
  if (a.sent_at) sent.textContent = 'Sent ' + ago(a.sent_at);
  const banner = $('banner');
  banner.hidden = version === latest;
  document.body.classList.toggle('banner', version !== latest);
  if (version !== latest) {
    banner.innerHTML = '';
    // One span, so the banner's gap doesn't split the sentence.
    const text = document.createElement('span');
    text.append('This is ');
    const b = document.createElement('b'); b.textContent = 'v' + version + ', from ' + ago(a.versions[version - 1].published_at); text.appendChild(b);
    text.append('. The latest is v' + latest + '.');
    banner.appendChild(text);
    const show = document.createElement('a'); show.textContent = 'Show v' + latest; show.href = pageUrl(latest);
    banner.appendChild(show);
  }
  const frame = $('frame');
  const src = contentUrl(version, a.frame_key, false);
  if (frame.getAttribute('src') !== src) frame.src = src;
}
function flash(text) { const s = $('sent'); s.hidden = false; s.textContent = text; setTimeout(render, 2000); }

async function load() {
  let reply;
  try { reply = await (await fetch(api('/api/artifact/' + ART.id) + (ART.machine ? '&machine=' + encodeURIComponent(ART.machine) : ''))).json(); }
  catch (_) { return gone('This artifact could not be loaded.'); }
  if (!reply || !reply.artifact) return gone(typeof reply === 'string' ? reply : 'This artifact is gone.');
  listing = reply;
  render();
}
function gone(text) {
  $('frame').hidden = true; $('bar').hidden = true; $('banner').hidden = true;
  const e = $('error'); e.hidden = false; e.textContent = text;
}

$('send').onclick = () => {
  $('overlay').hidden = false;
  $('picks').textContent = '…';
  $('note').value = '';
  const frame = $('frame').contentWindow;
  const heard = (e) => {
    if (!e.data || e.data.agentzPicks === undefined) return;
    window.removeEventListener('message', heard);
    $('picks').textContent = e.data.agentzPicks || '(The page gave nothing; your note still goes.)';
  };
  window.addEventListener('message', heard);
  frame.postMessage({ agentzCollect: true }, '*');
  setTimeout(() => { window.removeEventListener('message', heard); if ($('picks').textContent === '…') $('picks').textContent = '(The page gave nothing; your note still goes.)'; }, 600);
};
$('cancel').onclick = () => { $('overlay').hidden = true; };
$('sendit').onclick = async () => {
  const note = $('note').value, text = $('picks').textContent;
  const answer = await fetch(api('/api/send'), { method: 'POST', headers: {'content-type': 'application/json'}, body: JSON.stringify({ machine: ART.machine || null, id: ART.id, version, text: text.startsWith('(') ? '' : text, note }) });
  $('overlay').hidden = true;
  if (answer.ok) { flash('Sent just now'); load(); } else { const e = await answer.json().catch(() => ({})); flash(e.error || 'Couldn’t send'); }
};

const events = new EventSource(api('/events'));
events.onmessage = (e) => {
  const d = JSON.parse(e.data);
  if (d.type === 'theme') {
    const r = document.documentElement;
    r.style.colorScheme = d.dark ? 'dark' : 'light';
    for (const k in (d.variables || {})) r.style.setProperty(k, d.variables[k]);
    const frame = $('frame').contentWindow;
    if (frame) frame.postMessage({ agentzTheme: { dark: d.dark, variables: d.variables } }, '*');
  }
  if ((d.type === 'artifact' || d.type === 'deleted') && d.id === ART.id && (d.machine || null) === (ART.machine || null)) {
    if (d.type === 'deleted') return gone('This artifact was deleted.');
    const latest = listing ? listing.artifact.versions.length : 0;
    load();
    setTimeout(() => {
      const now = listing.artifact.versions.length;
      if (now > latest && version === latest) {
        const bar = $('updatebar');
        bar.innerHTML = '';
        bar.append('v' + now + ' was just published. ');
        const show = document.createElement('a'); show.textContent = 'Show v' + now;
        show.onclick = () => { location.href = pageUrl(now); };
        bar.appendChild(show);
        bar.hidden = false;
      }
    }, 50);
  }
};
load();
</script>
</body>
</html>"##;

const GALLERY: &str = r##"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>All artifacts · agentZ</title>
<style>
__VARS__
* { box-sizing: border-box; }
body { margin: 0; background: var(--background, #1e1e1e); color: var(--text, #ccc); font: 13px var(--font-sans, system-ui, sans-serif); }
header { display: flex; align-items: center; gap: 12px; padding: 14px 22px; border-bottom: 1px solid var(--border, #333); background: var(--surface, var(--panel, #252526)); position: sticky; top: 0; }
h1 { font-size: 15px; margin: 0; font-weight: 600; }
.seg { display: flex; border: 1px solid var(--border, #333); border-radius: 7px; overflow: hidden; }
.seg button { border: none; background: none; color: inherit; font: inherit; padding: 4px 12px; cursor: pointer; }
.seg button.on { background: var(--accent, #548af7); color: var(--accent-foreground, #fff); }
#q { border: 1px solid var(--border, #333); border-radius: 7px; background: var(--background, #1e1e1e); color: inherit; font: inherit; padding: 5px 10px; width: 220px; }
#grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 14px; padding: 20px 22px; }
.card { display: block; border: 1px solid var(--border, #333); border-radius: 10px; background: var(--surface, var(--panel, #252526)); padding: 14px; color: inherit; text-decoration: none; }
.card:hover { border-color: var(--accent, #548af7); }
.card .kind { color: var(--muted, #888); }
.card h2 { font-size: 14px; margin: 8px 0 4px; font-weight: 600; }
.card .meta { color: var(--muted, #888); font-size: 12px; }
.card .where { color: var(--muted, #888); font-size: 12px; margin-top: 6px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mbadge { border: 1px solid var(--border, #333); border-radius: 5px; padding: 0 5px; font-size: 11px; color: var(--muted, #888); margin-left: 6px; }
#empty { padding: 60px; text-align: center; color: var(--muted, #888); grid-column: 1 / -1; }
</style>
</head>
<body>
<header>
  <h1>All artifacts</h1>
  <div class="seg"><button data-f="all" class="on">All</button><button data-f="threads">Threads</button><button data-f="chats">Chats</button></div>
  <span style="flex:1"></span>
  <input id="q" type="search" placeholder="Search artifacts…">
</header>
<div id="grid"></div>
<script>__DATA__</script>
<script>
const PAGE_ICON = `__PAGE_ICON__`, DOC_ICON = `__DOC_ICON__`;
let machines = [], filter = 'all', query = '';
function ago(ms) {
  if (!ms) return '';
  const s = Math.max(1, Math.round((Date.now() - ms) / 1000));
  if (s < 60) return s + 's ago';
  const m = Math.round(s / 60); if (m < 60) return m + 'm ago';
  const h = Math.round(m / 60); if (h < 24) return h + 'h ago';
  const d = Math.round(h / 24); if (d < 30) return d + 'd ago';
  return new Date(ms).toLocaleDateString();
}
function draw() {
  const grid = document.getElementById('grid');
  grid.innerHTML = '';
  let shown = 0;
  for (const m of machines) {
    for (const listing of m.artifacts) {
      const a = listing.artifact;
      const chat = a.project_id === 0;
      if (filter === 'threads' && chat) continue;
      if (filter === 'chats' && !chat) continue;
      if (query && !(a.title + ' ' + listing.place).toLowerCase().includes(query)) continue;
      shown++;
      const card = document.createElement('a');
      card.className = 'card';
      card.href = (m.machine ? '/m/' + encodeURIComponent(m.machine) : '') + '/a/' + a.id + '?t=' + encodeURIComponent(GALLERY.token);
      card.innerHTML = '<span class="kind">' + (a.kind === 'document' ? DOC_ICON : PAGE_ICON) + '</span>'
        + '<h2></h2>'
        + '<div class="meta">' + (a.kind === 'document' ? 'Document' : 'Page') + ' · v' + a.versions.length + ' · ' + ago(a.versions[a.versions.length - 1].published_at) + (m.machine ? '<span class="mbadge"></span>' : '') + '</div>'
        + '<div class="where"></div>';
      card.querySelector('h2').textContent = a.title;
      card.querySelector('.where').textContent = listing.place;
      if (m.machine) card.querySelector('.mbadge').textContent = m.machine;
      grid.appendChild(card);
    }
  }
  if (!shown) { const e = document.createElement('div'); e.id = 'empty'; e.textContent = machines.length ? 'No artifacts match.' : 'Nothing published yet. An agent’s agentz_artifact_publish makes one.'; grid.appendChild(e); }
}
document.querySelectorAll('.seg button').forEach((b) => b.onclick = () => { document.querySelectorAll('.seg button').forEach((x) => x.classList.remove('on')); b.classList.add('on'); filter = b.dataset.f; draw(); });
document.getElementById('q').oninput = (e) => { query = e.target.value.toLowerCase(); draw(); };
async function load() {
  try { machines = await (await fetch('/api/artifacts?t=' + encodeURIComponent(GALLERY.token))).json(); } catch (_) { machines = []; }
  draw();
}
load();
new EventSource('/events?t=' + encodeURIComponent(GALLERY.token)).onmessage = (e) => { const d = JSON.parse(e.data); if (d.type !== 'theme') load(); else load(); };
</script>
</body>
</html>"##;

/// A Markdown artifact as a document page (topic 13 B): a 720px column, 15px text, a larger
/// title, ruled tables, code in the theme's colors, printing well. Code blocks are
/// unhighlighted, as the app's own markdown draws them.
fn document_page(title: &str, source: &str, theme: &PageTheme, print: bool) -> String {
    let body = markdown_html(source);
    let css = r##"
* { box-sizing: border-box; }
body { margin: 0; background: var(--background, #1e1e1e); color: var(--text, #ccc); font: 15px/1.65 var(--font-sans, system-ui, sans-serif); }
.doc { max-width: 720px; margin: 0 auto; padding: 48px 24px 96px; }
h1 { font-size: 27px; line-height: 1.25; margin: 0 0 18px; }
h2 { font-size: 20px; margin: 34px 0 12px; }
h3, h4 { margin: 26px 0 10px; }
p, ul, ol, pre, table, blockquote { margin: 0 0 16px; }
code { font: 13px var(--font-mono, ui-monospace, monospace); background: var(--surface, var(--panel, #252526)); border: 1px solid var(--border, #333); border-radius: 4px; padding: 1px 5px; }
pre { background: var(--surface, var(--panel, #252526)); border: 1px solid var(--border, #333); border-radius: 8px; padding: 12px 14px; overflow-x: auto; }
pre code { background: none; border: none; padding: 0; }
table { border-collapse: collapse; width: 100%; font-size: 14px; }
th, td { border: 1px solid var(--border, #333); padding: 6px 12px; text-align: left; }
th { background: var(--surface, var(--panel, #252526)); }
blockquote { border-left: 3px solid var(--border, #333); padding-left: 14px; color: var(--muted, #888); }
a { color: var(--accent, #548af7); }
img { max-width: 100%; }
hr { border: none; border-top: 1px solid var(--border, #333); margin: 28px 0; }
@media print { body { background: #fff; color: #000; } .doc { max-width: none; padding: 0; } }
"##;
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><style>{}{}</style></head><body><div class=\"doc\">{}</div>{}</body></html>",
        esc(title),
        vars_style(theme),
        css,
        body,
        bridge_script(print)
    )
}

/// Markdown to HTML for the blocks an agent writes: headings, lists, tables, code, quotes,
/// links and images. Raw HTML in the source is escaped; the page has the whole window.
fn markdown_html(source: &str) -> String {
    use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(source, options);
    let mut out = String::new();
    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => out.push_str("<p>"),
                Tag::Heading { level, .. } => {
                    let level = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    out.push_str(&format!("<h{level}>"));
                }
                Tag::BlockQuote(_) => out.push_str("<blockquote>"),
                Tag::CodeBlock(kind) => {
                    let class = match kind {
                        CodeBlockKind::Fenced(language) if !language.is_empty() => {
                            format!(" class=\"language-{}\"", esc(&language))
                        }
                        _ => String::new(),
                    };
                    out.push_str(&format!("<pre><code{class}>"));
                }
                Tag::List(start) => {
                    if let Some(start) = start {
                        out.push_str(&format!("<ol start=\"{start}\">"));
                    } else {
                        out.push_str("<ul>");
                    }
                }
                Tag::Item => out.push_str("<li>"),
                Tag::Table(_) => out.push_str("<table>"),
                Tag::TableHead => out.push_str("<thead><tr>"),
                Tag::TableRow => out.push_str("<tr>"),
                Tag::TableCell => out.push_str(if in_head(&out) { "<th>" } else { "<td>" }),
                Tag::Emphasis => out.push_str("<em>"),
                Tag::Strong => out.push_str("<strong>"),
                Tag::Strikethrough => out.push_str("<del>"),
                Tag::Link { dest_url, .. } => {
                    out.push_str(&format!(
                        "<a href=\"{}\" target=\"_blank\" rel=\"noreferrer\">",
                        esc(&dest_url)
                    ));
                }
                Tag::Image {
                    dest_url, title, ..
                } => {
                    out.push_str(&format!(
                        "<img src=\"{}\" title=\"{}\" alt=\"",
                        esc(&dest_url),
                        esc(&title)
                    ));
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => out.push_str("</p>"),
                TagEnd::Heading(level) => {
                    let level = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    out.push_str(&format!("</h{level}>"));
                }
                TagEnd::BlockQuote(_) => out.push_str("</blockquote>"),
                TagEnd::CodeBlock => out.push_str("</code></pre>"),
                TagEnd::List(true) => out.push_str("</ol>"),
                TagEnd::List(false) => out.push_str("</ul>"),
                TagEnd::Item => out.push_str("</li>"),
                TagEnd::Table => out.push_str("</table>"),
                TagEnd::TableHead => out.push_str("</tr></thead>"),
                TagEnd::TableRow => out.push_str("</tr>"),
                TagEnd::TableCell => out.push_str(if in_head(&out) { "</th>" } else { "</td>" }),
                TagEnd::Emphasis => out.push_str("</em>"),
                TagEnd::Strong => out.push_str("</strong>"),
                TagEnd::Strikethrough => out.push_str("</del>"),
                TagEnd::Link => out.push_str("</a>"),
                TagEnd::Image => out.push_str("\">"),
                _ => {}
            },
            Event::Text(text) => out.push_str(&esc(&text)),
            Event::Code(code) => out.push_str(&format!("<code>{}</code>", esc(&code))),
            Event::SoftBreak => out.push('\n'),
            Event::HardBreak => out.push_str("<br>"),
            Event::Rule => out.push_str("<hr>"),
            Event::Html(html) | Event::InlineHtml(html) => out.push_str(&esc(&html)),
            _ => {}
        }
    }
    out
}

/// Whether the table row being written is the head's.
fn in_head(out: &str) -> bool {
    match (out.rfind("<thead>"), out.rfind("</thead>")) {
        (Some(head), None) => {
            _ = head;
            true
        }
        (Some(head), Some(body)) => head > body,
        _ => false,
    }
}
