//! Secondary seam: the pure `render_title` function. Indicator behaviour is
//! asserted only through the title strings it produces.

mod common;

use common::snapshot_with_agents;
use herdr_window_title::config::{Config, SpinnerScope};
use herdr_window_title::render::render_title;

fn render(snapshot: &serde_json::Value, config: &Config) -> String {
    render_title(snapshot, config, "personal", "mbp", "⠋")
}

#[test]
fn indicator_shows_one_segment_by_priority() {
    let config = Config::default();
    // Full house: focused working, one background working, two blocked,
    // one done, one unknown (which must never matter).
    let full = snapshot_with_agents(&[
        ("w1:p1", "w1", "working"),
        ("w1:p2", "w1", "working"),
        ("w2:p1", "w2", "blocked"),
        ("w2:p2", "w2", "blocked"),
        ("w2:p3", "w2", "done"),
        ("w2:p4", "w2", "unknown"),
    ]);
    assert_eq!(render(&full, &config), "●2 herdr:personal");

    let no_blocked = snapshot_with_agents(&[
        ("w1:p1", "w1", "working"),
        ("w2:p3", "w2", "done"),
        ("w2:p4", "w2", "unknown"),
    ]);
    assert_eq!(render(&no_blocked, &config), "✓1 herdr:personal");

    let focused_working = snapshot_with_agents(&[
        ("w1:p1", "w1", "working"),
        ("w1:p2", "w1", "working"),
        ("w2:p4", "w2", "unknown"),
    ]);
    assert_eq!(render(&focused_working, &config), "⠋ herdr:personal");

    let background_only = snapshot_with_agents(&[
        ("w1:p2", "w1", "working"),
        ("w2:p4", "w2", "unknown"),
    ]);
    assert_eq!(render(&background_only, &config), "① herdr:personal");

    let idle = snapshot_with_agents(&[("w2:p4", "w2", "unknown")]);
    assert_eq!(render(&idle, &config), "herdr:personal");
}

#[test]
fn counts_cap_for_display() {
    let config = Config::default();
    let blocked: Vec<(String, String, &str)> = (0..12)
        .map(|i| (format!("w2:p{i}"), "w2".to_string(), "blocked"))
        .collect();
    let blocked: Vec<(&str, &str, &str)> = blocked
        .iter()
        .map(|(p, w, s)| (p.as_str(), w.as_str(), *s))
        .collect();
    assert_eq!(
        render(&snapshot_with_agents(&blocked), &config),
        "●9+ herdr:personal"
    );

    let background: Vec<(String, String, &str)> = (0..21)
        .map(|i| (format!("w2:p{i}"), "w2".to_string(), "working"))
        .collect();
    let background: Vec<(&str, &str, &str)> = background
        .iter()
        .map(|(p, w, s)| (p.as_str(), w.as_str(), *s))
        .collect();
    assert_eq!(
        render(&snapshot_with_agents(&background), &config),
        "⊕ herdr:personal"
    );
}

#[test]
fn spinner_scope_moves_the_focused_boundary() {
    // The focused pane is idle; a sibling pane in the same workspace works.
    let snapshot = snapshot_with_agents(&[
        ("w1:p1", "w1", "idle"),
        ("w1:p2", "w1", "working"),
    ]);

    let pane_scope = Config::default();
    assert_eq!(render(&snapshot, &pane_scope), "① herdr:personal");

    let workspace_scope = Config {
        spinner_scope: SpinnerScope::Workspace,
        ..Config::default()
    };
    assert_eq!(render(&snapshot, &workspace_scope), "⠋ herdr:personal");
}

#[test]
fn glyphs_come_from_config() {
    let config = Config {
        blocked_glyph: "!".into(),
        done_glyph: "ok".into(),
        ..Config::default()
    };
    let blocked = snapshot_with_agents(&[("w2:p1", "w2", "blocked")]);
    assert_eq!(render(&blocked, &config), "!1 herdr:personal");
    let done = snapshot_with_agents(&[("w2:p1", "w2", "done")]);
    assert_eq!(render(&done, &config), "ok1 herdr:personal");
}

#[test]
fn per_state_templates_reshape_the_title_and_fall_back_when_unset() {
    let config = Config {
        blocked_template: Some("attention: {session}".into()),
        ..Config::default()
    };
    let blocked = snapshot_with_agents(&[("w2:p1", "w2", "blocked")]);
    assert_eq!(render(&blocked, &config), "attention: personal");

    // done has no override configured → falls back to the base template.
    let done = snapshot_with_agents(&[("w2:p1", "w2", "done")]);
    assert_eq!(render(&done, &config), "✓1 herdr:personal");
}
