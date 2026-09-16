//! Stage: `classify`. See xtask/README.md for the pipeline contract.
//!
//! Two jobs, both of which the rest of the pipeline takes on trust:
//!
//! 1. Map every `Prop::rust_ty` onto exactly one `PropKind`. There is no
//!    fallback kind: a type this stage does not recognise is a hard error
//!    naming the span, because the alternative — guessing — surfaces
//!    downstream as a TypeScript `any` and a React component whose props do
//!    not match what the Yew component actually reads.
//! 2. Decide each component's `Status` from the #809 scope rules. Every one of
//!    the crate's 42 components ends up with a status, so the ABI can prove
//!    nothing was dropped by accident.
//!
//! Struct fields are classified too, not just component props: a struct prop's
//! fields are matrix axes, because components branch on them
//! (`src/molecules/props_table.rs:59` branches on `PropSpec.default`;
//! `src/molecules/annotation.rs:33-41` matches three ways on
//! `LiteralValue.language`/`.datatype`). Leaving them unclassified is how this
//! generator would silently hard-code one arm of those branches.
#![allow(dead_code, unused_imports)]

use std::collections::HashSet;

use crate::ir::*;
use anyhow::{bail, Result};

/// `src/demo.rs` — the crate's own showcase page, not a library component.
const SHOWCASE: &str = "DemoPage";
/// `src/molecules/modal.rs` hardcodes `inert={true}`, so the open state is
/// unrenderable today; #809 tracks adding `open: bool` upstream.
const MODAL: &str = "Modal";

/// Fills every `Prop::kind` (component props *and* plain-struct fields) and
/// sets every `Component::status`.
pub fn classify(tk: &mut Toolkit) -> Result<()> {
    // Borrowed up front: classification needs to know which names the parser
    // registered as enums, while the props it writes into are behind the same
    // `&mut`.
    let enums: HashSet<String> = tk.enums.iter().map(|e| e.name.clone()).collect();
    let structs: HashSet<String> = tk.structs.iter().map(|s| s.name.clone()).collect();

    for s in &mut tk.structs {
        for f in &mut s.fields {
            classify_prop(f, &enums)?;
        }
    }
    for c in &mut tk.components {
        for p in &mut c.props {
            classify_prop(p, &enums)?;
        }
        c.status = scope(c);
    }

    // A `Struct` kind on a component that will actually be generated must
    // resolve to a registered `PlainStruct`, or its fields are not matrix axes
    // and the emitter hard-codes whichever arm the sentinel render happened to
    // take. Excluded components are exempt, and the exemption is what keeps a
    // component whose prop type this crate cannot see — `BeeNest`'s
    // `OUT_DIR`-generated `BeeNestSpec` was the case that motivated it — from
    // aborting the whole run. No component needs it today; the exemption is
    // exercised by `excluded_components_tolerate_unresolvable_types`.
    for c in &tk.components {
        if matches!(c.status, Status::Excluded { .. }) {
            continue;
        }
        for p in &c.props {
            for name in struct_names(&p.kind) {
                if !structs.contains(&name) {
                    bail!(
                        "{}:{}: prop `{}: {}` names struct `{}`, which `parse` did not register; \
                         its fields are matrix axes and cannot be skipped",
                        p.span.file,
                        p.span.line,
                        p.name,
                        p.rust_ty,
                        name
                    );
                }
            }
        }
    }
    Ok(())
}

/// The TypeScript type for a classified prop.
///
/// `optional` widens with `| undefined` rather than relying on the emitter's
/// `?` alone: under `exactOptionalPropertyTypes` a React caller that spreads a
/// partially-filled object passes `undefined` explicitly, and `foo?: string`
/// rejects that while `foo?: string | undefined` accepts it.
pub fn ts_type(kind: &PropKind, optional: bool, tk: &Toolkit) -> String {
    let base = ts_base(kind, tk);
    if optional {
        format!("{base} | undefined")
    } else {
        base
    }
}

fn ts_base(kind: &PropKind, tk: &Toolkit) -> String {
    match kind {
        PropKind::Text => "string".into(),
        PropKind::Bool => "boolean".into(),
        PropKind::Num { .. } => "number".into(),
        PropKind::Slot => "ReactNode".into(),
        PropKind::Struct { name } => name.clone(),
        PropKind::UnitEnum { name } => ts_union(name, tk),
        PropKind::List { item } => {
            let inner = ts_base(item, tk);
            // `string | undefined[]` would parse as `string | (undefined[])`.
            if inner.contains('|') || inner.contains("=>") {
                format!("({inner})[]")
            } else {
                format!("{inner}[]")
            }
        }
        PropKind::Tuple { items } => {
            let items: Vec<String> = items.iter().map(|i| ts_base(i, tk)).collect();
            format!("[{}]", items.join(", "))
        }
        // `classify` proved the argument type maps before it ever built this
        // variant, so the fallback is unreachable; it is `never` rather than
        // `any` so that if it ever did happen the generated package fails to
        // typecheck instead of silently accepting anything.
        PropKind::Callback { arg } => {
            let ts = parse_ty(arg)
                .and_then(|t| kind_of(&t, &mut false, &HashSet::new(), &no_span(), arg, 0))
                .map(|k| ts_base(&k, tk))
                .unwrap_or_else(|_| "never".into());
            format!("({ts}) => void")
        }
    }
}

/// A fieldless enum becomes a string union of its variants in kebab-case.
///
/// Kebab-casing the *variant name* rather than the class string each enum
/// renders (`ButtonVariant::class()`, `src/atoms/button.rs:11`) is the only
/// convention available for all nine enums: `TextFieldKind`
/// (`src/molecules/text_field.rs:6`) has no `class()` at all — it is compared
/// with `==` — and the class strings that do exist are multi-token CSS
/// (`"btn btn-primary"`), which would make the React API
/// `variant="btn btn-primary"`. The emitter maps the union member back to the
/// variant the harness passes, so the class string never has to be the public
/// name.
fn ts_union(name: &str, tk: &Toolkit) -> String {
    let Some(e) = tk.unit_enum(name) else {
        return "never".into();
    };
    let mut out: Vec<String> = Vec::new();
    for v in &e.variants {
        for lit in variant_literals(v, tk) {
            if !out.contains(&lit) {
                out.push(lit);
            }
        }
    }
    if out.is_empty() {
        return "never".into();
    }
    out.iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The union members one recorded variant contributes.
///
/// Usually one, kebab-cased. `BadgeVariant::Kind(TermKind)`
/// (`src/atoms/onto_badge.rs:20`) is the exception: it wraps another enum, and
/// flattening it reproduces the union the Vue source this was ported from
/// actually declares (`Variant = TermKind | "lang" | "datatype" | ...`, quoted
/// at `src/atoms/onto_badge.rs:17`). When the payload is a concrete variant
/// path (`Kind(TermKind::Class)`) the leaf segment is used; when it is an
/// unregistered type the wrapper's own name is, which is lossy but never
/// wrong-by-omission.
fn variant_literals(variant: &str, tk: &Toolkit) -> Vec<String> {
    let Some(open) = variant.find('(') else {
        return vec![kebab(variant)];
    };
    let head = &variant[..open];
    let payload = variant[open + 1..].trim_end_matches(')').trim();
    let leaf = payload.rsplit("::").next().unwrap_or(payload).trim();
    if leaf.is_empty() {
        return vec![kebab(head)];
    }
    match tk.unit_enum(leaf) {
        // A wrapped enum: flatten it. One level only — no enum in this crate
        // nests further, and a cycle would otherwise not terminate.
        Some(inner) => inner
            .variants
            .iter()
            .map(|v| kebab(v.split('(').next().unwrap_or(v)))
            .collect(),
        None => vec![kebab(leaf)],
    }
}

fn kebab(s: &str) -> String {
    let mut out = String::new();
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            if prev_lower && !out.is_empty() {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower = false;
        } else if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_lower = true;
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
            prev_lower = false;
        }
    }
    out.trim_matches('-').to_string()
}

// ---- scope rules -----------------------------------------------------------

/// The #809 scope rules, in the order they must be applied: the exclusions
/// first, so that an interactive component is recorded as "interactive tier"
/// rather than as the `Callback` it also happens to have.
fn scope(c: &Component) -> Status {
    // `verify` is the only stage that can decide this, and it runs after
    // `classify`; re-running the pipeline must not erase its verdict.
    if let Status::Quarantined { .. } = c.status {
        return c.status.clone();
    }
    // Matches nothing today: `src/interactive/` moved to `eona-vocabulary-ui`.
    // Kept because it is keyed on *shape* (a tier, a path), not on a name, and
    // its subject can come back — that app depends on this crate by relative
    // path and may contribute a component to it. Without this rule a
    // reintroduced stateful component would be rendered by the harness under
    // SSR and emitted into React, which is the silent failure this pipeline
    // exists to catch. The `Callback` rule below is a zero-match shape guard on
    // the same grounds. Pinned by
    // `a_reintroduced_interactive_component_is_still_excluded`.
    if c.tier == Tier::Interactive || c.span.file.replace('\\', "/").contains("src/interactive/") {
        return Status::Excluded { reason: "interactive tier".into() };
    }
    // "demo", not "page": the 18 portal page sections left with the portal, and
    // the three that match now are `DemoPage`'s own sections.
    if c.props_ty.is_none() || c.props.is_empty() {
        return Status::Excluded { reason: "zero-prop demo section".into() };
    }
    if c.name == SHOWCASE {
        return Status::Excluded { reason: "crate showcase page".into() };
    }
    if let Some(p) = c.props.iter().find(|p| matches!(p.kind, PropKind::Callback { .. })) {
        return Status::NeedsOverride {
            reason: format!(
                "`{}: {}` is a Callback ({}:{}); behaviour cannot be pre-rendered",
                p.name, p.rust_ty, p.span.file, p.span.line
            ),
        };
    }
    if c.name == MODAL {
        return Status::NeedsOverride {
            reason: "inert hardcoded; needs open: bool upstream (#809)".into(),
        };
    }
    // One slot is React `children`. Two are not expressible that way, and
    // picking which one wins would be a silent API decision.
    let slots: Vec<&Prop> = c.props.iter().filter(|p| p.kind == PropKind::Slot).collect();
    if slots.len() > 1 {
        let names: Vec<&str> = slots.iter().map(|p| p.name.as_str()).collect();
        return Status::NeedsOverride {
            reason: format!(
                "{} slots ({}) at {}:{}; React `children` expresses only one",
                slots.len(),
                names.join(", "),
                c.span.file,
                c.span.line
            ),
        };
    }
    Status::Ok
}

// ---- type classification ---------------------------------------------------

fn classify_prop(p: &mut Prop, enums: &HashSet<String>) -> Result<()> {
    let ty = parse_ty(&p.rust_ty).map_err(|e| {
        anyhow::anyhow!("{}:{}: prop `{}`: {e}", p.span.file, p.span.line, p.name)
    })?;
    let mut optional = false;
    p.kind = kind_of(&ty, &mut optional, enums, &p.span, &p.name, 0)?;
    // `parse` records `optional` from the syntax it saw; the authoritative
    // answer is the one the type gives, and re-deriving it here keeps a
    // hand-edited IR honest.
    p.optional = optional;
    Ok(())
}

fn kind_of(
    ty: &Ty,
    optional: &mut bool,
    enums: &HashSet<String>,
    span: &Span,
    what: &str,
    depth: usize,
) -> Result<PropKind> {
    let (name, args) = match ty {
        Ty::Tuple(items) => {
            let mut kinds = Vec::with_capacity(items.len());
            for item in items {
                kinds.push(kind_of(item, optional, enums, span, what, depth + 1)?);
            }
            return Ok(PropKind::Tuple { items: kinds });
        }
        Ty::Named { name, args } => (name.as_str(), args),
    };
    let kind = match name {
        // `&'static str` and `&str` arrive here as `str` — references are
        // transparent to classification, and the harness passes string
        // literals, which are already `&'static str`
        // (`src/molecules/nav_dropdown.rs:18`).
        "AttrValue" | "String" | "str" => PropKind::Text,
        "Cow" => PropKind::Text,
        "bool" => PropKind::Bool,
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "i8" | "i16" | "i32" | "i64"
        | "i128" | "isize" | "f32" | "f64" => PropKind::Num { rust: name.to_string() },
        "Html" | "VNode" | "Children" | "ChildrenWithProps" | "ChildrenRenderer" => PropKind::Slot,
        "Option" => {
            let inner = one_arg(name, args, span, what)?;
            if depth > 0 {
                bail!(
                    "{}:{}: `{what}` nests `Option` inside another type; `PropKind` has no way \
                     to express that, so it would have to be flattened silently",
                    span.file,
                    span.line
                );
            }
            *optional = true;
            return kind_of(inner, optional, enums, span, what, depth + 1);
        }
        "Vec" => {
            let inner = one_arg(name, args, span, what)?;
            let item = kind_of(inner, optional, enums, span, what, depth + 1)?;
            PropKind::List { item: Box::new(item) }
        }
        "Callback" => {
            // `Callback<T, R>`: only the argument matters — the return value is
            // never rendered.
            let Some(arg) = args.first() else {
                bail!("{}:{}: `{what}`: `Callback` without a type argument", span.file, span.line);
            };
            // Validate now so `ts_type`, which cannot fail, never has to guess.
            let _ = kind_of(arg, &mut false, enums, span, what, depth + 1)?;
            PropKind::Callback { arg: render_ty(arg) }
        }
        other if enums.contains(other) => PropKind::UnitEnum { name: other.to_string() },
        // A remaining PascalCase path is a struct reachable from a prop.
        // Whether `parse` registered it is checked in `classify` — and only for
        // components that will be generated, so a type this crate cannot see
        // (`BeeNest`'s `OUT_DIR`-generated `BeeNestSpec` was the case that
        // motivated the exemption) does not fail the run.
        other if other.starts_with(|c: char| c.is_ascii_uppercase()) => {
            PropKind::Struct { name: other.to_string() }
        }
        other => bail!(
            "{}:{}: `{what}` has type `{other}`, which classify does not map; add a rule rather \
             than letting it reach TypeScript as `any`",
            span.file,
            span.line
        ),
    };
    Ok(kind)
}

fn one_arg<'a>(name: &str, args: &'a [Ty], span: &Span, what: &str) -> Result<&'a Ty> {
    match args {
        [only] => Ok(only),
        _ => bail!(
            "{}:{}: `{what}`: expected `{name}` to have exactly one type argument, found {}",
            span.file,
            span.line,
            args.len()
        ),
    }
}

fn struct_names(kind: &PropKind) -> Vec<String> {
    match kind {
        PropKind::Struct { name } => vec![name.clone()],
        PropKind::List { item } => struct_names(item),
        PropKind::Tuple { items } => items.iter().flat_map(struct_names).collect(),
        _ => Vec::new(),
    }
}

fn no_span() -> Span {
    Span { file: "<generated>".into(), line: 0 }
}

// ---- a very small Rust type-expression parser ------------------------------
//
// `syn` is a dependency, but `Prop::rust_ty` is text by the time it reaches
// this stage (it survives the ABI round-trip), and the shapes that occur in
// this crate are a closed set: paths with generic arguments, tuples, and
// references. Anything outside that set must fail loudly, which a hand-rolled
// parser does more legibly than pattern-matching `syn::Type`.

#[derive(Clone, Debug, PartialEq)]
enum Ty {
    Named { name: String, args: Vec<Ty> },
    Tuple(Vec<Ty>),
}

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Ident(String),
    Lifetime,
    Lt,
    Gt,
    LParen,
    RParen,
    Comma,
    Amp,
    PathSep,
}

fn lex(s: &str) -> Result<Vec<Tok>> {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        match c {
            c if c.is_whitespace() => i += 1,
            '&' => {
                out.push(Tok::Amp);
                i += 1;
            }
            '<' => {
                out.push(Tok::Lt);
                i += 1;
            }
            '>' => {
                out.push(Tok::Gt);
                i += 1;
            }
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            ':' => {
                if b.get(i + 1) == Some(&':') {
                    out.push(Tok::PathSep);
                    i += 2;
                } else {
                    bail!("stray `:` in type `{s}`");
                }
            }
            '\'' => {
                i += 1;
                while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
                    i += 1;
                }
                out.push(Tok::Lifetime);
            }
            c if c.is_alphanumeric() || c == '_' => {
                let start = i;
                while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
                    i += 1;
                }
                out.push(Tok::Ident(b[start..i].iter().collect()));
            }
            other => bail!("unexpected `{other}` in type `{s}`"),
        }
    }
    Ok(out)
}

fn parse_ty(s: &str) -> Result<Ty> {
    let toks = lex(s)?;
    let mut i = 0;
    let ty = parse_one(&toks, &mut i, s)?;
    if i != toks.len() {
        bail!("trailing tokens in type `{s}`");
    }
    Ok(ty)
}

fn parse_one(toks: &[Tok], i: &mut usize, src: &str) -> Result<Ty> {
    // References and their lifetimes are transparent: the harness owns every
    // value it passes, so `&'static str` classifies exactly like `str`.
    let mut had_ref = false;
    loop {
        match toks.get(*i) {
            Some(Tok::Amp) => {
                had_ref = true;
                *i += 1;
            }
            Some(Tok::Lifetime) if had_ref => *i += 1,
            Some(Tok::Ident(id)) if had_ref && id == "mut" => *i += 1,
            _ => break,
        }
    }
    match toks.get(*i) {
        Some(Tok::LParen) => {
            *i += 1;
            let mut items = Vec::new();
            let mut trailing_comma = false;
            while toks.get(*i) != Some(&Tok::RParen) {
                items.push(parse_one(toks, i, src)?);
                if toks.get(*i) == Some(&Tok::Comma) {
                    *i += 1;
                    trailing_comma = true;
                } else {
                    trailing_comma = false;
                    break;
                }
            }
            if toks.get(*i) != Some(&Tok::RParen) {
                bail!("unterminated tuple in type `{src}`");
            }
            *i += 1;
            match items.len() {
                0 => bail!("unit type `()` in `{src}`"),
                // `(T)` is a parenthesised type, `(T,)` a one-element tuple.
                1 if !trailing_comma => Ok(items.remove(0)),
                _ => Ok(Ty::Tuple(items)),
            }
        }
        Some(Tok::Ident(first)) => {
            let mut name = first.clone();
            *i += 1;
            while toks.get(*i) == Some(&Tok::PathSep) {
                *i += 1;
                match toks.get(*i) {
                    Some(Tok::Ident(seg)) => {
                        name = seg.clone();
                        *i += 1;
                    }
                    _ => bail!("path ends in `::` in type `{src}`"),
                }
            }
            let mut args = Vec::new();
            if toks.get(*i) == Some(&Tok::Lt) {
                *i += 1;
                while toks.get(*i) != Some(&Tok::Gt) {
                    // A lifetime argument carries no information for
                    // classification — `Cow<'static, str>` is `Cow<str>` here.
                    if toks.get(*i) == Some(&Tok::Lifetime) {
                        *i += 1;
                    } else {
                        args.push(parse_one(toks, i, src)?);
                    }
                    if toks.get(*i) == Some(&Tok::Comma) {
                        *i += 1;
                    } else {
                        break;
                    }
                }
                if toks.get(*i) != Some(&Tok::Gt) {
                    bail!("unterminated generic arguments in type `{src}`");
                }
                *i += 1;
            }
            Ok(Ty::Named { name, args })
        }
        _ => bail!("expected a type in `{src}`"),
    }
}

/// The Rust type text for a parsed type, normalised — what goes into
/// `PropKind::Callback::arg`, alongside `PropKind::Num::rust`.
fn render_ty(ty: &Ty) -> String {
    match ty {
        Ty::Tuple(items) => {
            format!("({})", items.iter().map(render_ty).collect::<Vec<_>>().join(", "))
        }
        Ty::Named { name, args } if args.is_empty() => name.clone(),
        Ty::Named { name, args } => {
            format!("{name}<{}>", args.iter().map(render_ty).collect::<Vec<_>>().join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(prop name, `rust_ty` verbatim, line in the `*Props` struct)`.
    type P = (&'static str, &'static str, usize);
    /// `(component, file, line of `#[function_component]`, interactive?, props)`.
    type C = (&'static str, &'static str, usize, bool, &'static [P]);

    /// The crate's own surface, transcribed from `src/` at the commit under
    /// test: all 42 `#[function_component]`s and, for the 39 that take props,
    /// every field of their `*Props` struct with its type verbatim.
    ///
    /// Hand-written rather than produced by `parse`, on purpose: this test has
    /// to fail when the scope rules drift, not when the parser does. It does
    /// mean the table goes stale if the toolkit gains a component — which is
    /// the intended failure, since a new component is exactly the thing #809
    /// says must never be silently dropped.
    const COMPONENTS: &[C] = &[
        // ---- atoms ----
        ("BadgeNum", "src/atoms/badge_num.rs", 10, false, &[("number", "AttrValue", 7)]),
        ("Button", "src/atoms/button.rs", 29, false, &[
            ("label", "AttrValue", 21),
            ("variant", "ButtonVariant", 23),
            ("href", "Option<AttrValue>", 26),
        ]),
        ("CodeBlock", "src/atoms/code_block.rs", 26, false, &[
            ("code", "AttrValue", 13),
            ("language", "AttrValue", 19),
            ("copyable", "bool", 23),
        ]),
        ("OntoBadge", "src/atoms/onto_badge.rs", 65, false, &[
            ("label", "AttrValue", 58),
            ("variant", "BadgeVariant", 60),
            ("title", "Option<AttrValue>", 62),
        ]),
        ("Pill", "src/atoms/pill.rs", 27, false, &[
            ("children", "Children", 22),
            ("tone", "PillTone", 24),
        ]),
        ("PillButton", "src/atoms/pill_button.rs", 36, false, &[
            ("label", "AttrValue", 24),
            ("kind", "PillButtonKind", 26),
            ("href", "Option<AttrValue>", 29),
            ("disabled", "bool", 33),
        ]),
        ("TagLabel", "src/atoms/tag_label.rs", 26, false, &[
            ("text", "AttrValue", 21),
            ("tone", "TagLabelTone", 23),
        ]),
        // ---- the crate's own showcase page ----
        ("DemoPage", "src/demo.rs", 61, false, &[
            ("nav", "Vec<SiteNavLink>", 51),
            ("footer_text", "Option<AttrValue>", 54),
            ("footer_link", "Option<NavLink>", 58),
        ]),
        // ---- molecules ----
        ("AccordionItem", "src/molecules/accordion_item.rs", 16, false, &[
            ("title", "AttrValue", 10),
            ("children", "Children", 11),
            ("open", "bool", 13),
        ]),
        ("OntoAnnotation", "src/molecules/annotation.rs", 43, false, &[
            ("label", "AttrValue", 16),
            ("values", "Vec<LiteralValue>", 17),
        ]),
        ("BgCheckItem", "src/molecules/bg_check_item.rs", 15, false, &[
            ("label", "AttrValue", 6),
            ("background", "AttrValue", 7),
            ("color", "AttrValue", 8),
            ("icon", "AttrValue", 10),
            ("border", "Option<AttrValue>", 12),
        ]),
        ("Callout", "src/molecules/callout.rs", 12, false, &[
            ("label", "AttrValue", 7),
            ("french", "AttrValue", 8),
            ("english", "AttrValue", 9),
        ]),
        ("CaseCard", "src/molecules/case_card.rs", 14, false, &[
            ("href", "AttrValue", 9),
            ("tag", "AttrValue", 10),
            ("title", "AttrValue", 11),
        ]),
        ("DatasetCard", "src/molecules/dataset_card.rs", 41, false, &[
            ("title", "AttrValue", 17),
            ("href", "AttrValue", 19),
            ("description", "Option<AttrValue>", 21),
            ("version", "Option<AttrValue>", 25),
            ("publisher", "Option<AttrValue>", 27),
            ("thumbnail", "Option<AttrValue>", 30),
            ("badge", "Option<AttrValue>", 33),
            ("badge_class", "Option<AttrValue>", 36),
            ("action_label", "AttrValue", 38),
        ]),
        ("DeclCard", "src/molecules/decl_card.rs", 16, false, &[
            ("category", "AttrValue", 8),
            ("src", "AttrValue", 9),
            ("alt", "AttrValue", 10),
            ("decl_type", "AttrValue", 11),
            ("title", "AttrValue", 12),
            ("description", "AttrValue", 13),
        ]),
        ("LogoDownloadCard", "src/molecules/logo_download_card.rs", 21, false, &[
            ("label", "AttrValue", 10),
            ("file", "AttrValue", 12),
            ("download_name", "AttrValue", 14),
            ("preview_style", "AttrValue", 16),
            ("is_transparent", "bool", 18),
        ]),
        ("MemberCard", "src/molecules/member_card.rs", 10, false, &[("name", "AttrValue", 7)]),
        ("MemberFilterCard", "src/molecules/member_filter_card.rs", 17, false, &[
            ("name", "AttrValue", 11),
            ("member_type", "AttrValue", 12),
            ("country", "AttrValue", 13),
            ("sectors", "Vec<&'static str>", 14),
        ]),
        ("Modal", "src/molecules/modal.rs", 19, false, &[
            ("id", "AttrValue", 14),
            ("title", "AttrValue", 15),
            ("children", "Children", 16),
        ]),
        ("MvoCard", "src/molecules/mvo_card.rs", 10, false, &[
            ("tag", "AttrValue", 6),
            ("description", "AttrValue", 7),
        ]),
        ("NavDropdown", "src/molecules/nav_dropdown.rs", 21, false, &[
            ("id", "AttrValue", 16),
            ("label", "AttrValue", 17),
            ("links", "Vec<(&'static str, &'static str)>", 18),
        ]),
        ("NewsCard", "src/molecules/news_card.rs", 15, false, &[
            ("href", "AttrValue", 10),
            ("tag", "AttrValue", 11),
            ("title", "AttrValue", 12),
        ]),
        ("PaletteGroup", "src/molecules/palette_group.rs", 36, false, &[
            ("title", "AttrValue", 28),
            ("description", "AttrValue", 29),
            ("colors", "Vec<ColorSpec>", 30),
            ("note", "Option<AttrValue>", 33),
        ]),
        ("PersonalityCard", "src/molecules/personality_card.rs", 10, false, &[
            ("tag", "AttrValue", 6),
            ("description", "AttrValue", 7),
        ]),
        ("PillarCard", "src/molecules/pillar_card.rs", 12, false, &[
            ("num", "AttrValue", 7),
            ("title", "AttrValue", 8),
            ("points", "Vec<(&'static str, &'static str)>", 9),
        ]),
        ("PropsTable", "src/molecules/props_table.rs", 39, false, &[
            ("component", "AttrValue", 35),
            ("props", "Vec<PropSpec>", 36),
        ]),
        ("RefCard", "src/molecules/ref_card.rs", 13, false, &[
            ("src", "AttrValue", 7),
            ("alt", "AttrValue", 8),
            ("title", "AttrValue", 9),
            ("description", "AttrValue", 10),
        ]),
        ("RuleListItem", "src/molecules/rule_list_item.rs", 27, false, &[
            ("kind", "RuleKind", 23),
            ("children", "Children", 24),
        ]),
        ("SectionHead", "src/molecules/section_head.rs", 14, false, &[
            ("number", "AttrValue", 9),
            ("kicker", "AttrValue", 10),
            ("title", "AttrValue", 11),
        ]),
        ("ShapeCard", "src/molecules/shape_card.rs", 11, false, &[
            ("demo", "Html", 6),
            ("title", "AttrValue", 7),
            ("description", "AttrValue", 8),
        ]),
        ("Swatch", "src/molecules/swatch.rs", 30, false, &[
            ("hex", "AttrValue", 23),
            ("name", "AttrValue", 24),
            ("usage", "AttrValue", 25),
            ("variants", "Vec<AttrValue>", 27),
        ]),
        ("TextField", "src/molecules/text_field.rs", 21, false, &[
            ("id", "AttrValue", 13),
            ("label", "AttrValue", 14),
            ("kind", "TextFieldKind", 16),
            ("placeholder", "AttrValue", 18),
        ]),
        ("TypeScaleRow", "src/molecules/type_scale_row.rs", 13, false, &[
            ("role", "AttrValue", 8),
            ("sample", "Html", 9),
            ("spec", "AttrValue", 10),
        ]),
        ("UsageExample", "src/molecules/usage_example.rs", 23, false, &[
            ("source", "AttrValue", 14),
            ("on_dark", "bool", 18),
            ("children", "Html", 20),
        ]),
        // ---- organisms with props ----
        ("Hero", "src/organisms/hero.rs", 34, false, &[
            ("variant", "HeroVariant", 26),
            ("eyebrow", "AttrValue", 27),
            ("title_suffix", "AttrValue", 28),
            ("lede", "AttrValue", 29),
            ("children", "Children", 31),
        ]),
        ("HeroFilter", "src/organisms/hero_filter.rs", 33, false, &[
            ("title", "AttrValue", 25),
            ("description", "AttrValue", 26),
            ("children", "Children", 28),
            ("with_backdrop", "bool", 30),
        ]),
        ("MainNav", "src/organisms/main_nav.rs", 25, false, &[
            ("back", "Option<NavLink>", 21),
            ("links", "Vec<NavLink>", 22),
        ]),
        ("SimpleFooter", "src/organisms/simple_footer.rs", 12, false, &[
            ("text", "AttrValue", 8),
            ("link", "NavLink", 9),
        ]),
        ("SiteHeader", "src/organisms/site_header.rs", 95, false, &[
            ("links", "Vec<SiteNavLink>", 90),
            ("show_login", "bool", 92),
        ]),
        // ---- zero-prop organisms: the three sections `DemoPage` composes ----
        ("ComponentsGallery", "src/organisms/components_gallery.rs", 18, false, &[]),
        ("IntegrationSection", "src/organisms/integration_section.rs", 21, false, &[]),
        ("OntologySection", "src/organisms/ontology_section.rs", 26, false, &[]),
    ];

    /// The enums `classify` has to see, with their variants as `syn` would
    /// record them. `BadgeVariant::Kind(TermKind)` keeps its payload so the
    /// flattening in `variant_literals` is exercised — which is also why
    /// `TermKind` is listed even though no prop in the crate names it any more
    /// (`Term.kind` left with the interactive tier): `variant_literals` resolves
    /// the payload against this pool, and without it `BadgeVariant` would
    /// collapse from twelve union members to five.
    const ENUMS: &[(&str, &str, usize, &[&str])] = &[
        ("ButtonVariant", "src/atoms/button.rs", 5, &["Primary", "Ghost"]),
        ("PillTone", "src/atoms/pill.rs", 6, &["Ok", "Bad"]),
        ("PillButtonKind", "src/atoms/pill_button.rs", 6, &["Solid", "GlassLight", "GlassDark"]),
        ("TagLabelTone", "src/atoms/tag_label.rs", 5, &["Default", "Blue"]),
        ("HeroVariant", "src/organisms/hero.rs", 8, &["Full", "Compact"]),
        ("TextFieldKind", "src/molecules/text_field.rs", 6, &["Input", "Textarea"]),
        ("RuleKind", "src/molecules/rule_list_item.rs", 5, &["Do", "Dont", "Caution"]),
        ("BadgeVariant", "src/atoms/onto_badge.rs", 20, &[
            "Kind(TermKind)", "Lang", "Datatype", "Deprecated", "Default",
        ]),
        ("TermKind", "src/ontology.rs", 26, &[
            "Ontology", "Class", "ObjectProperty", "DatatypeProperty", "AnnotationProperty",
            "Property", "NamedIndividual", "Other",
        ]),
    ];

    /// The plain structs reachable from a *presentational* prop — which, since
    /// the interactive tier and `BeeNest` left the crate, is all of them. The
    /// registered-struct check's exemption for excluded components is no longer
    /// exercised by a real component and is proved on a mutated fixture instead
    /// (`excluded_components_tolerate_unresolvable_types`).
    const STRUCTS: &[(&str, &str, usize, &[P])] = &[
        ("NavLink", "src/organisms/main_nav.rs", 4, &[
            ("href", "AttrValue", 5),
            ("label", "AttrValue", 6),
        ]),
        ("SiteNavLink", "src/organisms/site_header.rs", 6, &[
            ("href", "AttrValue", 7),
            ("label", "AttrValue", 8),
            ("current", "bool", 9),
        ]),
        ("ColorSpec", "src/molecules/palette_group.rs", 6, &[
            ("name", "AttrValue", 7),
            ("hex", "AttrValue", 8),
            ("usage", "AttrValue", 9),
            ("variants", "Vec<AttrValue>", 10),
        ]),
        ("PropSpec", "src/molecules/props_table.rs", 6, &[
            ("name", "AttrValue", 7),
            ("ty", "AttrValue", 8),
            ("default", "Option<AttrValue>", 11),
            ("doc", "AttrValue", 12),
        ]),
        ("LiteralValue", "src/ontology.rs", 41, &[
            ("value", "String", 42),
            ("language", "Option<String>", 43),
            ("datatype", "Option<String>", 44),
        ]),
    ];

    /// The 38 components #809 scopes in: presentational, and taking props.
    const IN_SCOPE: &[&str] = &[
        "AccordionItem", "BadgeNum", "BgCheckItem", "Button", "Callout", "CaseCard", "CodeBlock",
        "DatasetCard", "DeclCard", "Hero", "HeroFilter", "LogoDownloadCard", "MainNav",
        "MemberCard", "MemberFilterCard", "Modal", "MvoCard", "NavDropdown", "NewsCard",
        "OntoAnnotation", "OntoBadge", "PaletteGroup", "PersonalityCard", "Pill", "PillButton",
        "PillarCard", "PropsTable", "RefCard", "RuleListItem", "SectionHead", "ShapeCard",
        "SimpleFooter", "SiteHeader", "Swatch", "TagLabel", "TextField", "TypeScaleRow",
        "UsageExample",
    ];

    fn prop(p: &P, file: &str) -> Prop {
        Prop {
            name: p.0.into(),
            rust_ty: p.1.into(),
            // Overwritten by `classify`; `Text` is the parser's placeholder.
            kind: PropKind::Text,
            optional: false,
            default: PropDefault::Required,
            span: Span { file: file.into(), line: p.2 },
        }
    }

    fn fixture() -> Toolkit {
        Toolkit {
            components: COMPONENTS
                .iter()
                .map(|(name, file, line, interactive, props)| Component {
                    name: (*name).into(),
                    props_ty: (!props.is_empty()).then(|| format!("{name}Props")),
                    tier: if *interactive { Tier::Interactive } else { Tier::Presentational },
                    status: Status::Ok,
                    props: props.iter().map(|p| prop(p, file)).collect(),
                    span: Span { file: (*file).into(), line: *line },
                })
                .collect(),
            enums: ENUMS
                .iter()
                .map(|(name, file, line, variants)| PropEnum {
                    name: (*name).into(),
                    variants: variants.iter().map(|v| (*v).into()).collect(),
                    default_variant: None,
                    span: Span { file: (*file).into(), line: *line },
                })
                .collect(),
            structs: STRUCTS
                .iter()
                .map(|(name, file, line, fields)| PlainStruct {
                    name: (*name).into(),
                    fields: fields.iter().map(|f| prop(f, file)).collect(),
                    span: Span { file: (*file).into(), line: *line },
                })
                .collect(),
            src_hash: String::new(),
        }
    }

    fn classified() -> Toolkit {
        let mut tk = fixture();
        classify(&mut tk).expect("the crate's own surface must classify");
        tk
    }

    #[test]
    fn the_fixture_is_the_whole_crate() {
        assert_eq!(COMPONENTS.len(), 42, "the crate has 42 function components");
        assert_eq!(
            COMPONENTS.iter().filter(|c| !c.4.is_empty()).count(),
            39,
            "39 of them take props"
        );
    }

    #[test]
    fn scope_is_the_thirty_eight() {
        let tk = classified();
        let mut generated: Vec<&str> = tk
            .components
            .iter()
            .filter(|c| !matches!(c.status, Status::Excluded { .. }))
            .map(|c| c.name.as_str())
            .collect();
        generated.sort_unstable();
        assert_eq!(generated, IN_SCOPE, "in-scope set must be exactly #809's 38");
        assert_eq!(generated.len(), 38);
    }

    #[test]
    fn only_modal_needs_an_override() {
        let tk = classified();
        let overrides: Vec<(&str, &str)> = tk
            .components
            .iter()
            .filter_map(|c| match &c.status {
                Status::NeedsOverride { reason } => Some((c.name.as_str(), reason.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(
            overrides,
            vec![("Modal", "inert hardcoded; needs open: bool upstream (#809)")]
        );
        // 38 in scope, one of which cannot be pre-rendered yet. `Swatch` and
        // `DatasetCard` leave this stage `Ok` on purpose: only `verify` can see
        // that they panic on / transform the sentinel.
        assert_eq!(tk.generated().count(), 37);
    }

    #[test]
    fn every_exclusion_carries_its_rule() {
        let tk = classified();
        let mut by_reason: Vec<(&str, usize)> = Vec::new();
        for c in &tk.components {
            if let Status::Excluded { reason } = &c.status {
                match by_reason.iter_mut().find(|(r, _)| r == reason) {
                    Some((_, n)) => *n += 1,
                    None => by_reason.push((reason, 1)),
                }
            }
        }
        by_reason.sort();
        assert_eq!(
            by_reason,
            vec![("crate showcase page", 1), ("zero-prop demo section", 3)]
        );
        // Nothing may fall through without a status of its own.
        assert_eq!(
            tk.components.iter().filter(|c| matches!(c.status, Status::Excluded { .. })).count()
                + tk.components.iter().filter(|c| !matches!(c.status, Status::Excluded { .. })).count(),
            42
        );
    }

    #[test]
    fn verify_may_still_quarantine() {
        // `classify` must be idempotent across a re-run of the pipeline, or
        // `xtask check` would erase the verify gate's verdict.
        let mut tk = classified();
        let i = tk.components.iter().position(|c| c.name == "Swatch").unwrap();
        tk.components[i].status =
            Status::Quarantined { reason: "src/molecules/swatch.rs:11 slices &hex[0..2]".into() };
        classify(&mut tk).unwrap();
        assert!(matches!(tk.components[i].status, Status::Quarantined { .. }));
    }

    fn kind_of_prop<'a>(tk: &'a Toolkit, component: &str, prop: &str) -> &'a Prop {
        tk.component(component)
            .unwrap_or_else(|| panic!("no component {component}"))
            .props
            .iter()
            .find(|p| p.name == prop)
            .unwrap_or_else(|| panic!("no prop {component}.{prop}"))
    }

    #[test]
    fn types_map_to_the_expected_kinds() {
        let tk = classified();
        let cases: &[(&str, &str, PropKind, bool, &str)] = &[
            ("BadgeNum", "number", PropKind::Text, false, "string"),
            ("CodeBlock", "copyable", PropKind::Bool, false, "boolean"),
            ("Button", "href", PropKind::Text, true, "string | undefined"),
            ("MainNav", "back", PropKind::Struct { name: "NavLink".into() }, true,
                "NavLink | undefined"),
            ("MainNav", "links",
                PropKind::List { item: Box::new(PropKind::Struct { name: "NavLink".into() }) },
                false, "NavLink[]"),
            ("MemberFilterCard", "sectors",
                PropKind::List { item: Box::new(PropKind::Text) }, false, "string[]"),
            ("Pill", "children", PropKind::Slot, false, "ReactNode"),
            ("ShapeCard", "demo", PropKind::Slot, false, "ReactNode"),
            ("Button", "variant", PropKind::UnitEnum { name: "ButtonVariant".into() }, false,
                "\"primary\" | \"ghost\""),
            ("PillButton", "kind", PropKind::UnitEnum { name: "PillButtonKind".into() }, false,
                "\"solid\" | \"glass-light\" | \"glass-dark\""),
            ("TextField", "kind", PropKind::UnitEnum { name: "TextFieldKind".into() }, false,
                "\"input\" | \"textarea\""),
            // `Vec<(&'static str, &'static str)>` — `src/molecules/nav_dropdown.rs:18`.
            ("NavDropdown", "links",
                PropKind::List {
                    item: Box::new(PropKind::Tuple {
                        items: vec![PropKind::Text, PropKind::Text],
                    }),
                },
                false, "[string, string][]"),
        ];
        for (component, name, kind, optional, ts) in cases {
            let p = kind_of_prop(&tk, component, name);
            assert_eq!(&p.kind, kind, "{component}.{name} kind");
            assert_eq!(p.optional, *optional, "{component}.{name} optional");
            assert_eq!(&ts_type(&p.kind, p.optional, &tk), ts, "{component}.{name} ts");
        }
    }

    /// `BadgeVariant::Kind(TermKind)` (`src/atoms/onto_badge.rs:20`) flattens
    /// into the union the Vue source it ports declares.
    #[test]
    fn a_wrapped_enum_flattens() {
        let tk = classified();
        let p = kind_of_prop(&tk, "OntoBadge", "variant");
        assert_eq!(
            ts_type(&p.kind, p.optional, &tk),
            "\"ontology\" | \"class\" | \"object-property\" | \"datatype-property\" \
             | \"annotation-property\" | \"property\" | \"named-individual\" | \"other\" \
             | \"lang\" | \"datatype\" | \"deprecated\" | \"default\""
        );
    }

    /// The branch axes the emitter would otherwise hard-code: without these the
    /// `if let Some(default)` at `src/molecules/props_table.rs:59` and the
    /// three-way match at `src/molecules/annotation.rs:33-41` never render.
    #[test]
    fn nested_struct_fields_are_classified() {
        let tk = classified();
        let spec = tk.plain_struct("PropSpec").unwrap();
        let default = spec.fields.iter().find(|f| f.name == "default").unwrap();
        assert_eq!(default.kind, PropKind::Text);
        assert!(default.optional, "PropSpec.default drives props_table.rs:59");

        let lit = tk.plain_struct("LiteralValue").unwrap();
        for field in ["language", "datatype"] {
            let f = lit.fields.iter().find(|f| f.name == field).unwrap();
            assert!(f.optional, "LiteralValue.{field} drives annotation.rs:33-41");
        }
        assert!(!lit.fields.iter().find(|f| f.name == "value").unwrap().optional);
    }

    /// An excluded component's props are still classified — recorded, not
    /// skipped — so the ABI can show what it takes even though no `.tsx` is
    /// emitted for it. The crate's only `Callback`s were `OntoSection::on_toggle`
    /// and `OntologySelector::on_change`, both of which left with the
    /// interactive tier, so this runs on a reintroduced one.
    #[test]
    fn callbacks_classify_even_though_excluded() {
        let mut tk = fixture();
        tk.components.push(Component {
            name: "OntoSection".into(),
            props_ty: Some("OntoSectionProps".into()),
            tier: Tier::Interactive,
            status: Status::Ok,
            props: vec![Prop {
                name: "on_toggle".into(),
                rust_ty: "Callback<bool>".into(),
                kind: PropKind::Text,
                optional: false,
                default: PropDefault::Required,
                span: Span { file: "src/interactive/organisms/section.rs".into(), line: 26 },
            }],
            span: Span { file: "src/interactive/organisms/section.rs".into(), line: 29 },
        });
        classify(&mut tk).unwrap();
        let p = kind_of_prop(&tk, "OntoSection", "on_toggle");
        assert_eq!(p.kind, PropKind::Callback { arg: "bool".into() });
        assert_eq!(ts_type(&p.kind, false, &tk), "(boolean) => void");
    }

    /// The tier left the crate with this move; the rule did not. A component
    /// reintroduced under `src/interactive/` holds state, and the harness would
    /// render it under SSR and emit React for it if this stopped excluding —
    /// note the `Callback` rule, which would otherwise fire second, does not get
    /// the chance.
    #[test]
    fn a_reintroduced_interactive_component_is_still_excluded() {
        let mut tk = fixture();
        tk.components.push(Component {
            name: "ThemeToggle".into(),
            props_ty: Some("ThemeToggleProps".into()),
            // `parse` sets this from the path; a component added by hand with
            // the wrong tier must still be caught by the path half of the rule.
            tier: Tier::Presentational,
            status: Status::Ok,
            props: vec![Prop {
                name: "storage_key".into(),
                rust_ty: "AttrValue".into(),
                kind: PropKind::Text,
                optional: false,
                default: PropDefault::Required,
                span: Span { file: "src/interactive/atoms/theme_toggle.rs".into(), line: 69 },
            }],
            span: Span { file: "src/interactive/atoms/theme_toggle.rs".into(), line: 77 },
        });
        classify(&mut tk).unwrap();
        assert_eq!(
            tk.component("ThemeToggle").unwrap().status,
            Status::Excluded { reason: "interactive tier".into() }
        );
    }

    /// A `Callback` on a *presentational* component is the rule that must fire
    /// before `Ok` — none exist today, so it is proved on a mutated fixture
    /// rather than left untested.
    #[test]
    fn a_callback_forces_an_override() {
        let mut tk = fixture();
        let i = tk.components.iter().position(|c| c.name == "Button").unwrap();
        tk.components[i].props.push(Prop {
            name: "on_click".into(),
            rust_ty: "Callback<String>".into(),
            kind: PropKind::Text,
            optional: false,
            default: PropDefault::DefaultTrait,
            span: Span { file: "src/atoms/button.rs".into(), line: 27 },
        });
        classify(&mut tk).unwrap();
        match &tk.components[i].status {
            Status::NeedsOverride { reason } => {
                assert!(reason.contains("on_click"), "{reason}");
                assert!(reason.contains("src/atoms/button.rs:27"), "{reason}");
            }
            other => panic!("expected NeedsOverride, got {other:?}"),
        }
    }

    /// One slot is `children`; two are not expressible as `children`.
    #[test]
    fn two_slots_force_an_override() {
        let mut tk = fixture();
        let i = tk.components.iter().position(|c| c.name == "ShapeCard").unwrap();
        tk.components[i].props.push(Prop {
            name: "footer".into(),
            rust_ty: "Html".into(),
            kind: PropKind::Text,
            optional: false,
            default: PropDefault::DefaultTrait,
            span: Span { file: "src/molecules/shape_card.rs".into(), line: 9 },
        });
        classify(&mut tk).unwrap();
        match &tk.components[i].status {
            Status::NeedsOverride { reason } => assert!(reason.contains("demo, footer"), "{reason}"),
            other => panic!("expected NeedsOverride, got {other:?}"),
        }
    }

    #[test]
    fn an_unmapped_type_names_its_span() {
        let mut tk = fixture();
        let i = tk.components.iter().position(|c| c.name == "BadgeNum").unwrap();
        tk.components[i].props[0].rust_ty = "std::time::Duration".into();
        // `Duration` is PascalCase, so it classifies as a struct and then trips
        // the registered-struct check — the diagnostic a human can act on.
        let err = classify(&mut tk).unwrap_err().to_string();
        assert!(err.contains("src/atoms/badge_num.rs:7"), "{err}");
        assert!(err.contains("Duration"), "{err}");

        let mut tk = fixture();
        tk.components[i].props[0].rust_ty = "rc_str".into();
        let err = classify(&mut tk).unwrap_err().to_string();
        assert!(err.contains("src/atoms/badge_num.rs:7"), "{err}");
        assert!(err.contains("does not map"), "{err}");
    }

    /// A prop typed on something `parse` never registered aborts the run for an
    /// in-scope component (`an_unmapped_type_names_its_span`) but must not for
    /// an excluded one. `BeeNest`'s `&'static BeeNestSpec` — a type `build.rs`
    /// wrote into `OUT_DIR` — was the real case; it left with the BeeNest move,
    /// so the exemption is proved on a fixture that reintroduces the shape.
    #[test]
    fn excluded_components_tolerate_unresolvable_types() {
        let mut tk = fixture();
        tk.components.push(Component {
            name: "BeeNest".into(),
            props_ty: Some("BeeNestProps".into()),
            tier: Tier::Presentational,
            status: Status::Ok,
            props: vec![Prop {
                name: "spec".into(),
                rust_ty: "&'static BeeNestSpec".into(),
                kind: PropKind::Text,
                optional: false,
                default: PropDefault::Required,
                span: Span { file: "src/interactive/organisms/beenest.rs".into(), line: 312 },
            }],
            span: Span { file: "src/interactive/organisms/beenest.rs".into(), line: 315 },
        });
        classify(&mut tk).expect("an excluded component's unregistered struct is not fatal");
        let p = kind_of_prop(&tk, "BeeNest", "spec");
        assert_eq!(p.kind, PropKind::Struct { name: "BeeNestSpec".into() });
        assert!(tk.plain_struct("BeeNestSpec").is_none(), "the point of the exemption");
    }

    #[test]
    fn the_type_parser_handles_the_shapes_that_occur() {
        let cases = [
            ("AttrValue", "AttrValue"),
            ("&'static str", "str"),
            ("&str", "str"),
            ("Cow<'static, str>", "Cow<str>"),
            ("Option < AttrValue >", "Option<AttrValue>"),
            ("Vec<(&'static str, &'static str)>", "Vec<(str, str)>"),
            ("yew::Callback<bool>", "Callback<bool>"),
            ("Option<Vec<NavLink>>", "Option<Vec<NavLink>>"),
        ];
        for (src, want) in cases {
            assert_eq!(render_ty(&parse_ty(src).unwrap()), want, "parsing {src}");
        }
        assert!(parse_ty("Vec<").is_err());
        assert!(parse_ty("()").is_err());
    }

    #[test]
    fn kebab_case_matches_the_variant_names() {
        assert_eq!(kebab("Primary"), "primary");
        assert_eq!(kebab("GlassLight"), "glass-light");
        assert_eq!(kebab("ObjectProperty"), "object-property");
        assert_eq!(kebab("Dont"), "dont");
    }

    /// The one cross-stage contract the fixture cannot check: `parse` and
    /// `classify` must agree on what `BadgeVariant` (`src/atoms/onto_badge.rs:20`)
    /// is. They disagreed once already — `parse` withheld the enum entirely and
    /// `classify` could then map no type for `OntoBadge.variant`, so the whole
    /// run aborted on a component that is in scope. This drives the real crate
    /// end to end so the two cannot drift again.
    #[test]
    fn the_real_crate_classifies_end_to_end() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let mut tk = crate::parse::parse(&root).unwrap();
        classify(&mut tk).expect("every in-scope component maps");

        // Twelve inhabitants, each one a value the harness can construct, and
        // each one a distinct union member — the eight `Kind(..)` arms at
        // onto_badge.rs:40-46 must not collapse.
        let union = ts_union("BadgeVariant", &tk);
        assert_eq!(union.matches('|').count() + 1, 12, "{union}");
        for member in [
            "\"ontology\"",
            "\"object-property\"",
            "\"named-individual\"",
            "\"lang\"",
            "\"deprecated\"",
            "\"default\"",
        ] {
            assert!(union.contains(member), "{member} missing from {union}");
        }

        let scope: Vec<&str> = tk
            .components
            .iter()
            .filter(|c| !matches!(c.status, Status::Excluded { .. }))
            .map(|c| c.name.as_str())
            .collect();
        assert_eq!(scope.len(), 38, "#809's list: {scope:?}");
        assert_eq!(
            tk.components.iter().filter(|c| matches!(c.status, Status::Ok)).count(),
            37,
            "38 less Modal, which needs `open: bool` upstream"
        );

        // No prop anywhere may reach the emitter as an unmappable type.
        for c in tk.components.iter().filter(|c| !matches!(c.status, Status::Excluded { .. })) {
            for p in &c.props {
                if let PropKind::Struct { name } = &p.kind {
                    assert!(
                        tk.plain_struct(name).is_some(),
                        "{}.{} is `{name}` ({}:{}), which parse never registered",
                        c.name,
                        p.name,
                        p.span.file,
                        p.span.line
                    );
                }
                if let PropKind::UnitEnum { name } = &p.kind {
                    assert!(
                        tk.unit_enum(name).is_some(),
                        "{}.{} is `{name}` ({}:{}), which parse never registered",
                        c.name,
                        p.name,
                        p.span.file,
                        p.span.line
                    );
                }
            }
        }
    }
}
