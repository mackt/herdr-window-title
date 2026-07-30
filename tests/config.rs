//! Config behaviour observed at the fake herdr socket: custom templates,
//! error fallbacks, and reload between renders.

mod common;

use common::{run_hook, run_hook_capture, FakeHerdr};
use herdr_window_title::config::{Config, SpinnerScope};
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

#[test]
fn broken_template_falls_back_to_default_with_a_warning() {
    let fake = FakeHerdr::start();
    let config_dir = tempfile::tempdir().expect("config dir");
    write_config(
        config_dir.path(),
        r#"template = "herdr:{session}[ · {workspace}""#,
    );

    let output = run_hook_capture(
        &fake.socket_path,
        &[
            ("HERDR_SESSION", "personal"),
            (
                "HERDR_PLUGIN_CONFIG_DIR",
                config_dir.path().to_str().expect("utf8 path"),
            ),
        ],
    );

    fake.wait_for_title("herdr:personal");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("template"),
        "warning names the template problem; stderr was: {stderr}"
    );
}

#[test]
fn invalid_toml_keeps_defaults_and_still_sets_a_title() {
    let fake = FakeHerdr::start();
    let config_dir = tempfile::tempdir().expect("config dir");
    write_config(config_dir.path(), "template = not even toml [");

    let output = run_hook_capture(
        &fake.socket_path,
        &[
            ("HERDR_SESSION", "personal"),
            (
                "HERDR_PLUGIN_CONFIG_DIR",
                config_dir.path().to_str().expect("utf8 path"),
            ),
        ],
    );

    fake.wait_for_title("herdr:personal");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("config"),
        "warning names the config problem; stderr was: {stderr}"
    );
}

#[test]
fn config_edits_apply_on_the_next_render_without_restart() {
    let fake = FakeHerdr::start();
    fake.set_snapshot(workspace_snapshot());
    let config_dir = tempfile::tempdir().expect("config dir");
    let env = [
        ("HERDR_SESSION", "personal"),
        (
            "HERDR_PLUGIN_CONFIG_DIR",
            config_dir.path().to_str().expect("utf8 path"),
        ),
    ];

    write_config(config_dir.path(), r#"template = "one:{session}""#);
    run_hook(&fake.socket_path, &env);
    fake.wait_for_title("one:personal");

    write_config(config_dir.path(), r#"template = "two:{session}""#);
    run_hook(&fake.socket_path, &env);
    fake.wait_for_title("two:personal");
}

// Sanctioned exception to the two-seam rule: ticket #4's acceptance
// criterion explicitly requires "defaults for every key are asserted in
// tests and match the spec table", and interval defaults have no cheap
// observable at either seam. Everything else config-related asserts
// through titles (this file) or render_title (tests/indicator.rs).
#[test]
fn config_table_defaults_and_field_fallback_match_the_spec() {
    let config = Config::default();
    assert_eq!(config.template, "{indicator}herdr:{session}");
    assert_eq!(config.working_template, None);
    assert_eq!(config.blocked_template, None);
    assert_eq!(config.done_template, None);
    assert_eq!(config.spinner_scope, SpinnerScope::Pane);
    assert_eq!(config.spinner_interval_ms, 200);
    assert_eq!(config.idle_keepalive_ms, 2000);
    assert_eq!(config.blocked_glyph, "●");
    assert_eq!(config.done_glyph, "✓");

    let (config, warnings) = Config::parse("spinner_interval_ms = -5\nblocked_glyph = 7\n");
    assert_eq!(config.spinner_interval_ms, 200, "invalid interval falls back");
    assert_eq!(config.blocked_glyph, "●", "invalid glyph falls back");
    assert_eq!(warnings.len(), 2, "one warning per invalid field: {warnings:?}");
}

#[test]
fn unknown_token_warns_in_the_plugin_log_and_renders_literally() {
    let fake = FakeHerdr::start();
    let config_dir = tempfile::tempdir().expect("config dir");
    write_config(config_dir.path(), r#"template = "herdr:{sesion}""#);

    let output = run_hook_capture(
        &fake.socket_path,
        &[
            ("HERDR_SESSION", "personal"),
            (
                "HERDR_PLUGIN_CONFIG_DIR",
                config_dir.path().to_str().expect("utf8 path"),
            ),
        ],
    );

    fake.wait_for_title("herdr:{sesion}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown token {sesion}"),
        "hook warns about the typo; stderr was: {stderr}"
    );
}
