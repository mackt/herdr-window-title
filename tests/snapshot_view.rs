//! Secondary seam: token extraction asserted through `render_title` output.
//! The fixture mirrors the real 0.7.5 snapshot shape.

use herdr_window_title::config::{Config, HostDisplay};
use herdr_window_title::render::render_title;

fn snapshot() -> serde_json::Value {
    serde_json::json!({
        "focused_workspace_id": "w2",
        "focused_tab_id": "w2:t2",
        "focused_pane_id": "w2:p3",
        "workspaces": [
            {"workspace_id": "w1", "label": "github", "number": 1, "focused": false},
            {"workspace_id": "w2", "label": "dotfiles", "number": 2, "focused": true},
        ],
        "tabs": [
            {"tab_id": "w2:t1", "workspace_id": "w2", "label": "1", "number": 1, "focused": false},
            {"tab_id": "w2:t2", "workspace_id": "w2", "label": "logs", "number": 2, "focused": true},
            {"tab_id": "w1:t1", "workspace_id": "w1", "label": "1", "number": 1, "focused": false},
        ],
        "agents": [
            {"pane_id": "w2:p3", "workspace_id": "w2", "tab_id": "w2:t2", "agent": "claude",
             "agent_status": "idle", "focused": true,
             "terminal_title": "✳ fix titles", "terminal_title_stripped": "fix titles"},
        ],
        "panes": [
            {"pane_id": "w2:p3", "workspace_id": "w2", "tab_id": "w2:t2", "agent_status": "idle",
             "focused": true, "terminal_title_stripped": "fix titles"},
        ],
    })
}

fn token_config() -> Config {
    Config {
        template: "{workspace}/{tab} {agent}:{title}@{host}".into(),
        // Token extraction is under test, not host gating — pin it open.
        host_display: HostDisplay::Always,
        ..Config::default()
    }
}

#[test]
fn every_token_renders_its_focused_context_value() {
    let title = render_title(&snapshot(), &token_config(), "personal", "mbp", false, "⠋");
    assert_eq!(title, "dotfiles/logs claude:fix titles@mbp");
}

#[test]
fn numeric_tab_label_falls_back_to_switch_order() {
    let mut snap = snapshot();
    snap["tabs"][1]["label"] = serde_json::json!("2");
    let title = render_title(&snap, &token_config(), "personal", "mbp", false, "⠋");
    assert_eq!(
        title, "dotfiles/2 claude:fix titles@mbp",
        "second tab of the focused workspace by switch order"
    );
}

#[test]
fn missing_focus_collapses_optional_sections() {
    let config = Config {
        template: "herdr:{session}[ · {workspace}][ {agent}:{title}]".into(),
        ..Config::default()
    };
    let title = render_title(&serde_json::json!({}), &config, "personal", "mbp", false, "⠋");
    assert_eq!(title, "herdr:personal");
}

#[test]
fn host_renders_only_on_remote_servers_by_default() {
    let config = Config::default();
    let local = render_title(&snapshot(), &config, "personal", "mbp", false, "⠋");
    assert_eq!(local, "herdr:personal", "local server: host section collapses");
    let remote = render_title(&snapshot(), &config, "personal", "devbox", true, "⠋");
    assert_eq!(remote, "herdr:personal (devbox)");
}

#[test]
fn host_display_always_and_never_override_ssh_detection() {
    let always = Config {
        host_display: HostDisplay::Always,
        ..Config::default()
    };
    let title = render_title(&snapshot(), &always, "personal", "mbp", false, "⠋");
    assert_eq!(title, "herdr:personal (mbp)");

    let never = Config {
        host_display: HostDisplay::Never,
        ..Config::default()
    };
    let title = render_title(&snapshot(), &never, "personal", "devbox", true, "⠋");
    assert_eq!(title, "herdr:personal");
}

#[test]
fn title_falls_back_to_the_pane_entry_when_the_agent_entry_lacks_one() {
    let mut snap = snapshot();
    snap["agents"][0]
        .as_object_mut()
        .expect("agent entry")
        .remove("terminal_title_stripped");
    let title = render_title(&snap, &token_config(), "personal", "mbp", false, "⠋");
    assert_eq!(
        title, "dotfiles/logs claude:fix titles@mbp",
        "field-level fallback: the pane entry still carries the title"
    );
}
