//! Title template engine: `{token}` substitution.

#[derive(Debug, Default, Clone)]
pub struct TokenValues {
    pub indicator: String,
    pub session: String,
    pub workspace: String,
    pub tab: String,
    pub agent: String,
    pub title: String,
    pub host: String,
}

pub const KNOWN_TOKENS: [&str; 7] = [
    "indicator",
    "session",
    "workspace",
    "tab",
    "agent",
    "title",
    "host",
];

impl TokenValues {
    fn get(&self, name: &str) -> Option<&str> {
        match name {
            "indicator" => Some(&self.indicator),
            "session" => Some(&self.session),
            "workspace" => Some(&self.workspace),
            "tab" => Some(&self.tab),
            "agent" => Some(&self.agent),
            "title" => Some(&self.title),
            "host" => Some(&self.host),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
}

#[derive(Debug)]
enum Segment {
    Literal(String),
    Token(String),
    /// `[ ... ]` — vanishes entirely when every token inside renders empty.
    Section(Vec<Segment>),
}

#[derive(Debug)]
pub struct Template {
    segments: Vec<Segment>,
}

impl Template {
    pub fn parse(source: &str) -> Result<Self, ParseError> {
        let mut chars = source.chars().peekable();
        let segments = parse_segments(&mut chars, false)?;
        Ok(Self { segments })
    }

    pub fn render(&self, values: &TokenValues) -> String {
        render_segments(&self.segments, values).text
    }

    /// Token names in this template that no value exists for — rendered
    /// literally, and worth a warning in the plugin log.
    pub fn unknown_tokens(&self) -> Vec<&str> {
        fn walk<'template>(segments: &'template [Segment], out: &mut Vec<&'template str>) {
            for segment in segments {
                match segment {
                    Segment::Token(name) if !KNOWN_TOKENS.contains(&name.as_str()) => {
                        out.push(name);
                    }
                    Segment::Section(inner) => walk(inner, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.segments, &mut out);
        out
    }
}

/// Parse `source`, falling back to the built-in default template on syntax
/// errors. Warnings cover both fallback and unknown tokens; the caller
/// decides where they land (hooks print them into the plugin log).
pub fn parse_with_fallback(source: &str, default: &str) -> (Template, Vec<String>) {
    let mut warnings = Vec::new();
    let template = match Template::parse(source) {
        Ok(template) => template,
        Err(error) => {
            warnings.push(format!(
                "template: {}; using the default template",
                error.message
            ));
            Template::parse(default).expect("built-in template is valid")
        }
    };
    for name in template.unknown_tokens() {
        warnings.push(format!(
            "template: unknown token {{{name}}} renders literally (known: {})",
            KNOWN_TOKENS.join(", ")
        ));
    }
    (template, warnings)
}

struct Rendered {
    text: String,
    saw_token: bool,
    saw_token_value: bool,
}

fn render_segments(segments: &[Segment], values: &TokenValues) -> Rendered {
    let mut out = Rendered {
        text: String::new(),
        saw_token: false,
        saw_token_value: false,
    };
    for segment in segments {
        match segment {
            Segment::Literal(text) => out.text.push_str(text),
            Segment::Token(name) => match values.get(name) {
                Some(value) => {
                    out.saw_token = true;
                    if !value.is_empty() {
                        out.saw_token_value = true;
                        out.text.push_str(value);
                    }
                }
                None => {
                    out.text.push('{');
                    out.text.push_str(name);
                    out.text.push('}');
                }
            },
            Segment::Section(inner) => {
                let rendered = render_segments(inner, values);
                let vanish = rendered.saw_token && !rendered.saw_token_value;
                if !vanish {
                    out.saw_token |= rendered.saw_token;
                    out.saw_token_value |= rendered.saw_token_value;
                    out.text.push_str(&rendered.text);
                }
            }
        }
    }
    out
}

fn parse_segments(
    chars: &mut std::iter::Peekable<std::str::Chars<'_>>,
    in_section: bool,
) -> Result<Vec<Segment>, ParseError> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let flush = |literal: &mut String, segments: &mut Vec<Segment>| {
        if !literal.is_empty() {
            segments.push(Segment::Literal(std::mem::take(literal)));
        }
    };
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                Some(escaped) => literal.push(escaped),
                None => literal.push('\\'),
            },
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(ch) => name.push(ch),
                        None => {
                            return Err(ParseError {
                                message: format!(
                                    "unclosed '{{' before end of template: {{{name}"
                                ),
                            })
                        }
                    }
                }
                flush(&mut literal, &mut segments);
                segments.push(Segment::Token(name));
            }
            '[' => {
                flush(&mut literal, &mut segments);
                segments.push(Segment::Section(parse_segments(chars, true)?));
            }
            ']' if in_section => {
                flush(&mut literal, &mut segments);
                return Ok(segments);
            }
            ch => literal.push(ch),
        }
    }
    if in_section {
        return Err(ParseError {
            message: "unclosed '[' before end of template".into(),
        });
    }
    flush(&mut literal, &mut segments);
    Ok(segments)
}
