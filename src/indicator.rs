//! Agent-state aggregation across the session and the `{indicator}` token.

use crate::config::SpinnerScope;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SessionActivity {
    pub blocked: usize,
    pub done: usize,
    pub focused_working: bool,
    pub background_working: usize,
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
