//! The secondary test seam promised by the PRD: one pure function from
//! (snapshot, config, spinner frame) to the title string.

use crate::config::{Config, HostDisplay, DEFAULT_TEMPLATE};
use crate::indicator::{activity, indicator, select_template};
use crate::snapshot::token_values;
use crate::template::parse_with_fallback;

/// `host` is always the server's short hostname; `remote` is whether the
/// server was reached over SSH. `host_display` decides here which of the
/// two `{host}` actually reflects.
pub fn render_title(
    snapshot: &serde_json::Value,
    config: &Config,
    session: &str,
    host: &str,
    remote: bool,
    frame: &str,
) -> String {
    let host = match config.host_display {
        HostDisplay::Always => host,
        HostDisplay::Never => "",
        HostDisplay::Auto if remote => host,
        HostDisplay::Auto => "",
    };
    let activity = activity(snapshot, config.spinner_scope);
    let mut values = token_values(snapshot, session, host);
    values.indicator = indicator(&activity, config, frame);
    let (template, _) = parse_with_fallback(select_template(&activity, config), DEFAULT_TEMPLATE);
    template.render(&values)
}
