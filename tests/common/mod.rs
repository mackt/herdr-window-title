#![allow(dead_code)] // shared across test crates; each uses a subset
//! Fake herdr server: the primary test seam. Speaks herdr's
//! newline-delimited JSON protocol on a unix socket, records every request
//! with its arrival time, and answers from a mutable canned snapshot.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct FakeHerdr {
    pub socket_path: PathBuf,
    pub dir: tempfile::TempDir,
    requests: Arc<Mutex<Vec<(Instant, serde_json::Value)>>>,
    snapshot: Arc<Mutex<serde_json::Value>>,
    fail_snapshot: Arc<AtomicBool>,
}

impl FakeHerdr {
    pub fn start() -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let socket_path = dir.path().join("herdr.sock");
        let listener = UnixListener::bind(&socket_path).expect("bind fake herdr socket");
        let requests: Arc<Mutex<Vec<(Instant, serde_json::Value)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let snapshot = Arc::new(Mutex::new(serde_json::json!({})));
        let fail_snapshot = Arc::new(AtomicBool::new(false));

        let thread_requests = Arc::clone(&requests);
        let thread_snapshot = Arc::clone(&snapshot);
        let thread_fail = Arc::clone(&fail_snapshot);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { break };
                let requests = Arc::clone(&thread_requests);
                let snapshot = Arc::clone(&thread_snapshot);
                let fail = Arc::clone(&thread_fail);
                std::thread::spawn(move ||

                    serve_connection(stream, requests, snapshot, fail));
            }
        });

        Self {
            socket_path,
            dir,
            requests,
            snapshot,
            fail_snapshot,
        }
    }

    pub fn set_snapshot(&self, snapshot: serde_json::Value) {
        *self.snapshot.lock().expect("snapshot lock") = snapshot;
    }

    pub fn fail_snapshot_requests(&self, fail: bool) {
        self.fail_snapshot.store(fail, Ordering::SeqCst);
    }

    pub fn requests(&self) -> Vec<serde_json::Value> {
        self.requests
            .lock()
            .expect("requests lock")
            .iter()
            .map(|(_, request)| request.clone())
            .collect()
    }

    /// Titles from every `client.window_title.set` seen so far, oldest first.
    pub fn titles(&self) -> Vec<String> {
        self.requests()
            .into_iter()
            .filter(|request| request["method"] == "client.window_title.set")
            .filter_map(|request| request["params"]["title"].as_str().map(str::to_string))
            .collect()
    }

    /// Timestamps of every title write, for cadence assertions.
    pub fn title_instants(&self) -> Vec<Instant> {
        self.requests
            .lock()
            .expect("requests lock")
            .iter()
            .filter(|(_, request)| request["method"] == "client.window_title.set")
            .map(|(instant, _)| *instant)
            .collect()
    }

    /// Wait until `predicate` holds over the title list, or time out.
    pub fn wait_for_titles<F>(&self, timeout: Duration, predicate: F) -> Vec<String>
    where
        F: Fn(&[String]) -> bool,
    {
        let deadline = Instant::now() + timeout;
        loop {
            let titles = self.titles();
            if predicate(&titles) {
                return titles;
            }
            if Instant::now() >= deadline {
                panic!("timed out waiting for titles; saw {titles:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn wait_for_title(&self, expected: &str) -> Vec<String> {
        let expected = expected.to_string();
        self.wait_for_titles(DEFAULT_TIMEOUT, |titles| {
            titles.iter().any(|title| *title == expected)
        })
    }
}

fn serve_connection(
    stream: UnixStream,
    requests: Arc<Mutex<Vec<(Instant, serde_json::Value)>>>,
    snapshot: Arc<Mutex<serde_json::Value>>,
    fail_snapshot: Arc<AtomicBool>,
) {
    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut writer = stream;
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return,
            Ok(_) if line.trim().is_empty() => return,
            Ok(_) => {}
        }
        let request: serde_json::Value =
            serde_json::from_str(&line).expect("request is one JSON line");
        let response = respond_to(&request, &snapshot, &fail_snapshot);
        requests
            .lock()
            .expect("requests lock")
            .push((Instant::now(), request));
        let mut payload = response.to_string();
        payload.push('\n');
        if writer.write_all(payload.as_bytes()).is_err() {
            return;
        }
    }
}

fn respond_to(
    request: &serde_json::Value,
    snapshot: &Mutex<serde_json::Value>,
    fail_snapshot: &AtomicBool,
) -> serde_json::Value {
    let id = request["id"].clone();
    match request["method"].as_str() {
        Some("session.snapshot") => {
            if fail_snapshot.load(Ordering::SeqCst) {
                serde_json::json!({
                    "id": id,
                    "error": {"code": "internal_error", "message": "snapshot unavailable"},
                })
            } else {
                let snapshot = snapshot.lock().expect("snapshot lock").clone();
                serde_json::json!({
                    "id": id,
                    "result": {"type": "session_snapshot", "snapshot": snapshot},
                })
            }
        }
        Some("client.window_title.set") => serde_json::json!({
            "id": id,
            "result": {"type": "client_window_title", "changed": true, "reason": "set"},
        }),
        other => serde_json::json!({
            "id": id,
            "error": {"code": "not_implemented", "message": format!("{other:?}")},
        }),
    }
}

/// Run the hook binary against the fake, with a clean env plus `envs`.
pub fn run_hook(socket_path: &Path, envs: &[(&str, &str)]) -> std::process::ExitStatus {
    hook_command(socket_path, envs).status().expect("hook binary runs")
}

/// Like `run_hook`, but captures stdout/stderr for warning assertions.
pub fn run_hook_capture(socket_path: &Path, envs: &[(&str, &str)]) -> std::process::Output {
    hook_command(socket_path, envs).output().expect("hook binary runs")
}

fn hook_command(socket_path: &Path, envs: &[(&str, &str)]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_herdr-window-title"));
    command
        .env_remove("HERDR_SESSION")
        .env_remove("HERDR_BIN_PATH")
        .env_remove("HERDR_PLUGIN_CONFIG_DIR")
        .env("HERDR_SOCKET_PATH", socket_path);
    for (key, value) in envs {
        command.env(key, value);
    }
    command
}

/// Write an executable fake `herdr` CLI that prints canned `status --json` output.
pub fn write_fake_herdr_bin(dir: &Path, status_json: &str) -> PathBuf {
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
