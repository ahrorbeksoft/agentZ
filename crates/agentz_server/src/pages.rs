//! The pages server (design/artifacts, design/artifacts-2): serves the artifacts of every
//! machine the app reaches to the user's browser: the artifacts page at `/`, a document's
//! page, and the design page agentZ draws, which saves the user's picks as they go. It listens
//! on `127.0.0.1` only and takes the token from `artifacts/server.json` on every page and API
//! link. What an agent made (a document, a variant's picture or page) is served with a
//! per-artifact key instead, sandboxed, so an agent's scripts can't use the token
//! (`design/artifacts` backlog: an agent's scripts run in the user's browser). The port and
//! token are kept across restarts and server handoffs, so open pages and links keep working.
//!
//! Another machine's artifacts come through the app: a request naming one is passed to the
//! server actor ([`PageAsk`]), which relays it as [`Event::RelayArtifact`]. Pages hear of
//! publishes, picks, deletions and theme changes over `/events` (SSE). The pages themselves
//! are in `pages/`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use agentz_protocol::artifacts::{
    ArtifactId, ArtifactKind, ArtifactListing, ArtifactReply, ArtifactRequest, PageTheme,
    TopicPicks,
};
use agentz_protocol::{Event, PeerMachine};
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
/// The most a request's head or body may hold.
const MAX_HEAD: usize = 16 * 1024;
const MAX_BODY: usize = 1024 * 1024;
/// A relayed read through the app may take a moment on a slow link.
const RELAY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// What an agent made runs in an origin of its own even when it's opened outside its frame, so
/// its scripts can't reach the pages that hold the token.
const SANDBOX: &str = "sandbox allow-scripts allow-downloads allow-modals allow-forms allow-popups \
                       allow-popups-to-escape-sandbox";

const SHARED_CSS: &str = include_str!("pages/shared.css");
const SHARED_JS: &str = include_str!("pages/shared.js");
const HOME: &str = include_str!("pages/home.html");
const DOCUMENT: &str = include_str!("pages/document.html");
const DOCUMENT_CSS: &str = include_str!("pages/document.css");
const DESIGN: &str = include_str!("pages/design.html");

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
    /// Published again, or its picks changed; `machine` is `None` for this one.
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
    /// Where the app keeps its copies of the picture behind its window, in the data
    /// directory it shares with this server.
    backgrounds: PathBuf,
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
        backgrounds: data_dir.join("backgrounds"),
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

type Writing = tokio::net::tcp::OwnedWriteHalf;
type Reading = BufReader<tokio::net::tcp::OwnedReadHalf>;

async fn serve(
    stream: tokio::net::TcpStream,
    pages: Pages,
    inputs: mpsc::UnboundedSender<Input>,
) -> Result<()> {
    let (reading, mut writing) = stream.into_split();
    let mut reader = BufReader::new(reading);
    // The request line and headers, capped; only the API's POSTs carry a body.
    let mut head = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            return Ok(());
        }
        if head.len() + line.len() > MAX_HEAD {
            return text_reply(
                &mut writing,
                "431 Request Header Fields Too Large",
                "too large",
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
    let has_token = given.as_deref() == Some(pages.token.as_str());
    // The frame key opens only what an agent made: a document's content and a version's files.
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
        if !has_token {
            return forbidden(&mut writing).await;
        }
        return events(&mut writing, &pages).await;
    }

    // Everything outside an artifact's own paths needs the token; those check it or the key.
    match segments.first().map(String::as_str) {
        Some("a") | Some("m") => {}
        _ if !has_token => return forbidden(&mut writing).await,
        _ => {}
    }

    if at(&[]) {
        let project = query.get("p").filter(|project| !project.is_empty());
        return html_reply(&mut writing, home(&pages, project.as_deref())).await;
    }
    if segments.len() == 2 && segments[0] == "background" {
        return background(&mut writing, &pages, &segments[1]).await;
    }
    if at(&["api", "artifacts"]) {
        return json_reply(&mut writing, &everything(&inputs).await).await;
    }
    if at(&["api", "search"]) {
        let text = query.get("q").unwrap_or_default();
        return json_reply(&mut writing, &search(&inputs, &text).await).await;
    }
    let machine = query.get("machine").filter(|machine| !machine.is_empty());
    if at(&["api", "project-icon"]) {
        let project = query
            .get("project")
            .and_then(|project| project.parse().ok())
            .map(projects::ProjectId);
        let Some(project) = project else {
            return text_reply(&mut writing, "404 Not Found", "no such project").await;
        };
        return match ask(&inputs, machine, ArtifactRequest::ProjectIcon(project)).await {
            Ok(ArtifactReply::ProjectIcon { name, data }) => {
                use base64::Engine as _;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(data)
                    .unwrap_or_default();
                // A project's own file, so an SVG one's scripts mustn't run as the pages.
                let headers = [
                    ("content-security-policy", "sandbox".to_string()),
                    ("cache-control", "private, max-age=300".to_string()),
                ];
                let content_type = icon_type(&name).unwrap_or("application/octet-stream");
                reply(&mut writing, "200 OK", content_type, bytes, &headers).await
            }
            Ok(_) => text_reply(&mut writing, "404 Not Found", "no such icon").await,
            Err(error) => text_reply(&mut writing, "404 Not Found", &error).await,
        };
    }
    if segments.len() == 3 && segments[0] == "api" && segments[1] == "artifact" {
        let id = ArtifactId(segments[2].clone());
        return match ask(&inputs, machine, ArtifactRequest::Listing(id)).await {
            Ok(ArtifactReply::Listing(listing)) => json_reply(&mut writing, &listing).await,
            Ok(_) => text_reply(&mut writing, "404 Not Found", "no such artifact").await,
            Err(error) => text_reply(&mut writing, "404 Not Found", &error).await,
        };
    }
    if segments.len() == 3 && segments[0] == "api" && segments[1] == "design" {
        let id = ArtifactId(segments[2].clone());
        let version = version_of(query.get("v").as_deref());
        return match ask(&inputs, machine, ArtifactRequest::Design { id, version }).await {
            Ok(ArtifactReply::Design {
                listing,
                version,
                design,
                picks,
            }) => {
                let reply = json!({
                    "listing": listing,
                    "version": version,
                    "design": design,
                    "picks": picks,
                });
                json_reply(&mut writing, &reply).await
            }
            Ok(_) => text_reply(&mut writing, "404 Not Found", "no such design").await,
            Err(error) => text_reply(&mut writing, "404 Not Found", &error).await,
        };
    }
    if method == "POST" && (at(&["api", "picks"]) || at(&["api", "submit"])) {
        let Some(body) = read_body(&mut reader, &mut writing, content_length).await? else {
            return Ok(());
        };
        let body: Value = serde_json::from_slice(&body).unwrap_or_default();
        let machine = body["machine"]
            .as_str()
            .filter(|machine| !machine.is_empty())
            .map(str::to_string);
        let id = ArtifactId(body["id"].as_str().unwrap_or_default().to_string());
        let version = body["version"].as_u64().unwrap_or_default() as u32;
        let request = if at(&["api", "picks"]) {
            let picks: TopicPicks = match serde_json::from_value(body["picks"].clone()) {
                Ok(picks) => picks,
                Err(error) => return error_reply(&mut writing, &error.to_string()).await,
            };
            ArtifactRequest::SetPicks {
                id,
                version,
                topic: body["topic"].as_u64().unwrap_or_default() as u32,
                picks,
            }
        } else {
            ArtifactRequest::Submit {
                id,
                version,
                send: body["send"].as_bool().unwrap_or_default(),
                note: body["note"].as_str().unwrap_or_default().to_string(),
            }
        };
        return match ask(&inputs, machine, request).await {
            Ok(_) => json_reply(&mut writing, &json!({})).await,
            Err(error) => error_reply(&mut writing, &error).await,
        };
    }
    if at(&["api", "show-thread"]) && method == "POST" {
        let Some(body) = read_body(&mut reader, &mut writing, content_length).await? else {
            return Ok(());
        };
        let body: Value = serde_json::from_slice(&body).unwrap_or_default();
        let machine = body["machine"]
            .as_str()
            .filter(|machine| !machine.is_empty())
            .map(str::to_string);
        let thread = body["thread"].as_u64().unwrap_or_default();
        let event = Event::ShowThread {
            machine,
            thread_id: projects::ThreadId(thread),
        };
        let tell = Input::Run(Box::new(move |server: &mut crate::server::Server| {
            server.tell_clients(event);
        }));
        inputs.unbounded_send(tell).ok();
        return json_reply(&mut writing, &json!({})).await;
    }

    // /a/<id>, with the machine's /m/<machine>/a/<id>.
    let (machine, rest) = match segments.first().map(String::as_str) {
        Some("m") if segments.len() >= 3 => (Some(segments[1].clone()), &segments[2..]),
        Some("a") => (None, &segments[..]),
        _ => return text_reply(&mut writing, "404 Not Found", "not found").await,
    };
    if rest.len() < 2 || rest[0] != "a" {
        return text_reply(&mut writing, "404 Not Found", "not found").await;
    }
    let id = ArtifactId(rest[1].clone());
    if rest.len() == 2 {
        if !has_token {
            return forbidden(&mut writing).await;
        }
        let version = version_of(query.get("v").as_deref());
        let page = match ask(
            &inputs,
            machine.clone(),
            ArtifactRequest::Listing(id.clone()),
        )
        .await
        {
            Ok(ArtifactReply::Listing(listing)) => {
                artifact_page(&pages, machine.as_deref(), &listing, version)
            }
            Ok(_) => message_page(&pages, "This artifact is gone."),
            Err(error) => message_page(&pages, &error),
        };
        return html_reply(&mut writing, page).await;
    }
    let Some(version) = version_of(rest.get(3).map(String::as_str)) else {
        return text_reply(&mut writing, "404 Not Found", "not found").await;
    };
    let listing = match ask(
        &inputs,
        machine.clone(),
        ArtifactRequest::Listing(id.clone()),
    )
    .await
    {
        Ok(ArtifactReply::Listing(listing)) => listing,
        _ => return text_reply(&mut writing, "404 Not Found", "no such artifact").await,
    };
    let kind = listing.artifact.kind;
    match rest[2].as_str() {
        "content" if kind == ArtifactKind::Document => {
            if key.as_deref() != Some(listing.artifact.frame_key.as_str()) {
                return forbidden(&mut writing).await;
            }
            let read = ArtifactRequest::Read {
                id,
                version: Some(version),
            };
            let source = match ask(&inputs, machine, read).await {
                Ok(ArtifactReply::Page { source, .. }) => source,
                _ => return text_reply(&mut writing, "404 Not Found", "no such version").await,
            };
            let print = query.get("print").is_some();
            let body = document_page(&listing.artifact.title, &source, &pages.theme(), print);
            reply(
                &mut writing,
                "200 OK",
                "text/html; charset=utf-8",
                body.into_bytes(),
                &[("content-security-policy", SANDBOX.to_string())],
            )
            .await
        }
        "files" => {
            let Some(name) = rest.get(4) else {
                return text_reply(&mut writing, "404 Not Found", "not found").await;
            };
            if key.as_deref() != Some(listing.artifact.frame_key.as_str()) {
                return forbidden(&mut writing).await;
            }
            let file = ArtifactRequest::File {
                id,
                version,
                name: name.clone(),
            };
            let data = match ask(&inputs, machine, file).await {
                Ok(ArtifactReply::File(data)) => data,
                _ => return text_reply(&mut writing, "404 Not Found", "no such file").await,
            };
            use base64::Engine as _;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .unwrap_or_default();
            let content_type = shown_as(name);
            let disposition = if content_type.is_some() {
                "inline"
            } else {
                "attachment"
            };
            // A version's files never change.
            let headers = [
                (
                    "content-disposition",
                    format!("{disposition}; filename=\"{}\"", name.replace('"', "_")),
                ),
                ("content-security-policy", SANDBOX.to_string()),
                ("cache-control", "private, max-age=86400".to_string()),
            ];
            reply(
                &mut writing,
                "200 OK",
                content_type.unwrap_or("application/octet-stream"),
                bytes,
                &headers,
            )
            .await
        }
        "raw" if kind == ArtifactKind::Document => {
            if !has_token {
                return forbidden(&mut writing).await;
            }
            let read = ArtifactRequest::Read {
                id,
                version: Some(version),
            };
            match ask(&inputs, machine, read).await {
                Ok(ArtifactReply::Page { source, .. }) => {
                    reply(
                        &mut writing,
                        "200 OK",
                        "text/markdown; charset=utf-8",
                        source.into_bytes(),
                        &[],
                    )
                    .await
                }
                _ => text_reply(&mut writing, "404 Not Found", "no such version").await,
            }
        }
        _ => text_reply(&mut writing, "404 Not Found", "not found").await,
    }
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

fn version_of(text: Option<&str>) -> Option<u32> {
    text.and_then(|version| version.parse::<u32>().ok())
        .filter(|version| *version > 0)
}

/// A project's icon file's type: a picture's, or a favicon's.
fn icon_type(name: &str) -> Option<&'static str> {
    if name
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("ico"))
    {
        return Some("image/x-icon");
    }
    shown_as(name).filter(|content_type| content_type.starts_with("image/"))
}

/// The type a version's file is shown with in the browser: a picture or a page. Anything else
/// downloads.
fn shown_as(name: &str) -> Option<&'static str> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "html" | "htm" => "text/html; charset=utf-8",
        _ => return None,
    })
}

/// A machine's artifacts as the artifacts page gets them: an offline machine's are left out,
/// and the page says so (topic 7 A).
#[derive(Serialize)]
struct MachineArtifacts {
    machine: Option<String>,
    online: bool,
    artifacts: Vec<ArtifactListing>,
}

/// Every machine's artifacts, this one's first.
async fn everything(inputs: &mpsc::UnboundedSender<Input>) -> Vec<MachineArtifacts> {
    let (_, peers) = machines(inputs).await;
    let lists = futures::future::join_all(
        std::iter::once(None)
            .chain(
                peers
                    .iter()
                    .filter(|peer| peer.online)
                    .map(|peer| Some(peer.name.clone())),
            )
            .map(|machine| async move {
                let artifacts = match ask(inputs, machine.clone(), ArtifactRequest::List).await {
                    Ok(ArtifactReply::List(listings)) => listings,
                    _ => Vec::new(),
                };
                MachineArtifacts {
                    machine,
                    online: true,
                    artifacts,
                }
            }),
    )
    .await;
    let offline = peers
        .into_iter()
        .filter(|peer| !peer.online)
        .map(|peer| MachineArtifacts {
            machine: Some(peer.name),
            online: false,
            artifacts: Vec::new(),
        });
    lists.into_iter().chain(offline).collect()
}

#[derive(Serialize)]
struct Found {
    machine: Option<String>,
    id: ArtifactId,
    line: String,
}

/// What the search finds inside every online machine's artifacts (topic 5 A).
async fn search(inputs: &mpsc::UnboundedSender<Input>, text: &str) -> Vec<Found> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    let (_, peers) = machines(inputs).await;
    let machines = std::iter::once(None).chain(
        peers
            .into_iter()
            .filter(|peer| peer.online)
            .map(|peer| Some(peer.name)),
    );
    let found = futures::future::join_all(machines.map(|machine| async move {
        match ask(
            inputs,
            machine.clone(),
            ArtifactRequest::Search(text.to_string()),
        )
        .await
        {
            Ok(ArtifactReply::Found(found)) => found
                .into_iter()
                .map(|(id, line)| Found {
                    machine: machine.clone(),
                    id,
                    line,
                })
                .collect(),
            _ => Vec::new(),
        }
    }))
    .await;
    found.into_iter().flatten().collect()
}

/// The other machines the app says it reaches, and what it calls this one.
async fn machines(inputs: &mpsc::UnboundedSender<Input>) -> (Option<String>, Vec<PeerMachine>) {
    let (answer, receive) = oneshot::channel();
    let ask = Input::Run(Box::new(move |server: &mut crate::server::Server| {
        server.answer_machines(answer);
    }));
    if inputs.unbounded_send(ask).is_err() {
        return (None, Vec::new());
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
    reader: &mut Reading,
    writing: &mut Writing,
    content_length: usize,
) -> Result<Option<Vec<u8>>> {
    if content_length > MAX_BODY {
        text_reply(writing, "413 Content Too Large", "too large").await?;
        return Ok(None);
    }
    let mut body = vec![0; content_length];
    reader.read_exact(&mut body).await?;
    Ok(Some(body))
}

/// The events stream: one `data:` line per [`PageEvent`], until the browser goes.
async fn events(writing: &mut Writing, pages: &Pages) -> Result<()> {
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
                json!({
                    "type": "theme",
                    "dark": theme.dark,
                    "variables": theme.variables,
                    "picture": picture_link(pages, &theme),
                    "chats": theme.chats,
                })
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

async fn json_reply(writing: &mut Writing, value: &impl Serialize) -> Result<()> {
    let body = serde_json::to_vec(value).context("encoding a reply")?;
    reply(writing, "200 OK", "application/json", body, &[]).await
}

async fn error_reply(writing: &mut Writing, error: &str) -> Result<()> {
    let body = json!({"error": error}).to_string().into_bytes();
    reply(writing, "400 Bad Request", "application/json", body, &[]).await
}

async fn html_reply(writing: &mut Writing, page: String) -> Result<()> {
    reply(
        writing,
        "200 OK",
        "text/html; charset=utf-8",
        page.into_bytes(),
        &[],
    )
    .await
}

async fn text_reply(writing: &mut Writing, status: &str, text: &str) -> Result<()> {
    reply(
        writing,
        status,
        "text/plain; charset=utf-8",
        text.as_bytes().to_vec(),
        &[],
    )
    .await
}

async fn forbidden(writing: &mut Writing) -> Result<()> {
    text_reply(writing, "403 Forbidden", "forbidden").await
}

async fn reply(
    writing: &mut Writing,
    status: &str,
    content_type: &str,
    body: Vec<u8>,
    extra: &[(&str, String)],
) -> Result<()> {
    let mut head = format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n",
        body.len()
    );
    if !extra
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("cache-control"))
    {
        head.push_str("cache-control: no-store\r\n");
    }
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

/// The picture behind the app's window, which only the theme names, so no other file of the
/// data directory is served.
async fn background(writing: &mut Writing, pages: &Pages, name: &str) -> Result<()> {
    let theme = pages.theme();
    let Some(picture) = theme.picture.as_deref().filter(|picture| *picture == name) else {
        return text_reply(writing, "404 Not Found", "no such picture").await;
    };
    let Some(path) = picture_file(&pages.backgrounds, picture) else {
        return text_reply(writing, "404 Not Found", "no such picture").await;
    };
    match tokio::fs::read(&path).await {
        // Each picture's copy has a name of its own, so it never changes.
        Ok(bytes) => {
            let headers = [("cache-control", "private, max-age=86400".to_string())];
            let content_type = shown_as(picture).unwrap_or("application/octet-stream");
            reply(writing, "200 OK", content_type, bytes, &headers).await
        }
        Err(error) => {
            log::debug!("couldn't read {}: {error}", path.display());
            text_reply(writing, "404 Not Found", "no such picture").await
        }
    }
}

/// The picture's copy in `backgrounds`, for a name that's only a file's.
fn picture_file(backgrounds: &Path, name: &str) -> Option<PathBuf> {
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    plain.then(|| backgrounds.join(name))
}

/// Where agentZ's own pages load the picture behind the window from. It carries the token, so
/// it's never given to what an agent made.
fn picture_link(pages: &Pages, theme: &PageTheme) -> Option<String> {
    let picture = theme.picture.as_deref()?;
    picture_file(&pages.backgrounds, picture)?;
    Some(format!("/background/{picture}?t={}", pages.token))
}

/// The picture behind the window as the `--picture` the pages' background draws.
fn picture_style(pages: &Pages, theme: &PageTheme) -> String {
    picture_link(pages, theme)
        // Only what can't end the rule it's in reaches the page's style.
        .filter(|link| {
            link.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./?=".contains(c))
        })
        .map(|link| format!(":root{{--picture:url(\"{link}\");}}"))
        .unwrap_or_default()
}

/// The theme as a `:root` rule, before the first paint as t3code does it.
fn vars_style(theme: &PageTheme) -> String {
    let mut css = String::from(":root{color-scheme:");
    css.push_str(if theme.dark { "dark" } else { "light" });
    css.push(';');
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

/// One of agentZ's own pages: the shared styles and helpers, the theme, and what the page's
/// script starts from.
fn own_page(template: &str, pages: &Pages, data: Value) -> String {
    let data = serde_json::to_string(&data)
        .unwrap_or_default()
        .replace('<', "\\u003c");
    let theme = pages.theme();
    template
        .replace("__SHARED_CSS__", SHARED_CSS)
        .replace(
            "__VARS__",
            &(vars_style(&theme) + &picture_style(pages, &theme)),
        )
        .replace("__SHARED_JS__", SHARED_JS)
        .replace("__DATA__", &format!("const DATA = {data};"))
}

/// The artifacts page (topic 4 A), at a project's place when the app's button named one.
fn home(pages: &Pages, project: Option<&str>) -> String {
    own_page(
        HOME,
        pages,
        json!({"token": pages.token, "project": project, "chats": pages.theme().chats}),
    )
}

/// A document's page (topic 8 A) or a design's (topics 9 to 19).
fn artifact_page(
    pages: &Pages,
    machine: Option<&str>,
    listing: &ArtifactListing,
    version: Option<u32>,
) -> String {
    let template = match listing.artifact.kind {
        ArtifactKind::Document => DOCUMENT,
        ArtifactKind::Design => DESIGN,
    };
    own_page(
        template,
        pages,
        json!({
            "token": pages.token,
            "id": listing.artifact.id.0,
            "machine": machine,
            "version": version,
        }),
    )
}

/// A page that only says something: the artifact is gone, or its machine can't be reached.
fn message_page(pages: &Pages, message: &str) -> String {
    let theme = pages.theme();
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>agentZ</title><style>{SHARED_CSS}{}{}</style></head><body><div class=\"message\">{}</div></body></html>",
        vars_style(&theme),
        picture_style(pages, &theme),
        esc(message)
    )
}

/// The script a document's content gets: theme changes reach it as messages, and printing
/// starts once it's drawn.
fn theme_script(print: bool) -> String {
    let print = if print {
        "try{window.print()}catch(_){}"
    } else {
        ""
    };
    format!(
        r#"<script>(function(){{
function apply(t){{var r=document.documentElement;if(t.dark!==undefined){{r.style.colorScheme=t.dark?'dark':'light';}}var v=t.variables||[];for(var i=0;i<v.length;i++)r.style.setProperty(v[i][0],v[i][1]);}}
window.addEventListener('message',function(e){{var d=e.data||{{}};if(d.agentzTheme)apply(d.agentzTheme);}});
{print}
}})();</script>"#
    )
}

/// A Markdown artifact as a document page (the first round's topic 13 B): a 720px column,
/// 15px text, a larger title, ruled tables, code in the theme's colors, printing well. Code
/// blocks are unhighlighted, as the app's own markdown draws them.
fn document_page(title: &str, source: &str, theme: &PageTheme, print: bool) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><style>{}{}</style></head><body><div class=\"doc\">{}</div>{}</body></html>",
        esc(title),
        vars_style(theme),
        DOCUMENT_CSS,
        markdown_html(source),
        theme_script(print)
    )
}

/// Markdown to HTML for the blocks an agent writes: headings, lists, tables, code, quotes,
/// links and images. Raw HTML in the source is escaped; the page has the whole window.
fn markdown_html(source: &str) -> String {
    use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
    let options =
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let parser = Parser::new_ext(source, options);
    let heading = |level: HeadingLevel| match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    };
    let mut out = String::new();
    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => out.push_str("<p>"),
                Tag::Heading { level, .. } => out.push_str(&format!("<h{}>", heading(level))),
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
                TagEnd::Heading(level) => out.push_str(&format!("</h{}>", heading(level))),
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
        (Some(_), None) => true,
        (Some(head), Some(body)) => head > body,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_show_as_pictures_and_pages_or_download() {
        assert_eq!(shown_as("plans-A.PNG"), Some("image/png"));
        assert_eq!(shown_as("plans-B.html"), Some("text/html; charset=utf-8"));
        assert_eq!(shown_as("report.csv"), None);
        assert_eq!(shown_as("README"), None);
        assert_eq!(icon_type("favicon.ICO"), Some("image/x-icon"));
        assert_eq!(icon_type("favicon.svg"), Some("image/svg+xml"));
        assert_eq!(icon_type("icon.html"), None);
    }

    #[test]
    fn only_a_plain_picture_name_is_served() {
        let backgrounds = Path::new("/data/backgrounds");
        assert_eq!(
            picture_file(backgrounds, "17-soft.jpg"),
            Some(backgrounds.join("17-soft.jpg"))
        );
        for name in ["", "..", ".hidden", "../state.json", "a/b.jpg", "a\\b.jpg"] {
            assert_eq!(picture_file(backgrounds, name), None, "{name}");
        }
    }

    #[test]
    fn markdown_escapes_raw_html() {
        let html = markdown_html(
            "# Title\n\n<script>alert(1)</script>\n\n| a | b |\n|---|---|\n| 1 | 2 |\n",
        );
        assert!(html.contains("<h1>Title</h1>"));
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("<th>a</th>"));
        assert!(html.contains("<td>1</td>"));
    }
}
