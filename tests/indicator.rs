//! Pure seam: session activity aggregation and indicator rendering.

use herdr_window_title::config::{Config, SpinnerScope};
use herdr_window_title::indicator::{activity, indicator, SessionActivity};

/// Two workspaces; focus on w1:p1. Statuses across the whole session:
/// one blocked (other workspace), one done, one unknown (ignored),
/// focused pane working, plus one background working pane.
fn snapshot() -> serde_json::Value {
    serde_json::json!({
        "focused_workspace_id": "w1",
        "focused_tab_id": "w1:t1",
        "focused_pane_id": "w1:p1",
        "workspaces": [
            {"workspace_id": "w1", "label": "one", "focused": true},
            {"workspace_id": "w2", "label": "two", "focused": false},
        ],
        "tabs": [],
        "agents": [
            {"pane_id": "w1:p1", "workspace_id": "w1", "agent": "claude", "agent_status": "working", "focused": true},
            {"pane_id": "w1:p2", "workspace_id": "w1", "agent": "codex", "agent_status": "working", "focused": false},
            {"pane_id": "w2:p1", "workspace_id": "w2", "agent": "pi", "agent_status": "blocked", "focused": false},
            {"pane_id": "w2:p2", "workspace_id": "w2", "agent": "grok", "agent_status": "done", "focused": false},
            {"pane_id": "w2:p3", "workspace_id": "w2", "agent": "droid", "agent_status": "unknown", "focused": false},
        ],
        "panes": [],
    })
}

fn active(blocked: usize, done: usize, focused: bool, background: usize) -> SessionActivity {
    SessionActivity {
        blocked,
        done,
        focused_working: focused,
        background_working: background,
    }
}

#[test]
fn indicator_renders_one_segment_by_priority() {
    let config = Config::default();
    let frame = "⠋";
    // blocked beats everything; done beats working; spinner beats count.
    assert_eq!(indicator(&active(2, 1, true, 3), &config, frame), "●2 ");
    assert_eq!(indicator(&active(0, 1, true, 3), &config, frame), "✓1 ");
    assert_eq!(indicator(&active(0, 0, true, 3), &config, frame), "⠋ ");
    assert_eq!(indicator(&active(0, 0, false, 3), &config, frame), "③ ");
    assert_eq!(indicator(&active(0, 0, false, 0), &config, frame), "");
}

#[test]
fn counts_cap_for_display() {
    let config = Config::default();
    assert_eq!(indicator(&active(12, 0, false, 0), &config, "⠋"), "●9+ ");
    assert_eq!(indicator(&active(0, 10, false, 0), &config, "⠋"), "✓9+ ");
    assert_eq!(indicator(&active(0, 0, false, 20), &config, "⠋"), "⑳ ");
    assert_eq!(indicator(&active(0, 0, false, 21), &config, "⠋"), "⊕ ");
}

#[test]
fn glyphs_come_from_config() {
    let (config, warnings) = Config::parse(
        r#"
        blocked_glyph = "!"
        done_glyph = "ok"
        "#,
    );
    assert!(warnings.is_empty());
    assert_eq!(indicator(&active(1, 0, false, 0), &config, "⠋"), "!1 ");
    assert_eq!(indicator(&active(0, 2, false, 0), &config, "⠋"), "ok2 ");
}

#[test]
fn aggregates_all_five_statuses_across_the_session() {
    let activity = activity(&snapshot(), SpinnerScope::Pane);
    assert_eq!(
        activity,
        SessionActivity {
            blocked: 1,
            done: 1,
            focused_working: true,
            background_working: 1,
        }
    );
}

#[test]
fn workspace_scope_counts_focused_workspace_panes_as_focused_working() {
    let mut snap = snapshot();
    // Focused pane itself idle; another pane in the focused workspace works.
    snap["agents"][0]["agent_status"] = serde_json::json!("idle");
    let pane_scope = activity(&snap, SpinnerScope::Pane);
    assert!(!pane_scope.focused_working);
    assert_eq!(pane_scope.background_working, 1);

    let workspace_scope = activity(&snap, SpinnerScope::Workspace);
    assert!(workspace_scope.focused_working, "same-workspace work counts");
    assert_eq!(workspace_scope.background_working, 0);
}
