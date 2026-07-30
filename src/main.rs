use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

use herdr_window_title::config::{Config, DEFAULT_TEMPLATE};
use herdr_window_title::snapshot::token_values;
use herdr_window_title::template::Template;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = std::env::var("HERDR_SOCKET_PATH")?;
    let session = resolve_session();
    let host = short_hostname();

    let config_dir = std::env::var_os("HERDR_PLUGIN_CONFIG_DIR").map(std::path::PathBuf::from);
    let (config, warnings) = Config::load(config_dir.as_deref());
    for warning in &warnings {
        eprintln!("{warning}");
    }

    let snapshot = fetch_snapshot(&socket_path);
    let values = token_values(&snapshot, &session, &host);
    let template = parse_or_default(&config.template);
    let title = template.render(&values);

    let request = serde_json::json!({
        "id": "herdr-window-title:set",
        "method": "client.window_title.set",
        "params": {"title": title},
    });
    send_request(&socket_path, &request)?;
    Ok(())
}

/// A parsed template, falling back to the built-in default on syntax errors
/// so the title never disappears.
fn parse_or_default(source: &str) -> Template {
    match Template::parse(source) {
        Ok(template) => template,
        Err(error) => {
            eprintln!("template: {}; using default template", error.message);
            Template::parse(DEFAULT_TEMPLATE).expect("built-in template is valid")
        }
    }
}

/// One request, one response, own connection — mirrors herdr's CLI clients.
fn send_request(
    socket_path: &str,
    request: &serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(socket_path)?;
    let mut payload = request.to_string();
    payload.push('\n');
    stream.write_all(payload.as_bytes())?;
    stream.flush()?;

    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    Ok(serde_json::from_str(&response)?)
}

/// The current session snapshot, or `null` when unavailable — token
/// extraction turns that into empty optional tokens.
fn fetch_snapshot(socket_path: &str) -> serde_json::Value {
    let request = serde_json::json!({
        "id": "herdr-window-title:snapshot",
        "method": "session.snapshot",
    });
    match send_request(socket_path, &request) {
        Ok(response) => response["result"]["snapshot"].clone(),
        Err(_) => serde_json::Value::Null,
    }
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

fn short_hostname() -> String {
    std::process::Command::new("hostname")
        .arg("-s")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|name| name.trim().to_string())
        .unwrap_or_default()
}
