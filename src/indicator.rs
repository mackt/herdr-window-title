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

/// Template selection follows the aggregate state: blocked, then done,
/// then working (focused or background), each falling back to the base
/// template when its override is unset.
pub fn select_template<'config>(activity: &SessionActivity, config: &'config Config) -> &'config str {
    let selected = if activity.blocked > 0 {
        config.blocked_template.as_ref()
    } else if activity.done > 0 {
        config.done_template.as_ref()
    } else if activity.focused_working || activity.background_working > 0 {
        config.working_template.as_ref()
    } else {
        None
    };
    selected.unwrap_or(&config.template)
}

/// Count agent states across every workspace in the snapshot. `unknown`
/// never participates. "Focused working" follows the spinner scope: the
/// focused pane only, or any pane in the focused workspace.
pub fn activity(snapshot: &serde_json::Value, scope: SpinnerScope) -> SessionActivity {
    let focus = crate::snapshot::focus(snapshot);
    let mut activity = SessionActivity::default();

    for agent in crate::snapshot::agents(snapshot) {
        match agent.status.as_str() {
            "blocked" => activity.blocked += 1,
            "done" => activity.done += 1,
            "working" => {
                let in_scope = match scope {
                    SpinnerScope::Pane => agent.pane_id == focus.pane_id,
                    SpinnerScope::Workspace => agent.workspace_id == focus.workspace_id,
                    SpinnerScope::Session => true,
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
