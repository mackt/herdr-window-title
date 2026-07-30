//! Agent-state aggregation across the session and the `{indicator}` token.

use crate::config::{Config, SpinnerScope};

/// Braille spinner frames, advanced by the monitor while focused work runs.
pub const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SessionActivity {
    pub blocked: usize,
    pub done: usize,
    pub focused_working: bool,
    pub background_working: usize,
}

/// The `{indicator}` token: exactly one segment — the highest non-empty
/// priority — with a trailing space so the default template collapses
/// cleanly when idle. Priority: blocked > done > focused spinner >
/// background count > empty.
pub fn indicator(activity: &SessionActivity, config: &Config, frame: &str) -> String {
    if activity.blocked > 0 {
        return format!("{}{} ", config.blocked_glyph, capped(activity.blocked));
    }
    if activity.done > 0 {
        return format!("{}{} ", config.done_glyph, capped(activity.done));
    }
    if activity.focused_working {
        return format!("{frame} ");
    }
    if activity.background_working > 0 {
        return format!("{} ", circled(activity.background_working));
    }
    String::new()
}

fn capped(count: usize) -> String {
    if count > 9 {
        "9+".into()
    } else {
        count.to_string()
    }
}

/// Background-work count as a circled digit: ①–⑳, then ⊕ beyond.
fn circled(count: usize) -> String {
    match count {
        1..=20 => char::from_u32(0x2460 + (count as u32 - 1))
            .expect("U+2460..U+2473 are valid")
            .to_string(),
        _ => "⊕".into(),
    }
}

/// Count agent states across every workspace in the snapshot. `unknown`
/// never participates. "Focused working" follows the spinner scope: the
/// focused pane only, or any pane in the focused workspace.
pub fn activity(snapshot: &serde_json::Value, scope: SpinnerScope) -> SessionActivity {
    let focused_pane = snapshot["focused_pane_id"].as_str().unwrap_or("");
    let focused_workspace = snapshot["focused_workspace_id"].as_str().unwrap_or("");
    let mut activity = SessionActivity::default();

    let Some(agents) = snapshot["agents"].as_array() else {
        return activity;
    };
    for agent in agents {
        let status = agent["agent_status"].as_str().unwrap_or("unknown");
        match status {
            "blocked" => activity.blocked += 1,
            "done" => activity.done += 1,
            "working" => {
                let in_scope = match scope {
                    SpinnerScope::Pane => agent["pane_id"].as_str() == Some(focused_pane),
                    SpinnerScope::Workspace => {
                        agent["workspace_id"].as_str() == Some(focused_workspace)
                    }
                };
                if in_scope {
                    activity.focused_working = true;
                } else {
                    activity.background_working += 1;
                }
            }
            _ => {}
        }
    }
    activity
}
