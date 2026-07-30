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
fn substitutes_every_token() {
    let template =
        Template::parse("{indicator}herdr:{session} {workspace}/{tab} {agent}:{title}@{host}")
            .expect("valid template");
    assert_eq!(
        template.render(&values()),
        "●2 herdr:personal dotfiles/2 claude:fix titles@mbp"
    );
}
