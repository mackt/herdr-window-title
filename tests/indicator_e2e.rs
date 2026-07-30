//! Indicator behaviour end to end at the fake herdr socket.

mod common;

use common::{run_hook, FakeHerdr};

fn blocked_snapshot() -> serde_json::Value {
    serde_json::json!({
        "focused_workspace_id": "w1",
        "focused_tab_id": "w1:t1",
        "focused_pane_id": "w1:p1",
        "workspaces": [{"workspace_id": "w1", "label": "one", "focused": true}],
        "tabs": [],
        "agents": [
            {"pane_id": "w2:p1", "workspace_id": "w2", "agent": "pi", "agent_status": "blocked", "focused": false},
        ],
        "panes": [],
    })
}

#[test]
fn blocked_agent_shows_in_the_default_title() {
    let fake = FakeHerdr::start();
    fake.set_snapshot(blocked_snapshot());

    run_hook(&fake.socket_path, &[("HERDR_SESSION", "personal")]);

    fake.wait_for_title("●1 herdr:personal");
}

#[test]
fn blocked_template_reshapes_the_whole_title() {
    let fake = FakeHerdr::start();
    fake.set_snapshot(blocked_snapshot());
    let config_dir = tempfile::tempdir().expect("config dir");
    std::fs::write(
        config_dir.path().join("config.toml"),
        r#"blocked_template = "attention: {session}""#,
    )
    .expect("write config");

    run_hook(
        &fake.socket_path,
        &[
            ("HERDR_SESSION", "personal"),
            (
                "HERDR_PLUGIN_CONFIG_DIR",
                config_dir.path().to_str().expect("utf8 path"),
            ),
        ],
    );

    fake.wait_for_title("attention: personal");
}
