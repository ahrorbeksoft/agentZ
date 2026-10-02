//! `agentz-server start`, `proxy` and `stop`, run as processes against a scratch data directory.

#![allow(
    clippy::disallowed_methods,
    reason = "a plain test, with no executor to block"
)]

use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};

use agentz_protocol::{ClientHello, ClientKind, PROTOCOL_VERSION, ServerWelcome};

fn server(data_dir: &Path, argument: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_agentz-server"));
    command
        .arg(argument)
        .env("AGENTZ_DATA_DIR", data_dir)
        .env("RUST_LOG", "warn");
    command
}

fn frame(message: &impl serde::Serialize) -> Vec<u8> {
    let body = serde_json::to_vec(message).expect("encodes");
    let mut frame = u32::try_from(body.len())
        .expect("fits")
        .to_le_bytes()
        .to_vec();
    frame.extend(body);
    frame
}

#[test]
fn starts_proxies_and_stops() {
    let data_dir = tempfile::tempdir().expect("temp dir");
    let socket = data_dir.path().join("server.sock");
    let pid_file = data_dir.path().join("server.pid");

    let status = server(data_dir.path(), "start").status().expect("start");
    assert!(status.success());
    assert!(socket.exists());
    let pid = std::fs::read_to_string(&pid_file).expect("pid file");
    // Starting again finds the running server.
    let status = server(data_dir.path(), "start").status().expect("start");
    assert!(status.success());
    assert_eq!(std::fs::read_to_string(&pid_file).expect("pid file"), pid);

    let mut proxy = server(data_dir.path(), "proxy")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("proxy");
    let mut stdin = proxy.stdin.take().expect("stdin");
    stdin
        .write_all(&frame(&ClientHello {
            protocol_version: PROTOCOL_VERSION,
            client_version: "0.0.0-test".into(),
            client_kind: ClientKind::App,
        }))
        .expect("hello");
    let mut stdout = proxy.stdout.take().expect("stdout");
    let mut length = [0; 4];
    stdout.read_exact(&mut length).expect("welcome length");
    let mut body = vec![0; u32::from_le_bytes(length) as usize];
    stdout.read_exact(&mut body).expect("welcome");
    let welcome: ServerWelcome = serde_json::from_slice(&body).expect("decodes");
    assert_eq!(welcome.pid.to_string(), pid);
    drop(stdin);
    assert!(proxy.wait().expect("proxy exits").success());

    let status = server(data_dir.path(), "stop").status().expect("stop");
    assert!(status.success());
    assert!(!socket.exists());
    assert!(!pid_file.exists());
}
