//! Read-side view over herdr's `session.snapshot` payload.

use crate::template::TokenValues;

/// The focused workspace/tab/pane ids. The live 0.7.4+ snapshot carries
/// these at the top level (verified against a real 0.7.5 payload).
pub struct Focus {
    pub workspace_id: String,
    pub tab_id: String,
    pub pane_id: String,
}

pub fn focus(snapshot: &serde_json::Value) -> Focus {
    let id = |key: &str| snapshot[key].as_str().unwrap_or("").to_string();
    Focus {
        workspace_id: id("focused_workspace_id"),
        tab_id: id("focused_tab_id"),
        pane_id: id("focused_pane_id"),
    }
}

/// One entry of the snapshot's `agents` list — the only pane data the
/// indicator needs. All snapshot field knowledge stays in this module.
pub struct AgentEntry {
    pub pane_id: String,
    pub workspace_id: String,
    pub status: String,
}

pub fn agents(snapshot: &serde_json::Value) -> Vec<AgentEntry> {
    let Some(entries) = snapshot["agents"].as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .map(|entry| AgentEntry {
            pane_id: entry["pane_id"].as_str().unwrap_or("").to_string(),
            workspace_id: entry["workspace_id"].as_str().unwrap_or("").to_string(),
            status: entry["agent_status"].as_str().unwrap_or("unknown").to_string(),
        })
        .collect()
}

/// Build template token values from a snapshot. Anything that cannot be
/// resolved renders as the empty string so optional sections can collapse.
pub fn token_values(snapshot: &serde_json::Value, session: &str, host: &str) -> TokenValues {
    let focus = focus(snapshot);

    let workspace = find_by(snapshot, "workspaces", "workspace_id", &focus.workspace_id)
        .and_then(|workspace| workspace["label"].as_str())
        .unwrap_or("")
        .to_string();

    let tab = tab_label(snapshot, &focus.workspace_id, &focus.tab_id);

    let focused_agent = find_by(snapshot, "agents", "pane_id", &focus.pane_id);
    let agent = focused_agent
        .and_then(|entry| entry["agent"].as_str())
        .unwrap_or("")
        .to_string();
    // Field-level fallback: an agent entry that carries no stripped title
    // must not shadow the pane entry that does.
    let title = [
        focused_agent,
        find_by(snapshot, "panes", "pane_id", &focus.pane_id),
    ]
    .into_iter()
    .flatten()
    .find_map(|entry| {
        entry["terminal_title_stripped"]
            .as_str()
            .filter(|title| !title.is_empty())
    })
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
