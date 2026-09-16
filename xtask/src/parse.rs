//! Stage: `parse`. See xtask/README.md for the pipeline contract.
//!
//! Reads the toolkit crate's own `src/**/*.rs` with `syn` and fills the
//! `Toolkit` IR. Nothing here decides scope: every `#[function_component]` in
//! the tree lands in the IR with `Status::Ok`, and `classify.rs` applies the
//! exclusion rules, so a component can never vanish by being skipped here.
//!
//! The one judgement this stage does make is `Tier`, which is a fact about the
//! file path rather than an opinion: `src/lib.rs:38` gates `pub mod interactive`
//! behind `csr` because that tier holds state and touches the DOM.
//!
//! Line numbers come from `proc-macro2`'s `span-locations` feature (enabled in
//! xtask/Cargo.toml). Outside a proc-macro server `proc_macro2` runs its own
//! fallback lexer, which records a real `LineColumn` per token, so
//! `span().start().line` is exact and no text-search fallback is needed.
#![allow(dead_code, unused_imports)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use quote::ToTokens;
use sha2::{Digest, Sha256};
use syn::visit::Visit;
use walkdir::WalkDir;

use crate::ir::*;

/// A `#[derive(Properties)]` struct. Kept out of `Toolkit::structs` because its
/// fields become a component's `props`, not a nested matrix axis; only the plain
/// structs reachable *through* those fields are axes.
#[derive(Clone, Debug)]
pub struct PropsStruct {
    pub name: String,
    pub fields: Vec<Prop>,
    pub span: Span,
}

/// Everything the visitor found, before reachability filtering and before props
/// are attached to components. `parse` turns this into a `Toolkit`; the tests
/// assert against it directly because the raw counts (76 components, 48
/// `Properties` structs) are the contract with the crate under test.
#[derive(Clone, Debug, Default)]
pub struct Collected {
    pub components: Vec<Component>,
    pub props_structs: Vec<PropsStruct>,
    /// Every fieldless enum in the crate, before reachability filtering.
    pub enums: Vec<PropEnum>,
    /// Enums that carry data, so they cannot become a TypeScript string union.
    /// `BadgeVariant` (`src/atoms/onto_badge.rs:20`) is the crate's only one,
    /// and `OntoBadge` — a component in scope — takes it as a prop. It is kept
    /// here rather than flattened into `enums`: flattening `Kind(TermKind)` to
    /// the member `"Kind"` would drop seven of the variant's eight renderings
    /// (`onto_badge.rs:40-46`) and no safety check downstream would notice.
    pub data_enums: Vec<PropEnum>,
    /// Every non-`Properties` struct in the crate, before reachability
    /// filtering. Deliberately includes private ones: `PropSpec`
    /// (`src/molecules/props_table.rs:6`) is `pub`, but nothing guarantees the
    /// next nested axis will be.
    pub plain_structs: Vec<PlainStruct>,
    pub src_hash: String,
}

pub fn parse(root: &Path) -> Result<Toolkit> {
    link(collect(root)?)
}

// ---- walking ---------------------------------------------------------------

/// `src/**/*.rs`, as (slash-separated path relative to `root`, bytes), sorted by
/// path so both the hash and the IR ordering are machine-independent.
fn source_files(root: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let src = root.join("src");
    if !src.is_dir() {
        bail!("{} has no src/ directory — is this the eona-ui-toolkit root?", root.display());
    }
    let mut files = Vec::new();
    for entry in WalkDir::new(&src).follow_links(false) {
        let entry = entry.with_context(|| format!("walking {}", src.display()))?;
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .with_context(|| format!("{} is not under {}", entry.path().display(), root.display()))?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let bytes = std::fs::read(entry.path())
            .with_context(|| format!("reading {}", entry.path().display()))?;
        files.push((rel, bytes));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(files)
}

/// sha256 over the sorted `(path, bytes)` pairs, each length-prefixed so that no
/// rename can collide with a content change.
fn src_hash(files: &[(String, Vec<u8>)]) -> String {
    let mut hasher = Sha256::new();
    for (path, bytes) in files {
        hasher.update((path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    format!("{:x}", hasher.finalize())
}

pub fn collect(root: &Path) -> Result<Collected> {
    let files = source_files(root)?;
    let mut out = Collected { src_hash: src_hash(&files), ..Default::default() };

    for (rel, bytes) in &files {
        let text = std::str::from_utf8(bytes).with_context(|| format!("{rel} is not UTF-8"))?;
        let ast = syn::parse_file(text).with_context(|| format!("parsing {rel}"))?;
        let mut visitor = FileVisitor {
            file: rel.clone(),
            tier: if rel.starts_with("src/interactive/") {
                Tier::Interactive
            } else {
                Tier::Presentational
            },
            out: &mut out,
        };
        visitor.visit_file(&ast);
    }
    Ok(out)
}

// ---- visitor ---------------------------------------------------------------

struct FileVisitor<'a> {
    file: String,
    tier: Tier,
    out: &'a mut Collected,
}

impl FileVisitor<'_> {
    fn span(&self, span: proc_macro2::Span) -> Span {
        Span { file: self.file.clone(), line: span.start().line }
    }
}

impl<'ast> Visit<'ast> for FileVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if let Some(name) = component_name(&node.attrs, &node.sig.ident) {
            self.out.components.push(Component {
                name,
                props_ty: props_arg_ty(&node.sig),
                tier: self.tier,
                // `classify.rs` owns every status transition; parse only reports
                // what exists.
                status: Status::Ok,
                props: Vec::new(),
                span: self.span(node.sig.ident.span()),
            });
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        let span = self.span(node.ident.span());
        let name = node.ident.to_string();
        let fields = node
            .fields
            .iter()
            .filter_map(|f| f.ident.as_ref().map(|ident| prop(ident, f, &self.file)))
            .collect::<Vec<_>>();

        if derives(&node.attrs).iter().any(|d| d == "Properties") {
            self.out.props_structs.push(PropsStruct { name, fields, span });
        } else if matches!(node.fields, syn::Fields::Named(_)) {
            // Tuple and unit structs cannot be a matrix axis — they have no
            // named fields to vary — so they are not candidates.
            self.out.plain_structs.push(PlainStruct { name, fields, span });
        }
        syn::visit::visit_item_struct(self, node);
    }

    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        // Variants are recorded verbatim, payload included, so that a
        // data-carrying variant keeps the type it wraps: `BadgeVariant::Kind`
        // (`src/atoms/onto_badge.rs:21`) is stored as `Kind(TermKind)`, not as
        // the bare `Kind`. Dropping the payload here would collapse the eight
        // `Kind(..)` renderings at `onto_badge.rs:40-46` into one before any
        // later stage could see them; `link` expands it instead.
        let recorded = PropEnum {
            name: node.ident.to_string(),
            variants: node.variants.iter().map(variant_text).collect(),
            default_variant: default_variant(node),
            span: self.span(node.ident.span()),
        };
        if node.variants.iter().all(|v| matches!(v.fields, syn::Fields::Unit)) {
            self.out.enums.push(recorded);
        } else {
            self.out.data_enums.push(recorded);
        }
        syn::visit::visit_item_enum(self, node);
    }
}

/// The variant marked `#[default]`, which is what `#[prop_or_default]` on a
/// prop of this type resolves to.
///
/// `derive(Default)` only accepts `#[default]` on a *unit* variant, so the name
/// recorded here is always a bare ident and always survives
/// `expand_data_enums` unchanged — `BadgeVariant::Default`
/// (`src/atoms/onto_badge.rs:24`) is still spelled `Default` after `Kind(..)`
/// has been expanded into its eight inhabitants.
fn default_variant(node: &syn::ItemEnum) -> Option<String> {
    node.variants
        .iter()
        .find(|v| v.attrs.iter().any(|a| a.path().is_ident("default")))
        .map(|v| v.ident.to_string())
}

/// One enum variant as text. A unit variant is its bare ident; a tuple variant
/// keeps its payload type (`Kind(TermKind)`) so `link` can expand it and
/// `classify.rs` can map it to a TypeScript union member.
fn variant_text(v: &syn::Variant) -> String {
    match &v.fields {
        syn::Fields::Unit => v.ident.to_string(),
        fields => {
            let payload = fields
                .iter()
                .map(|f| normalise_tokens(&f.ty.to_token_stream().to_string()))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({payload})", v.ident)
        }
    }
}

fn prop(ident: &syn::Ident, field: &syn::Field, file: &str) -> Prop {
    Prop {
        name: ident.to_string(),
        rust_ty: normalise_tokens(&field.ty.to_token_stream().to_string()),
        // Placeholder: `classify.rs` rewrites every `kind` from `rust_ty`, and
        // fails rather than guessing. Nothing may read `kind` before that stage.
        kind: PropKind::Text,
        optional: option_inner(&field.ty).is_some(),
        default: field_default(&field.attrs),
        span: Span { file: file.to_string(), line: ident.span().start().line },
    }
}

// ---- attribute reading -----------------------------------------------------

/// The component name from `#[function_component(Name)]`, or from bare
/// `#[function_component]` / `#[component]`, where yew takes the function's own
/// identifier (which must then already be PascalCase).
fn component_name(attrs: &[syn::Attribute], fn_ident: &syn::Ident) -> Option<String> {
    for attr in attrs {
        let last = attr.path().segments.last()?.ident.to_string();
        if last != "function_component" && last != "component" {
            continue;
        }
        return match &attr.meta {
            syn::Meta::Path(_) => Some(fn_ident.to_string()),
            syn::Meta::List(list) => {
                // The argument is a bare ident; `parse_args` keeps this working
                // if yew ever accepts a path such as `crate::Button`.
                match list.parse_args::<syn::Path>() {
                    Ok(path) => path.segments.last().map(|s| s.ident.to_string()),
                    Err(_) => Some(fn_ident.to_string()),
                }
            }
            syn::Meta::NameValue(_) => Some(fn_ident.to_string()),
        };
    }
    None
}

/// `&FooProps` -> `Some("FooProps")`; no argument -> `None`. Also accepts an
/// owned or lifetime-annotated argument, so the shape of the borrow is not part
/// of the contract.
fn props_arg_ty(sig: &syn::Signature) -> Option<String> {
    let arg = sig.inputs.first()?;
    let syn::FnArg::Typed(pat) = arg else { return None };
    path_name(strip_refs(&pat.ty))
}

fn strip_refs(ty: &syn::Type) -> &syn::Type {
    match ty {
        syn::Type::Reference(r) => strip_refs(&r.elem),
        syn::Type::Paren(p) => strip_refs(&p.elem),
        syn::Type::Group(g) => strip_refs(&g.elem),
        other => other,
    }
}

/// The last path segment of a path type, e.g. `crate::atoms::ButtonProps` ->
/// `ButtonProps`.
fn path_name(ty: &syn::Type) -> Option<String> {
    match ty {
        syn::Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// The `T` of `Option<T>`, or `None` for any other type.
fn option_inner(ty: &syn::Type) -> Option<&syn::Type> {
    let syn::Type::Path(p) = strip_refs(ty) else { return None };
    let last = p.path.segments.last()?;
    if last.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else { return None };
    args.args.iter().find_map(|a| match a {
        syn::GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

fn derives(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if let Some(last) = meta.path.segments.last() {
                out.push(last.ident.to_string());
            }
            Ok(())
        });
    }
    out
}

fn field_default(attrs: &[syn::Attribute]) -> PropDefault {
    for attr in attrs {
        let Some(last) = attr.path().segments.last() else { continue };
        match last.ident.to_string().as_str() {
            "prop_or_default" => return PropDefault::DefaultTrait,
            "prop_or" | "prop_or_else" => {
                let syn::Meta::List(list) = &attr.meta else { continue };
                let tokens = list.tokens.to_string();
                let ts = syn::parse2::<syn::Expr>(list.tokens.clone())
                    .ok()
                    .as_ref()
                    .and_then(ts_literal);
                return PropDefault::Expr { expr: normalise_tokens(&tokens), ts };
            }
            _ => {}
        }
    }
    PropDefault::Required
}

/// The TypeScript literal for a `#[prop_or(...)]` expression, when the
/// expression is a constant this stage can evaluate without running rustc.
/// Enum paths (`ButtonVariant::Primary`) deliberately return `None`: mapping a
/// variant onto its string union member is `classify.rs`'s job, not a guess
/// made from an unresolved path.
fn ts_literal(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Lit(lit) => match &lit.lit {
            syn::Lit::Str(s) => serde_json::to_string(&s.value()).ok(),
            syn::Lit::Bool(b) => Some(b.value.to_string()),
            syn::Lit::Int(i) => Some(i.base10_digits().to_string()),
            syn::Lit::Float(f) => Some(f.base10_digits().to_string()),
            syn::Lit::Char(c) => serde_json::to_string(&c.value().to_string()).ok(),
            _ => None,
        },
        syn::Expr::Paren(p) => ts_literal(&p.expr),
        syn::Expr::Group(g) => ts_literal(&g.expr),
        // `AttrValue::Static("rust")` (src/atoms/code_block.rs:16) and
        // `AttrValue::from(..)` wrap a literal without changing its value.
        // `AttrValue::Static(STORAGE_KEY)`
        // (src/interactive/atoms/theme_toggle.rs:24) is a const path, not a
        // literal, and correctly falls through to `None`.
        syn::Expr::Call(call) => {
            let syn::Expr::Path(callee) = call.func.as_ref() else { return None };
            let name = callee.path.segments.last()?.ident.to_string();
            if !matches!(name.as_str(), "Static" | "from" | "new") || call.args.len() != 1 {
                return None;
            }
            ts_literal(call.args.first()?)
        }
        syn::Expr::MethodCall(call) => {
            if !matches!(call.method.to_string().as_str(), "into" | "to_string" | "to_owned") {
                return None;
            }
            ts_literal(&call.receiver)
        }
        _ => None,
    }
}

/// A token stream printed with the spacing a human would write:
/// `Vec < AttrValue >` -> `Vec<AttrValue>`, `& 'static str` -> `&'static str`,
/// `AttrValue :: Static ("rust")` -> `AttrValue::Static("rust")`. Used for both
/// `rust_ty` and `#[prop_or(...)]` expression text, which land in the ABI and in
/// diagnostics a human reads.
fn normalise_tokens(raw: &str) -> String {
    let mut s = raw.to_string();
    // `> ` is deliberately absent: it would corrupt a fn-pointer arrow
    // (`-> Html`), and a trailing space is trimmed anyway.
    for (from, to) in [
        (" <", "<"),
        ("< ", "<"),
        (" >", ">"),
        (" ::", "::"),
        (":: ", "::"),
        (" ,", ","),
        ("& ", "&"),
        ("' ", "'"),
        (" (", "("),
        ("( ", "("),
        (" )", ")"),
        ("[ ", "["),
        (" ]", "]"),
        ("  ", " "),
    ] {
        while s.contains(from) {
            s = s.replace(from, to);
        }
    }
    s.trim().to_string()
}

// ---- linking ---------------------------------------------------------------

/// Every type identifier appearing anywhere in a field's type, including inside
/// generic arguments and tuples, so `Vec<PropSpec>` and `Option<NavLink>` both
/// resolve. The type is re-parsed from `rust_ty` — it was written by `quote!`
/// from a real `syn::Type`, so a failure here means `normalise_tokens` corrupted
/// it and must be reported, never swallowed into a missing matrix axis.
fn field_ty_idents(field: &Prop, out: &mut Vec<String>) -> Result<()> {
    let ty = syn::parse_str::<syn::Type>(&field.rust_ty).with_context(|| {
        format!(
            "{}:{}: cannot re-parse the type of `{}` (`{}`)",
            field.span.file, field.span.line, field.name, field.rust_ty
        )
    })?;
    ty_idents(&ty, out);
    Ok(())
}

fn ty_idents(ty: &syn::Type, out: &mut Vec<String>) {
    struct Idents<'a>(&'a mut Vec<String>);
    impl<'ast> Visit<'ast> for Idents<'_> {
        fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
            for segment in &node.path.segments {
                self.0.push(segment.ident.to_string());
            }
            syn::visit::visit_type_path(self, node);
        }
    }
    Idents(out).visit_type(ty);
}

/// Resolve a name against a pool, rejecting an ambiguous hit rather than picking
/// one silently. The crate has four private structs called `Layer`
/// (e.g. `src/organisms/dataspace_layers.rs:134` and
/// `src/organisms/ids_ram_layers.rs:88`); none is reachable from a prop today,
/// but if one became reachable the generator must say so instead of guessing.
fn resolve<'a, T>(
    name: &str,
    pool: &'a [T],
    key: impl Fn(&T) -> (&str, &Span),
) -> Result<Option<&'a T>> {
    let mut hits = pool.iter().filter(|item| key(item).0 == name);
    let Some(first) = hits.next() else { return Ok(None) };
    if let Some(second) = hits.next() {
        let (_, a) = key(first);
        let (_, b) = key(second);
        bail!(
            "`{name}` is defined twice and is reachable from a prop: {}:{} and {}:{}; \
             rename one or teach parse.rs module paths",
            a.file,
            a.line,
            b.file,
            b.line
        );
    }
    Ok(Some(first))
}

/// Expand every data-carrying variant against the unit enums, so that each entry
/// in the returned pool names one *constructible* value.
///
/// `BadgeVariant::Kind(TermKind)` (`src/atoms/onto_badge.rs:21`) becomes eight
/// entries, `Kind(TermKind::Ontology)` .. `Kind(TermKind::Other)`, alongside the
/// four unit variants — the twelve inhabitants the Vue source's union declares
/// (`onto_badge.rs:17`). This is what keeps the three stages agreeing: the TS
/// union `classify.rs` derives, the axis values `matrix.rs` enumerates and the
/// expression the harness has to write are all the same twelve, and each one is
/// a path that compiles. Recording the bare `Kind` instead would give
/// `classify.rs` twelve union members but `matrix.rs` only five cells, so seven
/// of the renderings at `onto_badge.rs:40-46` would never be rendered and the
/// emitter would hard-code one of them.
///
/// A payload that is not itself a unit enum in this crate is left verbatim; it
/// is not this stage's job to guess at a type it cannot see.
fn expand_data_enums(unit: Vec<PropEnum>, data: Vec<PropEnum>) -> Vec<PropEnum> {
    let mut out = unit;
    for mut e in data {
        e.variants = e
            .variants
            .iter()
            .flat_map(|v| expand_variant(v, &out))
            .collect();
        // Expansion rewrites `variants`; a `#[default]` that no longer names one
        // of them would be a dangling reference by the time `emit` reads it.
        // Unreachable today (see `default_variant`), which is why it is checked
        // rather than assumed.
        if let Some(d) = &e.default_variant {
            if !e.variants.iter().any(|v| v == d) {
                e.default_variant = None;
            }
        }
        out.push(e);
    }
    out
}

fn expand_variant(variant: &str, unit: &[PropEnum]) -> Vec<String> {
    let Some(open) = variant.find('(') else { return vec![variant.to_string()] };
    let head = &variant[..open];
    let payload = variant[open + 1..].trim_end_matches(')').trim();
    match unit.iter().find(|e| e.name == payload) {
        Some(inner) => inner
            .variants
            .iter()
            .map(|iv| format!("{head}({payload}::{iv})"))
            .collect(),
        None => vec![variant.to_string()],
    }
}

fn link(collected: Collected) -> Result<Toolkit> {
    let Collected { mut components, props_structs, enums, data_enums, plain_structs, src_hash } =
        collected;

    // A data-carrying enum is expanded into its constructible inhabitants and
    // then joins the same pool as the unit enums. Withholding it entirely is not
    // an option: `OntoBadge` takes `BadgeVariant` (`src/atoms/onto_badge.rs:60`)
    // and is one of the components in scope, so a missing entry here is not a
    // loud failure downstream but a component that cannot be classified at all.
    let enums = expand_data_enums(enums, data_enums);

    // Attach each component's props, and keep the reachability frontier as we go.
    let mut frontier: Vec<String> = Vec::new();
    for component in &mut components {
        let Some(props_ty) = component.props_ty.clone() else { continue };
        match resolve(&props_ty, &props_structs, |p| (p.name.as_str(), &p.span))? {
            Some(found) => {
                for field in &found.fields {
                    field_ty_idents(field, &mut frontier)?;
                }
                component.props = found.fields.clone();
            }
            // Recorded, not dropped: the component stays in the ABI with a
            // reason a human can open.
            None => {
                component.status = Status::NeedsOverride {
                    reason: format!(
                        "props type `{props_ty}` is not defined in src/**/*.rs ({}:{})",
                        component.span.file, component.span.line
                    ),
                }
            }
        }
    }

    // Transitive closure over plain structs and enums. `PropSpec`
    // (src/molecules/props_table.rs:6) and `LiteralValue` (src/ontology.rs:39)
    // arrive here because their fields are matrix axes too:
    // `props_table.rs:59` branches on `PropSpec.default` and
    // `annotation.rs:33` matches three ways on `LiteralValue.language`/`.datatype`.
    let mut kept_structs: BTreeMap<String, PlainStruct> = BTreeMap::new();
    let mut kept_enums: BTreeMap<String, PropEnum> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    while let Some(name) = frontier.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        if let Some(found) = resolve(&name, &plain_structs, |s| (s.name.as_str(), &s.span))? {
            for field in &found.fields {
                field_ty_idents(field, &mut frontier)?;
            }
            kept_structs.insert(name.clone(), found.clone());
            continue;
        }
        if let Some(found) = resolve(&name, &enums, |e| (e.name.as_str(), &e.span))? {
            kept_enums.insert(name.clone(), found.clone());
        }
    }

    // Deterministic order: components by source position, types by name.
    components.sort_by(|a, b| {
        (&a.span.file, a.span.line, &a.name).cmp(&(&b.span.file, b.span.line, &b.name))
    });

    Ok(Toolkit {
        components,
        enums: kept_enums.into_values().collect(),
        structs: kept_structs.into_values().collect(),
        src_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The crate under test is the xtask's own parent directory.
    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
    }

    #[test]
    fn counts_match_the_crate() {
        let collected = collect(&root()).unwrap();
        assert_eq!(collected.components.len(), 42, "every #[function_component] in src/**/*.rs");
        assert_eq!(collected.props_structs.len(), 39, "every #[derive(Properties)] struct");

        let toolkit = parse(&root()).unwrap();
        assert_eq!(toolkit.components.len(), 42);
        assert_eq!(
            toolkit.components.iter().filter(|c| c.tier == Tier::Interactive).count(),
            0,
            "src/interactive/ moved to eona-vocabulary-ui; the tier is a guard, not a population"
        );
        assert_eq!(
            toolkit.components.iter().filter(|c| c.props_ty.is_none()).count(),
            3,
            "ComponentsGallery, IntegrationSection, OntologySection — the demo page's own sections"
        );
        assert_eq!(
            toolkit
                .components
                .iter()
                .filter_map(|c| c.props_ty.clone())
                .collect::<BTreeSet<_>>()
                .len(),
            39,
            "each Properties struct backs exactly one component"
        );
        assert!(toolkit.components.iter().all(|c| matches!(c.status, Status::Ok)));
    }

    #[test]
    fn button_props_are_read_verbatim() {
        let toolkit = parse(&root()).unwrap();
        let button = toolkit.component("Button").expect("Button");
        assert_eq!(button.span.file, "src/atoms/button.rs");
        assert_eq!(button.props_ty.as_deref(), Some("ButtonProps"));
        assert_eq!(button.tier, Tier::Presentational);

        let label = &button.props[0];
        assert_eq!((label.name.as_str(), label.rust_ty.as_str()), ("label", "AttrValue"));
        assert_eq!(label.default, PropDefault::Required);
        assert!(!label.optional);

        let variant = &button.props[1];
        assert_eq!(variant.rust_ty, "ButtonVariant");
        assert_eq!(
            variant.default,
            PropDefault::Expr { expr: "ButtonVariant::Primary".into(), ts: None }
        );

        let href = &button.props[2];
        assert_eq!(href.rust_ty, "Option<AttrValue>");
        assert!(href.optional);
        assert_eq!(href.default, PropDefault::DefaultTrait);
    }

    #[test]
    fn nested_axes_are_reachable() {
        let toolkit = parse(&root()).unwrap();

        // src/molecules/props_table.rs:59 branches on this field.
        let spec = toolkit.plain_struct("PropSpec").expect("PropSpec");
        let default = spec.fields.iter().find(|f| f.name == "default").unwrap();
        assert!(default.optional, "{}:{}", default.span.file, default.span.line);

        // src/molecules/annotation.rs:33-41 matches three ways on these two.
        let literal = toolkit.plain_struct("LiteralValue").expect("LiteralValue");
        assert!(literal.fields.iter().filter(|f| f.optional).count() >= 2);

        // Reached only through Vec<ColorSpec> / Option<NavLink>, i.e. through
        // generic arguments, not a bare path.
        assert!(toolkit.plain_struct("ColorSpec").is_some());
        assert!(toolkit.plain_struct("NavLink").is_some());
        assert!(toolkit.unit_enum("ButtonVariant").is_some());
        // `TermKind` is deliberately *not* here. Nothing in this crate takes it
        // as a prop any more — `Term.kind` left with the interactive tier — so
        // `link`'s reachability frontier never names it. It still reaches
        // `OntoBadge.tsx`, but through `expand_data_enums`, which bakes its
        // variants into `BadgeVariant` before the frontier is walked. See
        // `data_carrying_enums_expand_rather_than_flatten` below.
        assert!(toolkit.unit_enum("TermKind").is_none(), "reachable only through BadgeVariant");
    }

    /// `BadgeVariant` carries a `TermKind` (`src/atoms/onto_badge.rs:21`), so
    /// recording the bare variant name would collapse its eight `Kind(..)`
    /// renderings (`onto_badge.rs:40-46`) into one union member. `collect` keeps
    /// the payload and `link` expands it into the twelve constructible
    /// inhabitants, which is what lets `classify` and `matrix` agree on the
    /// same twelve.
    #[test]
    fn data_carrying_enums_expand_rather_than_flatten() {
        let collected = collect(&root()).unwrap();
        assert_eq!(
            collected.data_enums.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["BadgeVariant"],
            "the crate's only data-carrying enum"
        );
        assert!(
            collected.data_enums[0].variants.contains(&"Kind(TermKind)".to_string()),
            "the payload type survives collect: {:?}",
            collected.data_enums[0].variants
        );

        let toolkit = parse(&root()).unwrap();
        let badge = toolkit.unit_enum("BadgeVariant").expect("reached via OntoBadgeProps.variant");
        assert_eq!(badge.variants.len(), 12, "8 TermKind kinds + Lang/Datatype/Deprecated/Default");
        // Every entry names a value that actually compiles, because the harness
        // has to write it as a literal expression.
        assert!(badge.variants.contains(&"Kind(TermKind::ObjectProperty)".to_string()));
        assert!(badge.variants.contains(&"Deprecated".to_string()));
        assert!(
            !badge.variants.iter().any(|v| v == "Kind" || v == "Kind(TermKind)"),
            "no unconstructible entry survives expansion: {:?}",
            badge.variants
        );

        let variant = toolkit
            .component("OntoBadge")
            .unwrap()
            .props
            .iter()
            .find(|p| p.name == "variant")
            .unwrap();
        assert_eq!(variant.rust_ty, "BadgeVariant");
        assert_eq!(variant.span.file, "src/atoms/onto_badge.rs");
    }

    #[test]
    fn string_literal_defaults_evaluate() {
        let toolkit = parse(&root()).unwrap();
        let code_block = toolkit.component("CodeBlock").unwrap();
        let language = code_block.props.iter().find(|p| p.name == "language").unwrap();
        assert_eq!(
            language.default,
            PropDefault::Expr {
                expr: "AttrValue::Static(\"rust\")".into(),
                ts: Some("\"rust\"".into()),
            }
        );
        let copyable = code_block.props.iter().find(|p| p.name == "copyable").unwrap();
        assert_eq!(
            copyable.default,
            PropDefault::Expr { expr: "true".into(), ts: Some("true".into()) }
        );
    }

    #[test]
    fn hash_is_stable_and_content_addressed() {
        let a = collect(&root()).unwrap().src_hash;
        let b = collect(&root()).unwrap().src_hash;
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }
}
