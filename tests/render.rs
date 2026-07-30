//! Secondary seam: the pure render function. Table-driven tests for the
//! template engine — no processes, no sockets.

use herdr_window_title::template::{Template, TokenValues};

fn values() -> TokenValues {
    TokenValues {
        indicator: "●2 ".into(),
        session: "personal".into(),
        workspace: "dotfiles".into(),
        tab: "2".into(),
        agent: "claude".into(),
        title: "fix titles".into(),
        host: "mbp".into(),
    }
}

#[test]
fn optional_section_vanishes_when_all_inner_tokens_are_empty() {
    let template = Template::parse("{indicator}herdr:{session}[ · {workspace}]").expect("valid");
    let mut empty_extras = values();
    empty_extras.indicator = String::new();
    empty_extras.workspace = String::new();
    assert_eq!(template.render(&empty_extras), "herdr:personal");
}

#[test]
fn optional_section_renders_when_any_inner_token_has_a_value() {
    let template = Template::parse("herdr:{session}[ · {workspace}]").expect("valid");
    assert_eq!(template.render(&values()), "herdr:personal · dotfiles");
}

#[test]
fn backslash_escapes_brackets_and_braces() {
    let template = Template::parse(r"\[{session}\] \{literal\}").expect("valid");
    assert_eq!(template.render(&values()), "[personal] {literal}");
}

#[test]
fn unknown_token_renders_literally_for_self_diagnosis() {
    let template = Template::parse("herdr:{sesion}").expect("unknown tokens are not parse errors");
    assert_eq!(template.render(&values()), "herdr:{sesion}");
}

#[test]
fn unknown_token_inside_section_keeps_the_section() {
    let template = Template::parse("x[ {typo}]").expect("valid");
    assert_eq!(template.render(&values()), "x {typo}");
}

#[test]
fn section_with_no_tokens_renders_as_is() {
    let template = Template::parse("a[ fixed ]b").expect("valid");
    assert_eq!(template.render(&values()), "a fixed b");
}

#[test]
fn unclosed_brace_and_bracket_are_parse_errors() {
    assert!(Template::parse("herdr:{session").is_err());
    assert!(Template::parse("herdr:{session}[ · {workspace}").is_err());
}

#[test]
fn substitutes_every_token() {
    let template =
        Template::parse("{indicator}herdr:{session} {workspace}/{tab} {agent}:{title}@{host}")
            .expect("valid template");
    assert_eq!(
        template.render(&values()),
        "●2 herdr:personal dotfiles/2 claude:fix titles@mbp"
    );
}
