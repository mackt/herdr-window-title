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
}

#[derive(Debug)]
pub struct Template {
    segments: Vec<Segment>,
}

impl Template {
    pub fn parse(source: &str) -> Result<Self, ParseError> {
        let mut segments = Vec::new();
        let mut literal = String::new();
        let mut chars = source.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '{' => {
                    let mut name = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(ch) => name.push(ch),
                            None => {
                                return Err(ParseError {
                                    message: format!("unclosed '{{' before end of template: {{{name}"),
                                })
                            }
                        }
                    }
                    if !literal.is_empty() {
                        segments.push(Segment::Literal(std::mem::take(&mut literal)));
                    }
                    segments.push(Segment::Token(name));
                }
                ch => literal.push(ch),
            }
        }
        if !literal.is_empty() {
            segments.push(Segment::Literal(literal));
        }
        Ok(Self { segments })
    }

    pub fn render(&self, values: &TokenValues) -> String {
        let mut out = String::new();
        for segment in &self.segments {
            match segment {
                Segment::Literal(text) => out.push_str(text),
                Segment::Token(name) => match values.get(name) {
                    Some(value) => out.push_str(value),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                },
            }
        }
        out
    }
}
