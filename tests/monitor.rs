//! The single-writer monitor, observed purely at the fake herdr socket:
//! animation cadence, keepalive re-assertion, dedup, and takeover.

mod common;

use common::{run_hook, FakeHerdr};
use std::time::Duration;

fn working_snapshot() -> serde_json::Value {
    serde_json::json!({
        "focused_workspace_id": "w1",
        "focused_tab_id": "w1:t1",
        "focused_pane_id": "w1:p1",
        "workspaces": [{"workspace_id": "w1", "label": "one", "focused": true}],
        "tabs": [],
        "agents": [
            {"pane_id": "w1:p1", "workspace_id": "w1", "agent": "claude", "agent_status": "working", "focused": true},
        ],
        "panes": [],
    })
}

fn idle_snapshot() -> serde_json::Value {
    serde_json::json!({
        "focused_workspace_id": "w1",
        "workspaces": [{"workspace_id": "w1", "label": "one", "focused": true}],
        "tabs": [], "agents": [], "panes": [],
    })
}

fn write_config(dir: &std::path::Path, contents: &str) -> tempfile::TempDir {
    let config_dir = tempfile::tempdir().expect("config dir");
    let _ = dir; // config lives in its own dir; state lives next to the socket
    std::fs::write(config_dir.path().join("config.toml"), contents).expect("write config");
    config_dir
}

fn hook_env(config_dir: &tempfile::TempDir) -> Vec<(&str, &str)> {
    vec![
        ("HERDR_SESSION", "personal"),
        (
            "HERDR_PLUGIN_CONFIG_DIR",
            config_dir.path().to_str().expect("utf8 path"),
        ),
    ]
}

#[test]
fn spinner_animates_while_the_focused_agent_works() {
    let fake = FakeHerdr::start();
    fake.set_snapshot(working_snapshot());
    let config_dir = write_config(fake.dir.path(), "spinner_interval_ms = 50");

    run_hook(&fake.socket_path, &hook_env(&config_dir));

    let titles = fake.wait_for_titles(Duration::from_secs(3), |titles| {
        let distinct: std::collections::HashSet<&String> = titles.iter().collect();
        distinct.len() >= 2
    });
    assert!(
        titles.iter().all(|title| title.ends_with("herdr:personal")),
        "every frame keeps the template body: {titles:?}"
    );
}

#[test]
fn keepalive_reasserts_the_title_for_reattached_clients() {
    let fake = FakeHerdr::start();
    fake.set_snapshot(idle_snapshot());
    let config_dir = write_config(fake.dir.path(), "idle_keepalive_ms = 100");

    run_hook(&fake.socket_path, &hook_env(&config_dir));

    let titles = fake.wait_for_titles(Duration::from_secs(3), |titles| titles.len() >= 3);
    assert!(
        titles.iter().all(|title| title == "herdr:personal"),
        "keepalive re-sends the same title unconditionally: {titles:?}"
    );
}

#[test]
fn duplicate_renders_between_keepalives_are_suppressed() {
    let fake = FakeHerdr::start();
    fake.set_snapshot(idle_snapshot());
    let config_dir =
        write_config(fake.dir.path(), "idle_keepalive_ms = 60000\nspinner_interval_ms = 60000");

    run_hook(&fake.socket_path, &hook_env(&config_dir));
    fake.wait_for_title("herdr:personal");

    // A second poke with unchanged state must not produce a second write.
    run_hook(&fake.socket_path, &hook_env(&config_dir));
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(
        fake.titles().len(),
        1,
        "identical render between keepalives was re-sent"
    );
}

#[test]
fn a_new_monitor_takes_over_after_the_old_one_dies() {
    let fake = FakeHerdr::start();
    fake.set_snapshot(idle_snapshot());
    let config_dir = write_config(fake.dir.path(), "idle_keepalive_ms = 100");
    let env = hook_env(&config_dir);

    run_hook(&fake.socket_path, &env);
    fake.wait_for_titles(Duration::from_secs(3), |titles| !titles.is_empty());

    let pid_path = fake.dir.path().join("monitor.pid");
    let old_pid = std::fs::read_to_string(&pid_path).expect("monitor writes its pid");
    let killed = std::process::Command::new("kill")
        .args(["-9", old_pid.trim()])
        .status()
        .expect("kill runs");
    assert!(killed.success(), "old monitor was killable");
    std::thread::sleep(Duration::from_millis(200));

    let seen_before_takeover = fake.titles().len();
    run_hook(&fake.socket_path, &env);

    fake.wait_for_titles(Duration::from_secs(3), |titles| {
        titles.len() > seen_before_takeover
    });
    let new_pid = std::fs::read_to_string(&pid_path).expect("new monitor pid");
    assert_ne!(old_pid.trim(), new_pid.trim(), "a fresh monitor took over");
}
