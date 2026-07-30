//! Primary seam: hook-mode behaviour observed at the fake herdr socket.

mod common;

use common::{run_hook, write_fake_herdr_bin, FakeHerdr};

#[test]
fn hook_sets_title_from_herdr_session_env() {
    let fake = FakeHerdr::start();

    let status = run_hook(&fake.socket_path, &[("HERDR_SESSION", "personal")]);

    fake.wait_for_title("herdr:personal");
    assert!(status.success(), "hook exits zero after the title is set");
}

#[test]
fn hook_falls_back_to_status_json_when_env_is_absent() {
    let fake = FakeHerdr::start();
    let fake_bin = write_fake_herdr_bin(
        fake.dir.path(),
        r#"{"client":{"session":"work"},"server":{"session":"work"}}"#,
    );

    let status = run_hook(
        &fake.socket_path,
        &[("HERDR_BIN_PATH", fake_bin.to_str().expect("utf8 path"))],
    );

    fake.wait_for_title("herdr:work");
    assert!(status.success());
}

#[test]
fn hook_defaults_session_name_when_nothing_resolves() {
    let fake = FakeHerdr::start();
    let missing_bin = fake.dir.path().join("no-such-herdr");

    let status = run_hook(
        &fake.socket_path,
        &[("HERDR_BIN_PATH", missing_bin.to_str().expect("utf8 path"))],
    );

    fake.wait_for_title("herdr:default");
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
