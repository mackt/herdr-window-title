//! The secondary test seam promised by the PRD: one pure function from
//! (snapshot, config, spinner frame) to the title string.

use crate::config::{Config, DEFAULT_TEMPLATE};
use crate::indicator::{activity, indicator, select_template};
use crate::snapshot::token_values;
use crate::template::parse_with_fallback;

pub fn render_title(
    snapshot: &serde_json::Value,
    config: &Config,
    session: &str,
    host: &str,
    frame: &str,
) -> String {
    let activity = activity(snapshot, config.spinner_scope);
    let mut values = token_values(snapshot, session, host);
    values.indicator = indicator(&activity, config, frame);
    let (template, _) = parse_with_fallback(select_template(&activity, config), DEFAULT_TEMPLATE);
    template.render(&values)
}
