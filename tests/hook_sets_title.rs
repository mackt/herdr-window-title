//! Primary seam: a fake herdr socket. Tests boot the real binary against a
//! test-owned unix socket speaking herdr's newline-delimited JSON protocol
//! and assert only on what requests arrive there.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

const ACCEPT_TIMEOUT: Duration = Duration::from_secs(5);

/// Accept one connection, read one JSON request line, reply with a canned
/// success response, and return the parsed request.
fn serve_one_request(listener: UnixListener) -> mpsc::Receiver<serde_json::Value> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("hook never connected");
        stream
            .set_read_timeout(Some(ACCEPT_TIMEOUT))
            .expect("set read timeout");
        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read request line");
        let request: serde_json::Value =
            serde_json::from_str(&line).expect("request is one JSON line");
        respond_ok(&stream, &request);
        tx.send(request).ok();
    });
    rx
}

fn respond_ok(mut stream: &UnixStream, request: &serde_json::Value) {
    let response = serde_json::json!({
        "id": request["id"],
        "result": {"type": "client_window_title", "changed": true, "reason": "set"},
    });
    let mut payload = response.to_string();
    payload.push('\n');
    stream.write_all(payload.as_bytes()).expect("write response");
}

fn run_hook(socket_path: &Path, envs: &[(&str, &str)]) -> std::process::ExitStatus {
    let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-window-title"));
    command
        .env_remove("HERDR_SESSION")
        .env_remove("HERDR_BIN_PATH")
        .env("HERDR_SOCKET_PATH", socket_path);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.status().expect("hook binary runs")
}

/// Write an executable fake `herdr` CLI that prints canned `status --json` output.
fn write_fake_herdr_bin(dir: &Path, status_json: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin_path = dir.join("fake-herdr");
    std::fs::write(
        &bin_path,
        format!("#!/bin/sh\nprintf '%s\\n' '{status_json}'\n"),
    )
    .expect("write fake herdr bin");
    std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))
        .expect("chmod fake herdr bin");
    bin_path
}

#[test]
fn hook_sets_title_from_herdr_session_env() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("herdr.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind fake herdr socket");
    let request_rx = serve_one_request(listener);

    let status = run_hook(&socket_path, &[("HERDR_SESSION", "personal")]);

    let request = request_rx
        .recv_timeout(ACCEPT_TIMEOUT)
        .expect("hook sent a request to the herdr socket");
    assert_eq!(request["method"], "client.window_title.set");
    assert_eq!(request["params"]["title"], "herdr:personal");
    assert!(status.success(), "hook exits zero after the title is set");
}

#[test]
fn hook_falls_back_to_status_json_when_env_is_absent() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("herdr.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind fake herdr socket");
    let request_rx = serve_one_request(listener);
    let fake_bin = write_fake_herdr_bin(
        dir.path(),
        r#"{"client":{"session":"work"},"server":{"session":"work"}}"#,
    );

    let status = run_hook(
        &socket_path,
        &[("HERDR_BIN_PATH", fake_bin.to_str().expect("utf8 path"))],
    );

    let request = request_rx
        .recv_timeout(ACCEPT_TIMEOUT)
        .expect("hook sent a request to the herdr socket");
    assert_eq!(request["params"]["title"], "herdr:work");
    assert!(status.success());
}

#[test]
fn hook_defaults_session_name_when_nothing_resolves() {
    let dir = tempfile::tempdir().expect("temp dir");
    let socket_path = dir.path().join("herdr.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind fake herdr socket");
    let request_rx = serve_one_request(listener);
    let missing_bin = dir.path().join("no-such-herdr");

    let status = run_hook(
        &socket_path,
        &[("HERDR_BIN_PATH", missing_bin.to_str().expect("utf8 path"))],
    );

    let request = request_rx
        .recv_timeout(ACCEPT_TIMEOUT)
        .expect("hook sent a request to the herdr socket");
    assert_eq!(request["params"]["title"], "herdr:default");
    assert!(status.success());
}
