//! Guards the two properties the site header's menu has to hold, neither of
//! which the compiler — or a screenshot of the top of a page — will catch.
//!
//! 1. The backdrop stays painted once the page scrolls. `.site-header` is
//!    honestly transparent (it reproduces eona-x.eu's own `#en-tete`, which
//!    never paints itself); `.site-header::before` supplies the dark strip in
//!    its place. A `.site-header.is-scrolled::before { content: none }` rule
//!    used to drop that strip the moment the page scrolled, on the reasoning
//!    that the real site tolerates a see-through bar there. The real site can:
//!    its pages are dark under the header. This portal's are not — they are
//!    `--eona-gray-bg` (#F7F9FE) — and `.site-header__link` is white at 75%,
//!    so a scrolled header rendered its nav links at a **measured 1.04:1** on
//!    editor.html and **1.00:1** over the white cards on index.html. Both are
//!    WCAG 1.4.3 failures, and they were invisible to any check that only ever
//!    looked at the top of a page, where the backdrop is still there.
//!
//! 2. The menu's colours are parameters, not a palette the component dictates.
//!    A design-system organism that hard-codes one navy rectangle and one
//!    white-75% ink cannot be used by a consumer whose header is not navy —
//!    their only recourse is to override the component's own selectors, which
//!    puts this crate's internal class names into their stylesheet.
//!
//! The third test is the one that makes the second safe. The tokens are a
//! *pair*: a background and the ink that sits on it. Exposing them as two
//! independent knobs invites exactly the failure above in a new costume — a
//! consumer lightens the bar, leaves the ink at white-75%, and lands back at
//! 1.x:1. This computes the real contrast of the shipped defaults, in both
//! colour modes, through the same stack the browser paints (the chip's
//! translucent wash sits between the ink and the bar and measurably *lowers*
//! the ratio, 9.95:1 down to 7.97:1, so leaving it out would flatter the
//! result), and fails if a future edit to either half of a pair breaks it.

use eona_ui_toolkit::{COMPONENTS_CSS, TOKENS_CSS};

/// The stylesheet's SiteHeader section, comments removed. Every rule that
/// styles the header lives between these two section banners.
fn menu_rules() -> String {
    let start = COMPONENTS_CSS
        .find("/* ---------- SiteHeader")
        .expect("components.css should carry a SiteHeader section banner");
    let end = COMPONENTS_CSS[start..]
        .find("/* ---------- HeroFilter")
        .expect("the SiteHeader section should be followed by HeroFilter's banner")
        + start;
    strip_comments(&COMPONENTS_CSS[start..end])
}

fn strip_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        match rest[i..].find("*/") {
            Some(j) => rest = &rest[i + j + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// Colour arithmetic. Enough of it to compose the shipped defaults exactly the
// way the compositor does, and no more.
// ---------------------------------------------------------------------------

/// Straight (non-premultiplied) sRGB with an alpha channel, 0-255 / 0-1.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgba(f64, f64, f64, f64);

/// Resolves one `var(--x)` chain down to a literal colour, reading `block`
/// first and falling back to `TOKENS_CSS` as a whole — which is how the
/// cascade behaves for a `[data-theme]` block that only overrides some of the
/// semantic layer.
fn resolve(block: &str, value: &str) -> Rgba {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix("var(") {
        let name: String =
            rest.chars().take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
        let next = declaration(block, &name)
            .or_else(|| declaration(TOKENS_CSS, &name))
            .unwrap_or_else(|| panic!("tokens.css never declares {name}, which {value} reads"));
        return resolve(block, &next);
    }
    parse_color(value).unwrap_or_else(|| panic!("cannot read {value} as a colour"))
}

/// The value of `--name` in `css`, last declaration wins (as in the cascade).
fn declaration(css: &str, name: &str) -> Option<String> {
    let needle = format!("{name}:");
    let mut found = None;
    for (i, _) in css.match_indices(&needle) {
        // Guard against `--eona-text-on-header:` also matching a lookup for
        // `--eona-text-on-header-active:` — the name must end at the colon.
        let before = css[..i].chars().last().unwrap_or(' ');
        if before.is_alphanumeric() || before == '-' {
            continue;
        }
        let tail = &css[i + needle.len()..];
        let end = tail.find(';').unwrap_or(tail.len());
        found = Some(tail[..end].trim().to_string());
    }
    found
}

fn parse_color(v: &str) -> Option<Rgba> {
    if let Some(hex) = v.strip_prefix('#') {
        let n = |s: &str| u8::from_str_radix(s, 16).ok().map(f64::from);
        return match hex.len() {
            6 => Some(Rgba(n(&hex[0..2])?, n(&hex[2..4])?, n(&hex[4..6])?, 1.0)),
            3 => {
                let d = |c: char| n(&format!("{c}{c}"));
                let mut it = hex.chars();
                Some(Rgba(d(it.next()?)?, d(it.next()?)?, d(it.next()?)?, 1.0))
            }
            _ => None,
        };
    }
    let inner = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb("))?.trim_end_matches(')');
    let n: Vec<f64> = inner.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    match n.len() {
        3 => Some(Rgba(n[0], n[1], n[2], 1.0)),
        4 => Some(Rgba(n[0], n[1], n[2], n[3])),
        _ => None,
    }
}

/// `top` composited over `bottom`, source-over in sRGB — what the compositor
/// does with a translucent fill, and what WCAG then measures.
fn over(top: Rgba, bottom: Rgba) -> Rgba {
    let a = top.3 + bottom.3 * (1.0 - top.3);
    let mix = |t: f64, b: f64| (t * top.3 + b * bottom.3 * (1.0 - top.3)) / a;
    Rgba(mix(top.0, bottom.0), mix(top.1, bottom.1), mix(top.2, bottom.2), a)
}

fn luminance(c: Rgba) -> f64 {
    let f = |v: f64| {
        let v = v / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.0) + 0.7152 * f(c.1) + 0.0722 * f(c.2)
}

fn contrast(a: Rgba, b: Rgba) -> f64 {
    let (hi, lo) = {
        let (x, y) = (luminance(a), luminance(b));
        if x > y {
            (x, y)
        } else {
            (y, x)
        }
    };
    (hi + 0.05) / (lo + 0.05)
}

/// The two colour modes, as the two slices of `tokens.css` the cascade picks
/// between. Mirrors `ontology_css.rs`'s own split at the dark block.
fn modes() -> [(&'static str, &'static str); 2] {
    let dark_at = TOKENS_CSS
        .find(r#":root[data-theme="dark"]"#)
        .expect("tokens.css should carry a [data-theme=\"dark\"] block");
    let (light, dark) = TOKENS_CSS.split_at(dark_at);
    [("light", light), ("dark", dark)]
}

// ---------------------------------------------------------------------------

#[test]
fn the_scrolled_header_still_paints_its_backdrop() {
    let css = menu_rules();
    let offenders: Vec<&str> = css
        .split('}')
        .map(str::trim)
        .filter(|rule| rule.contains("is-scrolled") && rule.contains("::before"))
        .filter(|rule| {
            let body = rule.split('{').nth(1).unwrap_or("");
            body.contains("content: none")
                || body.contains("content:none")
                || body.contains("display: none")
                || body.contains("background: none")
                || body.contains("background: transparent")
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "the scrolled header drops its own backdrop: {offenders:?}\n\
         `.site-header` paints nothing itself, so without `::before` the nav links \
         (white at 75%) render straight onto page content — measured 1.04:1 on \
         editor.html and 1.00:1 over index.html's white cards, both WCAG 1.4.3 failures"
    );
}

#[test]
fn the_menu_hard_codes_no_colour_of_its_own() {
    let css = menu_rules();
    let mut literals: Vec<String> = Vec::new();
    for (i, _) in css.match_indices('#') {
        let lit: String = css[i..].chars().take_while(|c| !c.is_whitespace() && *c != ';').collect();
        literals.push(lit);
    }
    for pat in ["rgb(", "rgba("] {
        for (i, _) in css.match_indices(pat) {
            let lit: String = css[i..].chars().take_while(|c| *c != ')').collect();
            literals.push(lit + ")");
        }
    }
    assert!(
        literals.is_empty(),
        "the SiteHeader rules still hard-code colours {literals:?}; the menu's background \
         and ink are meant to be parameters (`--eona-surface-header`, `--eona-text-on-header` \
         and their pairs in tokens.css), so a consumer whose header is not navy can theme it \
         without overriding this crate's own class names"
    );
}

#[test]
fn the_default_menu_pair_clears_wcag_aa_in_both_modes() {
    // Composited exactly as the browser paints it: the bar, then the nav pill's
    // translucent chip on top of it, then the ink on top of that. The links live
    // inside the pill, so the chip is part of their background, and it lightens
    // the bar toward the ink rather than away from it.
    for (mode, block) in modes() {
        let bar = resolve(block, "var(--eona-surface-header)");
        let chip = resolve(block, "var(--eona-surface-header-chip)");
        let behind = over(chip, bar);
        let ink = over(resolve(block, "var(--eona-text-on-header)"), behind);
        let ratio = contrast(ink, behind);
        assert!(
            ratio >= 4.5,
            "{mode}: the menu's resting ink measures {ratio:.2}:1 on its own bar — below the \
             WCAG 1.4.3 AA floor of 4.5:1. `--eona-surface-header` and `--eona-text-on-header` \
             are a pair; moving one without the other is what this guards"
        );

        // The current-page chip is its own pair, and inverts the first one.
        let active = resolve(block, "var(--eona-surface-header-active)");
        let active_ink = over(resolve(block, "var(--eona-text-on-header-active)"), active);
        let active_ratio = contrast(active_ink, active);
        assert!(
            active_ratio >= 4.5,
            "{mode}: the current-page chip's label measures {active_ratio:.2}:1 on the chip — \
             below the WCAG 1.4.3 AA floor of 4.5:1"
        );

        // Hover only ever raises the ink toward the strong end, but it is a
        // separate token and can be themed into the floor on its own.
        let hover_bg = over(resolve(block, "var(--eona-surface-header-chip)"), bar);
        let hover_ink = over(resolve(block, "var(--eona-text-on-header-strong)"), hover_bg);
        let hover_ratio = contrast(hover_ink, hover_bg);
        assert!(
            hover_ratio >= 4.5,
            "{mode}: the menu's hover ink measures {hover_ratio:.2}:1 on the hovered chip — \
             below the WCAG 1.4.3 AA floor of 4.5:1"
        );
    }
}

#[test]
fn every_token_the_menu_reads_is_actually_defined() {
    let css = menu_rules();
    let mut missing: Vec<String> = Vec::new();
    for (i, _) in css.match_indices("var(") {
        let rest = &css[i + 4..];
        let name: String =
            rest.chars().take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect();
        // `var(--eona-blur, 16px)` carries its own fallback: it is a knob this
        // stylesheet offers, deliberately undeclared, not a token it depends on.
        let has_fallback = rest[name.len()..].trim_start().starts_with(',');
        if !has_fallback
            && declaration(TOKENS_CSS, &name).is_none()
            && !missing.contains(&name)
        {
            missing.push(name);
        }
    }
    assert!(
        missing.is_empty(),
        "the SiteHeader rules read tokens {missing:?} that tokens.css never declares, so they \
         resolve against whatever the host happens to define — or against nothing at all"
    );
}
