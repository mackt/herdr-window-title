//! Primary seam: hook-mode behaviour observed at the fake herdr socket.

mod common;

use common::{run_hook, FakeHerdr};

#[test]
fn hook_sets_title_from_herdr_session_env() {
    let fake = FakeHerdr::start();

    let status = run_hook(&fake.socket_path, &[("HERDR_SESSION", "personal")]);

    fake.wait_for_title("herdr:personal");
    assert!(status.success(), "hook exits zero after the title is set");
}

#[test]
fn session_name_derives_from_a_named_session_socket_path() {
    let fake = FakeHerdr::start_at("sessions/work/herdr.sock");

    let status = run_hook(&fake.socket_path, &[]);

    fake.wait_for_title("herdr:work");
    assert!(status.success());
}

#[test]
fn socket_path_outranks_a_foreign_herdr_session_env() {
    // Regression: a monitor respawned from a shell inside another session's
    // pane inherits that session's HERDR_SESSION; the socket names the
    // server actually being monitored and must win.
    let fake = FakeHerdr::start_at("sessions/work/herdr.sock");

    run_hook(&fake.socket_path, &[("HERDR_SESSION", "personal")]);

    fake.wait_for_title("herdr:work");
}

#[test]
fn hook_defaults_session_name_when_nothing_resolves() {
    let fake = FakeHerdr::start();

    let status = run_hook(&fake.socket_path, &[]);

    fake.wait_for_title("herdr:default");
    assert!(status.success());
}

#[test]
fn ssh_environment_adds_the_host_to_the_default_title() {
    let fake = FakeHerdr::start();

    let status = run_hook(
        &fake.socket_path,
        &[
            ("HERDR_SESSION", "personal"),
            ("SSH_CONNECTION", "192.168.0.15 55767 192.168.0.11 22"),
        ],
    );

    let host = std::process::Command::new("hostname")
        .arg("-s")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|name| name.trim().to_string())
        .expect("test host has a hostname");
    fake.wait_for_title(&format!("herdr:personal ({host})"));
    assert!(status.success());
}

#[test]
fn hook_fetches_the_snapshot_before_setting_the_title() {
    let fake = FakeHerdr::start();

    run_hook(&fake.socket_path, &[("HERDR_SESSION", "personal")]);

    fake.wait_for_title("herdr:personal");
    let methods: Vec<String> = fake
        .requests()
        .iter()
        .filter_map(|request| request["method"].as_str().map(str::to_string))
        .collect();
    let snapshot_at = methods
        .iter()
        .position(|method| method == "session.snapshot")
        .expect("hook requested session.snapshot");
    let set_at = methods
        .iter()
        .position(|method| method == "client.window_title.set")
        .expect("hook set the title");
    assert!(
        snapshot_at < set_at,
        "snapshot must be fetched before the title is rendered; saw {methods:?}"
    );
}

#[test]
fn hook_still_sets_a_title_when_the_snapshot_request_fails() {
    let fake = FakeHerdr::start();
    fake.fail_snapshot_requests(true);

    let status = run_hook(&fake.socket_path, &[("HERDR_SESSION", "personal")]);

    fake.wait_for_title("herdr:personal");
    assert!(status.success(), "snapshot failure must not break the title");
}
