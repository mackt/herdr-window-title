use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = std::env::var("HERDR_SOCKET_PATH")?;
    let session = resolve_session();

    let request = serde_json::json!({
        "id": "herdr-window-title:set",
        "method": "client.window_title.set",
        "params": {"title": format!("herdr:{session}")},
    });

    let mut stream = UnixStream::connect(&socket_path)?;
    let mut payload = request.to_string();
    payload.push('\n');
    stream.write_all(payload.as_bytes())?;
    stream.flush()?;

    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    Ok(())
}

/// Session name, in resolution order: HERDR_SESSION env (inherited from the
/// server process on named sessions), then `herdr status --json`, then the
/// literal name of herdr's unnamed session.
fn resolve_session() -> String {
    if let Ok(session) = std::env::var("HERDR_SESSION") {
        return session;
    }
    session_from_status().unwrap_or_else(|| "default".into())
}

fn session_from_status() -> Option<String> {
    let herdr_bin = std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".into());
    let output = std::process::Command::new(herdr_bin)
        .args(["status", "--json"])
        .output()
        .ok()?;
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    Some(status["server"]["session"].as_str()?.to_string())
}
