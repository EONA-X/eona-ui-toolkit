//! Guards the two properties `ONTOLOGY_CSS` has to hold, neither of which the
//! compiler enforces:
//!
//! 1. It must resolve entirely against `TOKENS_CSS`. These rules were ported
//!    from an app whose stylesheet used generic shadcn-style names (`--accent`,
//!    `--card`, `--radius`). Those were re-pointed at the charter on the way in;
//!    a rule that reintroduces one silently depends on whatever the *host*
//!    happens to call its `--accent`.
//! 2. The themeable layer must stay themeable — every semantic token needs both
//!    a light value and a `[data-theme="dark"]` override, or dark mode has
//!    holes that only show up visually.
//!
//! Was `tests/interactive_tier.rs` until the stateful tier moved to
//! `eona-vocabulary-ui`: `ONTOLOGY_CSS` now covers only `OntoBadge` and
//! `OntoAnnotation`, so the old name named something this crate no longer has.
//! `eona-vocabulary-ui` owns the counterpart check for the ~580 rules it took.

use eona_ui_toolkit::{ONTOLOGY_CSS, TOKENS_CSS};

/// Every `var(--x)` reference in `css`, deduplicated.
fn vars_read(css: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in css.match_indices("var(") {
        let rest = &css[i + 4..];
        let name: String =
            rest.chars().take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
        if name.starts_with("--") && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

/// Every `--x:` declaration in `css`, deduplicated.
fn vars_declared(css: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in css.match_indices("--") {
        let name: String =
            css[i..].chars().take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
        if css[i + name.len()..].starts_with(':') && !out.contains(&name) {
            out.push(name);
        }
    }
    out
}

#[test]
fn ontology_css_reads_only_charter_tokens() {
    let strays: Vec<_> =
        vars_read(ONTOLOGY_CSS).into_iter().filter(|v| !v.starts_with("--eona-")).collect();
    assert!(
        strays.is_empty(),
        "ontology.css reads non-charter custom properties {strays:?}; these resolve against \
         whatever the host defines, not against tokens.css"
    );
}

#[test]
fn every_token_ontology_css_reads_is_actually_defined() {
    let declared = vars_declared(TOKENS_CSS);
    let missing: Vec<_> =
        vars_read(ONTOLOGY_CSS).into_iter().filter(|v| !declared.contains(v)).collect();
    assert!(missing.is_empty(), "ontology.css reads tokens {missing:?} that tokens.css never defines");
}

#[test]
fn the_semantic_layer_is_fully_themed() {
    // Split at the dark block: whatever the light side introduces, the dark side
    // must answer for.
    let dark_at = TOKENS_CSS
        .find(r#":root[data-theme="dark"]"#)
        .expect("tokens.css should carry a [data-theme=\"dark\"] block");
    let (light, dark) = TOKENS_CSS.split_at(dark_at);

    let semantic: Vec<_> = vars_declared(light)
        .into_iter()
        .filter(|v| {
            v.starts_with("--eona-surface-")
                || v.starts_with("--eona-text-")
                || v.starts_with("--eona-border-")
                || v == "--eona-accent"
                || v == "--eona-accent-contrast"
        })
        .collect();
    assert!(!semantic.is_empty(), "no semantic surface tokens found in the light block");

    let overridden = vars_declared(dark);
    let holes: Vec<_> = semantic.into_iter().filter(|v| !overridden.contains(v)).collect();
    assert!(
        holes.is_empty(),
        "semantic tokens {holes:?} have no dark override, so they keep their light value \
         on a dark surface"
    );
}
