//! Plugin configuration: `config.toml` in the herdr-managed config dir,
//! re-read before every render so edits apply without a server restart.
//! Every key is optional; anything invalid falls back to its default with
//! a warning instead of breaking the title.

pub const DEFAULT_TEMPLATE: &str = "{indicator}herdr:{session}[ ({host})]";

/// When `{host}` resolves to the server's hostname versus the empty string
/// (empty is what lets a `[ ({host})]` section collapse on local servers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostDisplay {
    /// Hostname only when the server was reached over SSH — remote sessions
    /// name themselves, local ones stay clean.
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinnerScope {
    Pane,
    Workspace,
    /// Any working agent anywhere in the session spins; nothing is
    /// "background", so the working count never appears.
    Session,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub template: String,
    pub working_template: Option<String>,
    pub blocked_template: Option<String>,
    pub done_template: Option<String>,
    pub host_display: HostDisplay,
    pub spinner_scope: SpinnerScope,
    pub spinner_interval_ms: u64,
    pub idle_keepalive_ms: u64,
    pub blocked_glyph: String,
    pub done_glyph: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            template: DEFAULT_TEMPLATE.into(),
            working_template: None,
            blocked_template: None,
            done_template: None,
            host_display: HostDisplay::Auto,
            spinner_scope: SpinnerScope::Session,
            spinner_interval_ms: 200,
            idle_keepalive_ms: 2000,
            blocked_glyph: "●".into(),
            done_glyph: "✓".into(),
        }
    }
}

impl Config {
    /// Load from `<config_dir>/config.toml`. A missing file is the default
    /// config; unreadable or invalid content degrades per-field and reports
    /// human-readable warnings for the plugin log.
    pub fn load(config_dir: Option<&std::path::Path>) -> (Self, Vec<String>) {
        let Some(dir) = config_dir else {
            return (Self::default(), Vec::new());
        };
        let path = dir.join("config.toml");
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), Vec::new());
            }
            Err(error) => {
                return (
                    Self::default(),
                    vec![format!("config: cannot read {}: {error}", path.display())],
                );
            }
        };
        Self::parse(&contents)
    }

    pub fn parse(contents: &str) -> (Self, Vec<String>) {
        let mut config = Self::default();
        let mut warnings = Vec::new();
        let table: toml::Table = match contents.parse() {
            Ok(table) => table,
            Err(error) => {
                warnings.push(format!("config: invalid TOML, using defaults: {error}"));
                return (config, warnings);
            }
        };

        let string = |key: &str, into: &mut String, warnings: &mut Vec<String>| {
            match table.get(key) {
                None => {}
                Some(toml::Value::String(value)) => *into = value.clone(),
                Some(other) => warnings.push(bad_type(key, "a string", other)),
            }
        };
        string("template", &mut config.template, &mut warnings);
        string("blocked_glyph", &mut config.blocked_glyph, &mut warnings);
        string("done_glyph", &mut config.done_glyph, &mut warnings);

        let optional = |key: &str, into: &mut Option<String>, warnings: &mut Vec<String>| {
            match table.get(key) {
                None => {}
                Some(toml::Value::String(value)) if value.is_empty() => {}
                Some(toml::Value::String(value)) => *into = Some(value.clone()),
                Some(other) => warnings.push(bad_type(key, "a string", other)),
            }
        };
        optional("working_template", &mut config.working_template, &mut warnings);
        optional("blocked_template", &mut config.blocked_template, &mut warnings);
        optional("done_template", &mut config.done_template, &mut warnings);

        let interval = |key: &str, into: &mut u64, warnings: &mut Vec<String>| {
            match table.get(key) {
                None => {}
                Some(toml::Value::Integer(value)) if *value > 0 => *into = *value as u64,
                Some(other) => warnings.push(bad_type(key, "a positive integer", other)),
            }
        };
        interval("spinner_interval_ms", &mut config.spinner_interval_ms, &mut warnings);
        interval("idle_keepalive_ms", &mut config.idle_keepalive_ms, &mut warnings);

        match table.get("host_display") {
            None => {}
            Some(toml::Value::String(value)) if value == "auto" => {
                config.host_display = HostDisplay::Auto;
            }
            Some(toml::Value::String(value)) if value == "always" => {
                config.host_display = HostDisplay::Always;
            }
            Some(toml::Value::String(value)) if value == "never" => {
                config.host_display = HostDisplay::Never;
            }
            Some(other) => warnings.push(bad_type(
                "host_display",
                "\"auto\", \"always\", or \"never\"",
                other,
            )),
        }

        match table.get("spinner_scope") {
            None => {}
            Some(toml::Value::String(value)) if value == "pane" => {
                config.spinner_scope = SpinnerScope::Pane;
            }
            Some(toml::Value::String(value)) if value == "workspace" => {
                config.spinner_scope = SpinnerScope::Workspace;
            }
            Some(toml::Value::String(value)) if value == "session" => {
                config.spinner_scope = SpinnerScope::Session;
            }
            Some(other) => warnings.push(bad_type(
                "spinner_scope",
                "\"pane\", \"workspace\", or \"session\"",
                other,
            )),
        }

        (config, warnings)
    }
}

fn bad_type(key: &str, expected: &str, got: &toml::Value) -> String {
    format!("config: `{key}` must be {expected}, got {got}; using default")
}
