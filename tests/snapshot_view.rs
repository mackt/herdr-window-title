//! Pure seam: extracting TokenValues from a `session.snapshot` payload.
//! The fixture mirrors the real 0.7.5 snapshot shape.

use herdr_window_title::snapshot::token_values;

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
             "agent_status": "working", "focused": true,
             "terminal_title": "✳ fix titles", "terminal_title_stripped": "fix titles"},
        ],
        "panes": [
            {"pane_id": "w2:p3", "workspace_id": "w2", "tab_id": "w2:t2", "agent_status": "working",
             "focused": true, "terminal_title_stripped": "fix titles"},
        ],
    })
}

#[test]
fn extracts_focused_context_from_snapshot() {
    let values = token_values(&snapshot(), "personal", "mbp");
    assert_eq!(values.session, "personal");
    assert_eq!(values.host, "mbp");
    assert_eq!(values.workspace, "dotfiles");
    assert_eq!(values.tab, "logs");
    assert_eq!(values.agent, "claude");
    assert_eq!(values.title, "fix titles");
}

#[test]
fn numeric_tab_label_falls_back_to_switch_order() {
    let mut snap = snapshot();
    snap["tabs"][1]["label"] = serde_json::json!("2");
    let values = token_values(&snap, "personal", "mbp");
    assert_eq!(values.tab, "2", "second tab of the focused workspace by switch order");
}

#[test]
fn missing_focus_yields_empty_optional_tokens() {
    let values = token_values(&serde_json::json!({}), "personal", "mbp");
    assert_eq!(values.session, "personal");
    assert_eq!(values.workspace, "");
    assert_eq!(values.tab, "");
    assert_eq!(values.agent, "");
    assert_eq!(values.title, "");
}
