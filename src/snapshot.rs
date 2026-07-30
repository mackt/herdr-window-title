//! Read-side view over herdr's `session.snapshot` payload.

use crate::template::TokenValues;

/// Build template token values from a snapshot. Anything that cannot be
/// resolved renders as the empty string so optional sections can collapse.
pub fn token_values(snapshot: &serde_json::Value, session: &str, host: &str) -> TokenValues {
    let focused_workspace = snapshot["focused_workspace_id"].as_str().unwrap_or("");
    let focused_tab = snapshot["focused_tab_id"].as_str().unwrap_or("");
    let focused_pane = snapshot["focused_pane_id"].as_str().unwrap_or("");

    let workspace = find_by(snapshot, "workspaces", "workspace_id", focused_workspace)
        .and_then(|workspace| workspace["label"].as_str())
        .unwrap_or("")
        .to_string();

    let tab = tab_label(snapshot, focused_workspace, focused_tab);

    let focused_agent = find_by(snapshot, "agents", "pane_id", focused_pane);
    let agent = focused_agent
        .and_then(|entry| entry["agent"].as_str())
        .unwrap_or("")
        .to_string();
    let title = focused_agent
        .or_else(|| find_by(snapshot, "panes", "pane_id", focused_pane))
        .and_then(|entry| entry["terminal_title_stripped"].as_str())
        .unwrap_or("")
        .to_string();

    TokenValues {
        indicator: String::new(),
        session: session.to_string(),
        workspace,
        tab,
        agent,
        title,
        host: host.to_string(),
    }
}

/// A tab's display label: its own label when non-numeric, otherwise its
/// 1-based switch order among the focused workspace's tabs (prefix+N).
fn tab_label(snapshot: &serde_json::Value, workspace_id: &str, tab_id: &str) -> String {
    let Some(tabs) = snapshot["tabs"].as_array() else {
        return String::new();
    };
    let workspace_tabs: Vec<&serde_json::Value> = tabs
        .iter()
        .filter(|tab| tab["workspace_id"].as_str() == Some(workspace_id))
        .collect();
    let Some(order) = workspace_tabs
        .iter()
        .position(|tab| tab["tab_id"].as_str() == Some(tab_id))
    else {
        return String::new();
    };
    let label = workspace_tabs[order]["label"].as_str().unwrap_or("");
    if label.is_empty() || label.chars().all(|ch| ch.is_ascii_digit()) {
        (order + 1).to_string()
    } else {
        label.to_string()
    }
}

fn find_by<'a>(
    snapshot: &'a serde_json::Value,
    list: &str,
    key: &str,
    wanted: &str,
) -> Option<&'a serde_json::Value> {
    if wanted.is_empty() {
        return None;
    }
    snapshot[list]
        .as_array()?
        .iter()
        .find(|entry| entry[key].as_str() == Some(wanted))
}
