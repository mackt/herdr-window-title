use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixDatagram, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use herdr_window_title::config::{Config, DEFAULT_TEMPLATE};
use herdr_window_title::indicator::{activity, indicator, select_template, SPINNER_FRAMES};
use herdr_window_title::snapshot::token_values;
use herdr_window_title::template::Template;

/// The monitor ticks at least this often so it can notice a vanished herdr
/// server (and exit) even when configured cadences are long.
const MAX_TICK: Duration = Duration::from_secs(2);
/// Consecutive socket connection failures before the monitor concludes the
/// herdr server is gone and exits.
const MAX_SOCKET_FAILURES: u32 = 3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("monitor") => run_monitor(),
        _ => run_hook(),
    }
}

/// Hook mode: every herdr event invocation. Surfaces config warnings into
/// the plugin log, guarantees a monitor exists, pokes it, and exits — the
/// monitor is the only process that ever writes the title.
fn run_hook() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::resolve()?;

    let (config, warnings) = Config::load(paths.config_dir.as_deref());
    for warning in &warnings {
        eprintln!("{warning}");
    }
    // Pre-flight every configured template so syntax problems land in the
    // plugin log; the monitor renders silently with the same fallback.
    for source in [
        Some(&config.template),
        config.working_template.as_ref(),
        config.blocked_template.as_ref(),
        config.done_template.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Err(error) = Template::parse(source) {
            eprintln!("template: {}; the default template will be used", error.message);
        }
    }

    if try_lock_monitor(&paths).is_some() {
        // Nobody holds the lock: the lock we just probed is dropped here and
        // a detached monitor child races to take it. Losers exit on their own.
        std::process::Command::new(std::env::current_exe()?)
            .arg("monitor")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
    }

    // Wake the monitor now rather than waiting out its keepalive tick.
    let poke = UnixDatagram::unbound()?;
    let _ = poke.send_to(b"poke", paths.poke_socket());
    Ok(())
}

/// Monitor mode: the single writer. Holds the lock for its lifetime, wakes
/// on pokes or cadence ticks, re-reads config, re-renders, and only talks
/// to the outer terminal through `client.window_title.set`.
fn run_monitor() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::resolve()?;
    // Held for the process lifetime; kernel-released if we die.
    let Some(_lock) = try_lock_monitor(&paths) else {
        return Ok(()); // another monitor already owns the title
    };
    std::fs::write(paths.pid_file(), std::process::id().to_string())?;

    let poke_path = paths.poke_socket();
    let _ = std::fs::remove_file(&poke_path);
    let pokes = UnixDatagram::bind(&poke_path)?;

    let session = resolve_session();
    let host = short_hostname();
    let mut frame_index = 0usize;
    let mut socket_failures = 0u32;
    let mut last_sent: Option<String> = None;
    let mut last_sent_at = Instant::now();

    loop {
        let (config, _) = Config::load(paths.config_dir.as_deref());

        let snapshot = match fetch_snapshot(&paths.socket_path) {
            Ok(snapshot) => {
                socket_failures = 0;
                snapshot
            }
            Err(_) => {
                socket_failures += 1;
                if socket_failures >= MAX_SOCKET_FAILURES {
                    return Ok(()); // herdr is gone; a future event restarts us
                }
                serde_json::Value::Null
            }
        };

        let activity = activity(&snapshot, config.spinner_scope);
        let mut values = token_values(&snapshot, &session, &host);
        values.indicator = indicator(&activity, &config, SPINNER_FRAMES[frame_index]);
        let template = parse_or_default(select_template(&activity, &config));
        let title = template.render(&values);

        let keepalive = Duration::from_millis(config.idle_keepalive_ms);
        let changed = last_sent.as_deref() != Some(title.as_str());
        if changed || last_sent_at.elapsed() >= keepalive {
            if set_title(&paths.socket_path, &title).is_ok() {
                last_sent = Some(title);
                last_sent_at = Instant::now();
            } else {
                socket_failures += 1;
                if socket_failures >= MAX_SOCKET_FAILURES {
                    return Ok(());
                }
            }
        }

        let cadence = if activity.focused_working {
            frame_index = (frame_index + 1) % SPINNER_FRAMES.len();
            Duration::from_millis(config.spinner_interval_ms)
        } else {
            frame_index = 0;
            keepalive
        };
        pokes.set_read_timeout(Some(cadence.min(MAX_TICK)))?;
        let mut buf = [0u8; 8];
        let _ = pokes.recv(&mut buf); // wake on poke or timeout, either is a tick
    }
}

struct Paths {
    socket_path: String,
    state_dir: PathBuf,
    config_dir: Option<PathBuf>,
}

impl Paths {
    fn resolve() -> Result<Self, Box<dyn std::error::Error>> {
        let socket_path = std::env::var("HERDR_SOCKET_PATH")?;
        let state_dir = std::env::var_os("HERDR_PLUGIN_STATE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(&socket_path)
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_path_buf()
            });
        let config_dir = std::env::var_os("HERDR_PLUGIN_CONFIG_DIR").map(PathBuf::from);
        Ok(Self {
            socket_path,
            state_dir,
            config_dir,
        })
    }

    fn lock_file(&self) -> PathBuf {
        self.state_dir.join("monitor.lock")
    }

    fn pid_file(&self) -> PathBuf {
        self.state_dir.join("monitor.pid")
    }

    fn poke_socket(&self) -> PathBuf {
        self.state_dir.join("poke.sock")
    }
}

/// The monitor lock: kernel-released on process death, so stale locks from
/// crashes can never wedge the plugin. Some(file) means we now hold it.
fn try_lock_monitor(paths: &Paths) -> Option<std::fs::File> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(paths.lock_file())
        .ok()?;
    match file.try_lock() {
        Ok(()) => Some(file),
        Err(_) => None,
    }
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

fn set_title(socket_path: &str, title: &str) -> Result<(), Box<dyn std::error::Error>> {
    let request = serde_json::json!({
        "id": "herdr-window-title:set",
        "method": "client.window_title.set",
        "params": {"title": title},
    });
    send_request(socket_path, &request).map(|_| ())
}

/// The current session snapshot; `Err` only for transport failures (used to
/// detect a vanished server). An in-band error response degrades to `null`,
/// which renders as empty optional tokens.
fn fetch_snapshot(socket_path: &str) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let request = serde_json::json!({
        "id": "herdr-window-title:snapshot",
        "method": "session.snapshot",
    });
    let response = send_request(socket_path, &request)?;
    Ok(response["result"]["snapshot"].clone())
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
