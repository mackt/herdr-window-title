//! Config behaviour observed at the fake herdr socket: custom templates,
//! error fallbacks, and reload between renders.

mod common;

use common::{run_hook, FakeHerdr};
use std::path::Path;

fn write_config(dir: &Path, contents: &str) {
    std::fs::write(dir.join("config.toml"), contents).expect("write config.toml");
}

fn workspace_snapshot() -> serde_json::Value {
    serde_json::json!({
        "focused_workspace_id": "w1",
        "workspaces": [{"workspace_id": "w1", "label": "dotfiles", "focused": true}],
        "tabs": [], "agents": [], "panes": [],
    })
}

#[test]
fn custom_template_from_config_drives_the_title() {
    let fake = FakeHerdr::start();
    fake.set_snapshot(workspace_snapshot());
    let config_dir = tempfile::tempdir().expect("config dir");
    write_config(config_dir.path(), r#"template = "{session}@{workspace}""#);

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

    fake.wait_for_title("personal@dotfiles");
}
