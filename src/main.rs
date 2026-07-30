use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = std::env::var("HERDR_SOCKET_PATH")?;
    let session = std::env::var("HERDR_SESSION")?;

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
