//! Stage: `examples`. See xtask/README.md for the pipeline contract.
//!
//! The components page has to show every component twice — the Yew `html!` call
//! and the React JSX that does the same thing. Writing the second one by hand
//! would reintroduce, in two panes on one page, exactly the drift this generator
//! exists to remove: the Yew half is checked against `src/` by the compiler, and
//! a hand-written React half is checked by nobody.
//!
//! So both halves are derived here, from one input each run: `abi::Abi` supplies
//! the member list, the TS type, optionality and the default — the document that
//! *is* the React package's public contract — and [`crate::ir::Toolkit`] supplies
//! the `PropKind` and the Rust variant names, which the ABI deliberately does not
//! carry (it records the TS union, not the Rust enum). Both are built in the same
//! pipeline run from the same `src_hash`, so they cannot describe different
//! crates. A prop added in Rust therefore appears in both snippets on the next
//! `bridgegen`, with no second place to update.
//!
//! # Why the props table is not duplicated here
//!
//! `abi/toolkit.abi.json` already lists every prop's name, Rust type, TS type,
//! optionality and default. Restating those rows in this document would give one
//! fact two committed spellings that could disagree — the mistake
//! `abi::AbiDefault` exists to undo. The docs page reads the snippets from here
//! and the prop rows from the ABI.
//!
//! # Two output files, and why both
//!
//! - `abi/examples.json` is the reviewable half. Sorted, pretty-printed, no
//!   timestamps, diffed byte-for-byte by `check` exactly like the ABI.
//! - `bridge/examples.rs` is the consumable half: plain `&'static str` tables the
//!   toolkit crate can pull in with `#[path = "../bridge/examples.rs"] mod`. It
//!   exists because the toolkit's whole dependency set is `yew` — after the
//!   ontology parser left, `serde_json` did too (`../Cargo.toml`), so there is
//!   nothing in the crate that could read the JSON at runtime, and no `build.rs`
//!   left to read it at build time either.
//!
//! `bridge/`, not `src/`: `parse::source_files` (xtask/src/parse.rs) hashes
//! `src/**/*.rs` into `src_hash`, and a generated file inside `src/` would be an
//! input to the hash of the document that describes it — `check` would need two
//! passes to converge. `abi/` holds JSON and TOML that `check` diffs; a `.rs`
//! file the toolkit compiles is a different kind of thing and gets its own
//! directory.
//!
//! # What the snippets do *not* try to be
//!
//! Real copy. Values come from [`PLACEHOLDERS`], a closed table keyed on prop
//! (or field, or tuple-element) name, falling back to the name itself in title
//! case. The snippet documents the *shape* of the call — which props exist, how
//! each is spelled in each stack — and the table exists only for the handful of
//! names where a title-cased name would be actively wrong: `href="Href"` is not
//! a link and `hex="Hex"` is not a colour.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::abi::{Abi, AbiComponent, AbiDefault, AbiMember};
use crate::ir::{PlainStruct, Prop, PropDefault, PropKind, Span, Status, Toolkit};
use crate::splice::{js_prop_name, variant_literal};

/// Bumped when the *shape* of `abi/examples.json` changes, so a stale file fails
/// to deserialise loudly instead of diffing as if every component moved at once.
/// Same contract as [`crate::abi::ABI_VERSION`].
pub const EXAMPLES_VERSION: u32 = 1;

/// The reviewable artifact, relative to the toolkit root.
pub const EXAMPLES_JSON: &str = "abi/examples.json";
/// The consumable artifact: zero-dependency Rust the toolkit can `#[path]`-mod.
pub const EXAMPLES_RS: &str = "bridge/examples.rs";

/// The npm package name a React snippet imports from. Same constant `emit` puts
/// in `package.json`, so the import line in a snippet cannot name a package the
/// generator does not publish.
use crate::emit::PACKAGE_NAME;

/// The Rust crate a Yew snippet imports from — the toolkit's own `package.name`
/// with `-` as `_`, which is how a `use` statement spells it.
const CRATE_NAME: &str = "eona_ui_toolkit";

/// How wide a single-line call may be before it is broken one attribute per
/// line. Matches the hand-written snippets in the demo gallery, which wrap
/// `NewsCard` (3 attributes, one of them a sentence) and leave `MemberCard`
/// (1 attribute) alone.
const WRAP_AT: usize = 76;

// ---------------------------------------------------------------- the document

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Examples {
    pub examples_version: u32,
    /// `Toolkit::src_hash`. The generator's own hash is deliberately not stored
    /// a second time: it is in `abi/toolkit.abi.json`, this document is derived
    /// from that one, and `check` regenerates and byte-diffs both on every run.
    pub src_hash: String,
    /// Sorted by `component`. Total: every component the crate defines appears,
    /// including the ones excluded by rule, for the reason `abi.rs` gives — a
    /// component must not be able to leave a generated artifact by being absent
    /// from a list nobody counts.
    pub components: Vec<Example>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Example {
    pub component: String,
    /// `file:line` of the `#[function_component]`, openable as-is.
    pub source: String,
    /// `ir::Status` verbatim, so a page can render the snippet and its fate
    /// without joining against the ABI.
    #[serde(flatten)]
    pub status: Status,
    pub yew: YewSnippet,
    #[serde(flatten)]
    pub react: ReactSnippet,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YewSnippet {
    /// The `use` lines the snippet needs, newline-separated. Enum and struct
    /// prop types are resolved through `src/lib.rs`'s re-exports: everything
    /// under `src/atoms`, `src/molecules` and `src/organisms` is at the crate
    /// root, and anything else keeps its module path (`ontology::TermKind`).
    pub uses: String,
    pub snippet: String,
}

/// Whether the component exists in `@eona-x/ui-toolkit-react`, and if not, the
/// honest note to print in place of a snippet.
///
/// Five components are in the crate and not in the package — `Modal` needs an
/// override, `Swatch`, `PaletteGroup`, `DatasetCard` and `OntoAnnotation` are
/// quarantined — and a docs page that showed them a React snippet anyway would
/// be handing a reader an import that does not resolve.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "react", rename_all = "kebab-case")]
pub enum ReactSnippet {
    Available {
        /// The import the JSX needs, naming only what `index.ts` exports.
        import: String,
        jsx: String,
    },
    Unavailable {
        /// The generator's own rule, in its own words.
        why: String,
        /// The human decision from `abi/overrides.toml`; empty for a component
        /// excluded by rule, which needs no override entry
        /// (`check::check_overrides`).
        disposition: String,
        /// `tracked_by` from the same entry, when there is one.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        tracked_by: String,
    },
}

impl Examples {
    pub fn get(&self, component: &str) -> Option<&Example> {
        self.components.iter().find(|e| e.component == component)
    }
}

// ---------------------------------------------------------------- building

/// Build the document. `overrides` is `check::parse_overrides`' map, so the
/// disposition a reader sees is the one a human wrote rather than a second
/// paraphrase of the machine reason.
pub fn build(
    abi: &Abi,
    tk: &Toolkit,
    overrides: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<Examples> {
    let mut components = Vec::with_capacity(abi.components.len());
    for c in &abi.components {
        components.push(example(c, tk, overrides)?);
    }
    // `Abi::components` is already sorted by name; sorting again is the cheap
    // way to make this document's ordering its own property rather than an
    // inherited one.
    components.sort_by(|a, b| a.component.cmp(&b.component));
    Ok(Examples { examples_version: EXAMPLES_VERSION, src_hash: abi.src_hash.clone(), components })
}

fn example(
    c: &AbiComponent,
    tk: &Toolkit,
    overrides: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<Example> {
    let Some(ir) = tk.component(&c.name) else {
        bail!(
            "the ABI lists {} ({}) but the IR does not; the two documents came from \
             different runs",
            c.name,
            c.source
        );
    };
    let call = Call::build(c, tk)?;
    let react = match &c.status {
        Status::Ok => ReactSnippet::Available {
            import: format!("import {{ {} }} from '{PACKAGE_NAME}';", c.name),
            jsx: call.jsx(&c.name),
        },
        other => {
            let entry = overrides.get(&c.name);
            ReactSnippet::Unavailable {
                why: why(other),
                disposition: entry
                    .and_then(|e| e.get("disposition"))
                    .cloned()
                    .unwrap_or_default(),
                tracked_by: entry.and_then(|e| e.get("tracked_by")).cloned().unwrap_or_default(),
            }
        }
    };
    Ok(Example {
        component: c.name.clone(),
        source: c.source.clone(),
        status: c.status.clone(),
        yew: YewSnippet {
            uses: call.uses(&c.name, &ir.span, tk)?,
            snippet: call.html(&c.name),
        },
        react,
    })
}

/// The React half's note, phrased for a reader of the page rather than for the
/// gate: `Status`'s own reason is the machine's sentence, and it is quoted
/// rather than rewritten so the note and `abi/toolkit.abi.json` cannot diverge.
fn why(status: &Status) -> String {
    match status {
        Status::Ok => String::new(),
        Status::NeedsOverride { reason } => {
            format!("not in the React package — it needs an override: {reason}")
        }
        Status::Quarantined { reason } => {
            format!("not in the React package — the verify gate quarantined it: {reason}")
        }
        Status::Excluded { reason } => {
            format!("never generated — out of scope by rule: {reason}")
        }
    }
}

// ---------------------------------------------------------------- the call

/// One component's call, in both spellings, before it is laid out.
///
/// Built once and rendered twice so the two panes are the same set of props by
/// construction: a prop can only reach one stack's snippet by reaching this
/// struct, which appends to both vectors in the same step.
#[derive(Debug, Default)]
struct Call {
    yew_attrs: Vec<String>,
    jsx_attrs: Vec<String>,
    /// The component takes a `children` slot, so the call has an element body.
    children: bool,
    /// Rust types the Yew snippet names, for the `use` lines.
    rust_types: BTreeSet<String>,
    /// The Yew snippet spells `html! { .. }` for a named slot.
    needs_html_macro: bool,
}

impl Call {
    fn build(c: &AbiComponent, tk: &Toolkit) -> Result<Self> {
        let mut call = Call::default();
        let ir = tk.component(&c.name);
        for m in &c.props {
            // The ABI is the member list; the IR is the only place the
            // classified shape lives, and without it a value would have to be
            // guessed from the Rust type text — i.e. `classify` reimplemented.
            let Some(p) = ir.and_then(|ir| ir.props.iter().find(|p| p.name == m.name)) else {
                bail!(
                    "{}: the ABI lists prop `{}` ({}) that the IR does not, so its kind is \
                     unknown; the two documents came from different runs",
                    c.name,
                    m.name,
                    m.source
                );
            };
            call.push(m, p, tk)?;
        }
        Ok(call)
    }

    fn push(&mut self, m: &AbiMember, p: &Prop, tk: &Toolkit) -> Result<()> {
        // A `children` slot is the element body in both stacks, never an
        // attribute. `Html`/`Children` props under any other name stay
        // attributes: React takes them as `ReactNode` props (`ShapeCard.demo`,
        // `TypeScaleRow.sample`).
        if matches!(p.kind, PropKind::Slot) && p.name == "children" {
            self.children = true;
            return Ok(());
        }
        // `Option<T>` props are written exactly like `T`: yew's blanket
        // `impl<T> IntoPropValue<Option<T>> for T` accepts the inner value in
        // attribute position, which is how the crate's own callers already
        // spell them. Struct *fields* get no such help — see `struct_literal`.
        let js = js_prop_name(&p.name);
        let (yew, jsx) = match &p.kind {
            PropKind::Text => {
                let text = text_for(&p.name, &m.default);
                (format!("{}={}", p.name, yew_text(&text)), format!("{js}={}", jsx_text(&text)))
            }
            PropKind::Bool => {
                let v = shown_bool(&m.default);
                // A bare JSX attribute is `true`; `false` has to be written out.
                let jsx = if v { js.clone() } else { format!("{js}={{false}}") };
                (format!("{}={{{v}}}", p.name), jsx)
            }
            PropKind::Num { .. } => (format!("{}={{1}}", p.name), format!("{js}={{1}}")),
            PropKind::UnitEnum { name } => {
                let variant = shown_variant(name, &p.default, tk)?;
                self.rust_types.insert(name.clone());
                self.rust_types.extend(payload_types(&variant));
                (
                    format!("{}={{{name}::{variant}}}", p.name),
                    format!("{js}=\"{}\"", variant_literal(&variant)),
                )
            }
            PropKind::List { item } => {
                let v = self.value(item, vec_item_ty(&p.rust_ty), &singular(&p.name), tk)?;
                (
                    format!("{}={{vec![{}]}}", p.name, v.yew),
                    format!("{js}={{[{}]}}", v.jsx),
                )
            }
            PropKind::Tuple { .. } | PropKind::Struct { .. } => {
                let v = self.value(&p.kind, &p.rust_ty, &p.name, tk)?;
                (format!("{}={{{}}}", p.name, v.yew), format!("{js}={{{}}}", v.jsx))
            }
            PropKind::Slot => {
                self.needs_html_macro = true;
                (
                    format!("{}={{html! {{ <span>{{ \"Slot content.\" }}</span> }}}}", p.name),
                    format!("{js}={{<span>Slot content.</span>}}"),
                )
            }
            // No component in scope has one — a `Callback` prop forces
            // `NeedsOverride`, so the React half of such a component is a note
            // rather than a snippet and this value is only ever read by the Yew
            // pane. `Callback::noop()` is the one spelling that is correct for
            // every argument type.
            PropKind::Callback { .. } => (
                format!("{}={{Callback::noop()}}", p.name),
                format!("{js}={{() => {{}}}}"),
            ),
        };
        self.yew_attrs.push(yew);
        self.jsx_attrs.push(jsx);
        Ok(())
    }

    /// A value in expression position — the element of a list, a tuple, a struct
    /// literal. `path` is the name the placeholder table is keyed on, `rust_ty`
    /// the Rust type verbatim, which is what decides whether a string literal
    /// needs `.into()`.
    fn value(&mut self, kind: &PropKind, rust_ty: &str, path: &str, tk: &Toolkit) -> Result<Value> {
        Ok(match kind {
            PropKind::Text => {
                let text = text_for(path, &AbiDefault::Required);
                // Outside attribute position there is no `IntoPropValue` to lean
                // on, so `AttrValue`, `String` and `Cow` need the conversion
                // written out and `&'static str` must not have it — `.into()` on
                // a `&str` field is noise a reader would copy into their own
                // code. Unknown types take `.into()`, which is the identity for
                // `&'static str` and so is never wrong, only verbose.
                let lit = yew_literal(&text);
                let yew =
                    if is_str_slice(rust_ty) { lit } else { format!("{lit}.into()") };
                Value { yew, jsx: jsx_text(&text) }
            }
            PropKind::Bool => Value { yew: "false".into(), jsx: "false".into() },
            PropKind::Num { .. } => Value { yew: "1".into(), jsx: "1".into() },
            PropKind::UnitEnum { name } => {
                let variant = shown_variant(name, &PropDefault::Required, tk)?;
                self.rust_types.insert(name.clone());
                self.rust_types.extend(payload_types(&variant));
                Value {
                    yew: format!("{name}::{variant}"),
                    jsx: format!("\"{}\"", variant_literal(&variant)),
                }
            }
            PropKind::List { item } => {
                let v = self.value(item, vec_item_ty(rust_ty), &singular(path), tk)?;
                Value { yew: format!("vec![{}]", v.yew), jsx: format!("[{}]", v.jsx) }
            }
            PropKind::Tuple { items } => {
                let element_tys = tuple_element_tys(rust_ty);
                let mut yew = Vec::new();
                let mut jsx = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    let ty = element_tys.get(i).copied().unwrap_or("");
                    let v = self.value(item, ty, &format!("{path}.{i}"), tk)?;
                    yew.push(v.yew);
                    jsx.push(v.jsx);
                }
                Value {
                    yew: format!("({})", yew.join(", ")),
                    jsx: format!("[{}]", jsx.join(", ")),
                }
            }
            PropKind::Struct { name } => {
                let Some(s) = tk.plain_struct(name) else {
                    bail!("struct `{name}` is a prop type but `parse` did not register it");
                };
                self.rust_types.insert(name.clone());
                self.struct_literal(s, tk)?
            }
            // Neither can appear nested: `classify` only ever produces `Slot` and
            // `Callback` at the top level of a prop (xtask/src/classify.rs), so
            // reaching here means the IR grew a shape this stage has not been
            // taught, and guessing would put a lie on the docs page.
            PropKind::Slot | PropKind::Callback { .. } => bail!(
                "a `{}` cannot be written as a literal value, and one appeared nested at `{path}`",
                if matches!(kind, PropKind::Slot) { "slot" } else { "callback" }
            ),
        })
    }

    fn struct_literal(&mut self, s: &PlainStruct, tk: &Toolkit) -> Result<Value> {
        let mut yew = Vec::new();
        let mut jsx = Vec::new();
        for f in &s.fields {
            // A struct field is plain Rust, not a Yew prop: there is no
            // `IntoPropValue` here, so `Option<T>` has to be written out. `None`
            // rather than a `Some(..)` placeholder, because a value invented for
            // an optional field reads as a requirement.
            if f.optional {
                yew.push(format!("{}: None", f.name));
                continue;
            }
            let v = self.value(&f.kind, &f.rust_ty, &f.name, tk)?;
            yew.push(format!("{}: {}", f.name, v.yew));
            jsx.push(format!("{}: {}", js_prop_name(&f.name), v.jsx));
        }
        Ok(Value {
            yew: format!("{} {{ {} }}", s.name, yew.join(", ")),
            jsx: format!("{{ {} }}", jsx.join(", ")),
        })
    }

    /// `<Name a b />`, wrapped one attribute per line when it would not fit.
    fn html(&self, name: &str) -> String {
        layout(name, &self.yew_attrs, self.children.then_some("{ \"Children go here.\" }"), 4)
    }

    fn jsx(&self, name: &str) -> String {
        layout(name, &self.jsx_attrs, self.children.then_some("Children go here."), 2)
    }

    /// The `use` lines, one per module path, sorted.
    ///
    /// The component is resolved through [`module_path`] like every other name
    /// rather than assumed to be at the crate root: `DemoPage` lives in
    /// `src/demo.rs`, which `src/lib.rs` declares as a `pub mod` and does *not*
    /// glob, so `eona_ui_toolkit::DemoPage` does not resolve.
    fn uses(&self, name: &str, span: &Span, tk: &Toolkit) -> Result<String> {
        let mut by_module: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        by_module.entry(module_path(span)).or_default().insert(name.to_string());
        for ty in &self.rust_types {
            let span = type_span(ty, tk).with_context(|| {
                format!("`{ty}` is named by {name}'s snippet but is not a type `parse` registered")
            })?;
            by_module.entry(module_path(span)).or_default().insert(ty.clone());
        }
        let mut lines: Vec<String> = by_module
            .into_iter()
            .map(|(module, names)| {
                let names: Vec<String> = names.into_iter().collect();
                if names.len() == 1 {
                    format!("use {module}::{};", names[0])
                } else {
                    format!("use {module}::{{{}}};", names.join(", "))
                }
            })
            .collect();
        if self.needs_html_macro {
            lines.push("use yew::prelude::*;".into());
        }
        Ok(lines.join("\n"))
    }
}

struct Value {
    yew: String,
    jsx: String,
}

/// `<Name attrs />` on one line, or broken one attribute per line.
///
/// `indent` differs per stack on purpose: `../src` is 4-space Rust and
/// `packages/react` is 2-space TSX, and a snippet that a reader pastes should
/// match the file it lands in.
fn layout(name: &str, attrs: &[String], child: Option<&str>, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let head = if attrs.is_empty() {
        format!("<{name}")
    } else {
        format!("<{name} {}", attrs.join(" "))
    };
    let one_line = match child {
        Some(_) => head.len() + 1,
        None => head.len() + 3,
    } <= WRAP_AT;

    match (one_line, child) {
        (true, None) => format!("{head} />"),
        (true, Some(c)) => format!("{head}>\n{pad}{c}\n</{name}>"),
        (false, child) => {
            let mut s = format!("<{name}\n");
            for a in attrs {
                s.push_str(&format!("{pad}{a}\n"));
            }
            match child {
                None => s.push_str("/>"),
                Some(c) => s.push_str(&format!(">\n{pad}{c}\n</{name}>")),
            }
            s
        }
    }
}

// ---------------------------------------------------------------- values

/// The bool worth showing: the one that is *not* the default.
///
/// A prop rendered at its own default documents nothing — `<CodeBlock
/// copyable={true} />` is the same call as `<CodeBlock />`. Yew's
/// `#[prop_or_default]` on a `bool` is `false`, so those show `true`.
fn shown_bool(default: &AbiDefault) -> bool {
    match default {
        AbiDefault::Expr { ts: Some(lit), .. } => lit != "true",
        _ => true,
    }
}

/// The enum variant worth showing: the first *writable* one that is not the
/// prop's default. A single-variant enum has no such choice and shows its only
/// variant.
///
/// "Writable" is the constraint that does the work. `parse::expand_data_enums`
/// rewrites `BadgeVariant::Kind(TermKind)` into its eight inhabitants, but it
/// does **not** keep `TermKind` in the IR — `parse` registers the types a prop
/// names, and no prop names `TermKind` (`src/atoms/onto_badge.rs:60` names
/// `BadgeVariant`). So a snippet spelling `BadgeVariant::Kind(TermKind::Ontology)`
/// would need a `use` line for a type this stage has no span for, and inventing
/// one is how a documentation snippet stops compiling. `BadgeVariant::Lang` is
/// the first payload-free variant, needs only `BadgeVariant`, and covers exactly
/// as much of the API — the full twelve-member union is in
/// `abi/toolkit.abi.json`, which is where a reader looks for the whole set.
fn shown_variant(name: &str, default: &PropDefault, tk: &Toolkit) -> Result<String> {
    let Some(e) = tk.unit_enum(name) else {
        bail!("enum `{name}` is a prop type but `parse` did not register it");
    };
    let default_variant = match default {
        // `#[prop_or(ButtonVariant::Primary)]` — the tail of the path is the
        // variant, and `parse` records variants without their enum prefix.
        PropDefault::Expr { expr, .. } => {
            expr.rsplit("::").next().map(|v| v.trim().to_string())
        }
        PropDefault::DefaultTrait => e.default_variant.clone(),
        PropDefault::Required => None,
    };
    let writable = |v: &&String| payload_types(v).iter().all(|t| type_span(t, tk).is_some());
    e.variants
        .iter()
        .filter(writable)
        .find(|v| Some(v.as_str()) != default_variant.as_deref())
        .or_else(|| e.variants.iter().find(writable))
        .cloned()
        .ok_or_else(|| {
            anyhow::anyhow!(
                "enum `{name}` ({}:{}) has no variant this stage can write: every one of \
                 [{}] carries a payload type `parse` did not register, so no `use` line for \
                 the snippet can be derived",
                e.span.file,
                e.span.line,
                e.variants.join(", ")
            )
        })
}

/// The types a recorded variant's payload names.
///
/// `parse::expand_variant` rewrites a data-carrying variant into its
/// inhabitants, so `BadgeVariant::Kind` is recorded as `Kind(TermKind::Ontology)`
/// — and a snippet that spells that needs `TermKind` in scope as well as
/// `BadgeVariant`. `TermKind` lives in `src/ontology.rs`, which `src/lib.rs`
/// does *not* glob, so getting this wrong is a `use` line that does not resolve.
fn payload_types(variant: &str) -> Vec<String> {
    let Some(open) = variant.find('(') else { return Vec::new() };
    let payload = variant[open + 1..].trim_end_matches(')').trim();
    match payload.split("::").next() {
        Some(head) if head.chars().next().is_some_and(char::is_uppercase) => {
            vec![head.to_string()]
        }
        _ => Vec::new(),
    }
}

/// Where a named type is declared, for [`module_path`].
fn type_span<'a>(name: &str, tk: &'a Toolkit) -> Option<&'a Span> {
    tk.unit_enum(name)
        .map(|e| &e.span)
        .or_else(|| tk.plain_struct(name).map(|s| &s.span))
}

/// The path a `use` statement needs, derived from `src/lib.rs`'s re-exports.
///
/// `src/lib.rs` ends with `pub use atoms::*; pub use molecules::*; pub use
/// organisms::*;`, so everything declared under those three directories is at
/// the crate root. Nothing else is — `ontology` is a `pub mod` with no glob
/// re-export, which is why `TermKind` and `LiteralValue` keep their module
/// segment.
fn module_path(span: &Span) -> String {
    let flattened = ["src/atoms/", "src/molecules/", "src/organisms/"];
    if flattened.iter().any(|d| span.file.starts_with(d)) {
        return CRATE_NAME.to_string();
    }
    let module = span
        .file
        .trim_start_matches("src/")
        .trim_end_matches(".rs")
        .replace('/', "::");
    if module.is_empty() || module == "lib" {
        CRATE_NAME.to_string()
    } else {
        format!("{CRATE_NAME}::{module}")
    }
}

/// `Vec<AttrValue>` -> `AttrValue`. Anything that is not a `Vec` comes back
/// unchanged, which lands the caller on the `.into()` side of [`is_str_slice`] —
/// verbose but always correct.
fn vec_item_ty(rust_ty: &str) -> &str {
    rust_ty
        .trim()
        .strip_prefix("Vec<")
        .and_then(|t| t.strip_suffix('>'))
        .map(str::trim)
        .unwrap_or(rust_ty)
}

/// `(&'static str, &'static str)` -> the two element types, split on top-level
/// commas so a nested generic cannot be cut in half.
fn tuple_element_tys(rust_ty: &str) -> Vec<&str> {
    let Some(inner) = rust_ty.trim().strip_prefix('(').and_then(|t| t.strip_suffix(')')) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0usize);
    for (i, ch) in inner.char_indices() {
        match ch {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(inner[start..].trim());
    out
}

/// A borrowed `str`, which takes a literal directly and rejects `.into()`-noise.
fn is_str_slice(rust_ty: &str) -> bool {
    matches!(rust_ty.trim(), "&str" | "&'static str")
}

/// `sectors` -> `sector`, so a list's element is keyed on a singular name in
/// [`PLACEHOLDERS`]. Naive on purpose: every plural prop and field in the crate
/// is a bare `-s`, and a real inflector would be a dependency to get `points` ->
/// `point` right.
fn singular(name: &str) -> String {
    match name.strip_suffix('s') {
        Some(stem) if !stem.is_empty() => stem.to_string(),
        _ => name.to_string(),
    }
}

/// The text a prop, field or tuple element is shown with.
///
/// A default the ABI could evaluate wins, because it is a fact about the
/// component rather than a placeholder. `PropDefault::Expr::ts` is already a TS
/// literal, quotes included.
fn text_for(path: &str, default: &AbiDefault) -> String {
    if let AbiDefault::Expr { ts: Some(lit), .. } = default {
        if let Some(inner) = lit.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            return inner.replace("\\\"", "\"").replace("\\\\", "\\");
        }
    }
    if let Some((_, v)) = PLACEHOLDERS.iter().find(|(k, _)| *k == path) {
        return (*v).to_string();
    }
    title_case(path)
}

/// Names whose title-cased form would be *wrong* rather than merely bland, and
/// what to show instead. Keyed on a prop name, a struct field name, or
/// `<name>.<index>` for one element of a tuple.
///
/// Everything not listed falls back to [`title_case`] of the name, which is why
/// this table is short and why it may stay short: an entry earns its place by
/// being a value a reader would otherwise have to guess is a URL, a CSS
/// fragment, a hex colour or a glyph.
const PLACEHOLDERS: &[(&str, &str)] = &[
    // Links and asset paths. `href="Href"` is not a link.
    ("href", "#"),
    // Singular, because a list element is keyed on `singular(prop)`: the two
    // halves of one `NavDropdownProps::links` entry are `link.0` and `link.1`.
    ("link.0", "#"),
    ("link.1", "About"),
    ("src", "/assets/placeholder.svg"),
    ("thumbnail", "/assets/placeholder.svg"),
    ("file", "/assets/eona-x-logo.svg"),
    ("alt", "EONA-X logo"),
    // CSS fragments, passed through to a `style`/`class` attribute verbatim.
    ("hex", "#040553"),
    ("background", "#fff"),
    ("color", "var(--eona-navy)"),
    ("border", "1px solid rgba(4,5,83,.12)"),
    ("preview_style", "background:var(--eona-navy);padding:24px;"),
    // Short codes a reader would otherwise read as prose.
    ("id", "example"),
    ("number", "01"),
    ("num", "01"),
    ("icon", "\u{2713}"),
    // Content whose shape is the point: `PillarCard.points` is (label, text),
    // `CodeBlock.code` is source, `UsageExample.source` is a Yew snippet — and
    // the last two contain a `"`, which is what exercises the raw-string path in
    // `yew_text`.
    ("point.0", "Open"),
    ("point.1", "by default, not by exception"),
    ("code", "let label = \"Go\";"),
    ("source", "<Button label=\"Go\" />"),
    ("component", "Button"),
    ("ty", "AttrValue"),
    ("doc", "What the prop does."),
    ("usage", "Primary brand colour"),
    ("value", "An annotation value"),
    ("sector", "Mobility"),
    ("variant", "100"),
];

/// `title_suffix` -> `Title Suffix`. The fallback placeholder: it names the prop
/// back at the reader, which is bland but never misleading.
fn title_case(name: &str) -> String {
    name.split(['_', '-', '.'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut cs = w.chars();
            match cs.next() {
                Some(c) => c.to_uppercase().collect::<String>() + cs.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------- literals

/// A text value in Yew attribute position.
///
/// A `"` inside forces the braced raw-string form the crate's own snippets use
/// (`src/organisms/components_gallery.rs` writes `source={r##"..."##}`); escape
/// soup in a documentation snippet is unreadable and invites a transcription
/// error when someone copies it.
fn yew_text(s: &str) -> String {
    if s.contains('"') {
        format!("{{{}}}", raw_str(s))
    } else {
        format!("\"{}\"", escape(s))
    }
}

/// The same value in Rust expression position, where there is no attribute
/// shorthand and a raw string is still the readable form.
fn yew_literal(s: &str) -> String {
    if s.contains('"') {
        raw_str(s)
    } else {
        format!("\"{}\"", escape(s))
    }
}

/// A text value in JSX attribute position. JSX attribute strings do not process
/// escapes, so a `"` has to leave attribute position entirely and become a
/// braced expression.
fn jsx_text(s: &str) -> String {
    if s.contains('"') {
        format!("{{\"{}\"}}", escape(s))
    } else {
        format!("\"{}\"", escape(s))
    }
}

/// `\` and `"`, the only two characters Rust and JavaScript string literals
/// spell the same way and the only two any placeholder contains.
fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// `r#"..."#` with as many hashes as the content needs.
fn raw_str(s: &str) -> String {
    let mut hashes = 1;
    while s.contains(&format!("\"{}", "#".repeat(hashes))) {
        hashes += 1;
    }
    let h = "#".repeat(hashes);
    format!("r{h}\"{s}\"{h}")
}

// ---------------------------------------------------------------- emitting

/// Both artifacts, as relative path -> contents. Pure, so `check` can diff bytes
/// without writing anything.
pub fn files(ex: &Examples) -> Result<BTreeMap<&'static str, String>> {
    let mut out = BTreeMap::new();
    out.insert(EXAMPLES_JSON, to_json(ex)?);
    out.insert(EXAMPLES_RS, to_rust(ex)?);
    Ok(out)
}

pub fn to_json(ex: &Examples) -> Result<String> {
    let json = serde_json::to_string_pretty(ex).context("serialising the usage examples")?;
    Ok(format!("{json}\n"))
}

/// The `bridge/examples.rs` table.
///
/// Every string is a raw literal so that a snippet's own quotes and backslashes
/// survive verbatim: the Yew snippets are Rust source, and re-escaping Rust
/// source into a Rust escape sequence is how a documentation string ends up
/// showing `\\\"` on a web page.
pub fn to_rust(ex: &Examples) -> Result<String> {
    let mut s = String::new();
    s.push_str(&format!(
        "//! Generated by `cargo xtask bridgegen` from the eona-ui-toolkit Yew crate. Do not edit.\n\
         //!\n\
         //! One usage snippet per component, in both stacks. The React half is\n\
         //! derived from the same `abi/toolkit.abi.json` entry as the Yew half, so a\n\
         //! prop added in `src/` reaches both panes of the components page at once.\n\
         //!\n\
         //! Outside `src/` on purpose: `src_hash` is taken over `src/**/*.rs`\n\
         //! (xtask/src/parse.rs), and a generated file inside `src/` would be an input\n\
         //! to the hash of the document that describes it.\n\
         //!\n\
         //! src_hash {}\n\n",
        ex.src_hash
    ));
    s.push_str(REACT_SNIPPET_TY);
    s.push_str(SNIPPET_TY);
    s.push_str(
        "/// Every component the crate defines, sorted by name — excluded ones\n\
         /// included, so a component cannot leave this table by being absent from a\n\
         /// list nobody counts.\n\
         pub static SNIPPETS: &[Snippet] = &[\n",
    );
    for e in &ex.components {
        s.push_str("    Snippet {\n");
        s.push_str(&format!("        component: {},\n", rust_lit(&e.component)));
        s.push_str(&format!("        source: {},\n", rust_lit(&e.source)));
        s.push_str(&format!("        yew_uses: {},\n", rust_lit(&e.yew.uses)));
        s.push_str(&format!("        yew: {},\n", rust_lit(&e.yew.snippet)));
        match &e.react {
            ReactSnippet::Available { import, jsx } => {
                s.push_str("        react: ReactSnippet::Available {\n");
                s.push_str(&format!("            import: {},\n", rust_lit(import)));
                s.push_str(&format!("            jsx: {},\n", rust_lit(jsx)));
                s.push_str("        },\n");
            }
            ReactSnippet::Unavailable { why, disposition, tracked_by } => {
                s.push_str("        react: ReactSnippet::Unavailable {\n");
                s.push_str(&format!("            why: {},\n", rust_lit(why)));
                s.push_str(&format!("            disposition: {},\n", rust_lit(disposition)));
                s.push_str(&format!("            tracked_by: {},\n", rust_lit(tracked_by)));
                s.push_str("        },\n");
            }
        }
        s.push_str("    },\n");
    }
    s.push_str("];\n\n");
    s.push_str(LOOKUP_FN);
    if s.contains('\r') {
        bail!("{EXAMPLES_RS} contains a CR; the generated artifacts are LF-only");
    }
    Ok(s)
}

const REACT_SNIPPET_TY: &str = "\
/// Whether a component ships in `@eona-x/ui-toolkit-react`.
///
/// Five do not — one needs an override and four are quarantined — and a page
/// that showed them JSX anyway would hand a reader an import that does not
/// resolve. `abi/overrides.toml` is where the disposition comes from.
pub enum ReactSnippet {
    Available { import: &'static str, jsx: &'static str },
    Unavailable { why: &'static str, disposition: &'static str, tracked_by: &'static str },
}

";

const SNIPPET_TY: &str = "\
/// One component's usage snippet, in both stacks.
pub struct Snippet {
    pub component: &'static str,
    /// `file:line` of the `#[function_component]`.
    pub source: &'static str,
    /// The `use` lines the Yew snippet needs, newline-separated.
    pub yew_uses: &'static str,
    pub yew: &'static str,
    pub react: ReactSnippet,
}

";

const LOOKUP_FN: &str = "\
/// The snippet for a component, by name.
///
/// Keyed on the component name rather than on the snippet text: a lookup keyed
/// on a value is invisible to the generator's verify gate, which renders every
/// component with sentinel props — both alphabets would miss the table, both
/// renders would agree, and the gate would certify a component frozen into the
/// not-found arm. See xtask/README.md, \"Handling a quarantine\".
pub fn snippet(component: &str) -> Option<&'static Snippet> {
    SNIPPETS
        .binary_search_by(|s| s.component.cmp(component))
        .ok()
        .map(|i| &SNIPPETS[i])
}
";

/// A `&'static str` literal for generated Rust: raw when it can be, escaped when
/// the content would break out of a raw string.
fn rust_lit(s: &str) -> String {
    if s.is_empty() {
        return "\"\"".into();
    }
    // A raw string cannot hold a CR and cannot end in `\`; neither occurs in a
    // snippet, but the escaped form is correct for both and costs nothing.
    if s.contains('\r') || s.ends_with('\\') {
        return format!("\"{}\"", escape(s).replace('\n', "\\n"));
    }
    raw_str(s)
}

/// Write both artifacts. Returns the relative paths written, for `check`'s
/// `--write` report.
pub fn write(root: &Path, ex: &Examples) -> Result<Vec<String>> {
    let mut written = Vec::new();
    for (rel, contents) in files(ex)? {
        let path = root.join(rel);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(&path, contents)
            .with_context(|| format!("writing {}", path.display()))?;
        written.push(rel.to_string());
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Component, PropEnum, Tier};

    fn span(file: &str, line: usize) -> Span {
        Span { file: file.into(), line }
    }

    fn prop(name: &str, rust_ty: &str, kind: PropKind, optional: bool, default: PropDefault) -> Prop {
        Prop {
            name: name.into(),
            rust_ty: rust_ty.into(),
            kind,
            optional,
            default,
            span: span("src/atoms/probe.rs", 7),
        }
    }

    fn component(name: &str, status: Status, props: Vec<Prop>) -> Component {
        Component {
            name: name.into(),
            props_ty: (!props.is_empty()).then(|| format!("{name}Props")),
            tier: Tier::Presentational,
            status,
            props,
            span: span("src/atoms/probe.rs", 20),
        }
    }

    /// A miniature crate exercising every shape this stage can write: text with
    /// and without a `"`, a bool with each kind of default, an enum whose
    /// default is an `#[prop_or(..)]` path, a struct, a list, a tuple, a slot
    /// and `children`.
    fn fixture() -> Toolkit {
        Toolkit {
            components: vec![
                component(
                    "Probe",
                    Status::Ok,
                    vec![
                        prop("label", "AttrValue", PropKind::Text, false, PropDefault::Required),
                        prop("href", "Option<AttrValue>", PropKind::Text, true, PropDefault::DefaultTrait),
                        prop("open", "bool", PropKind::Bool, false, PropDefault::DefaultTrait),
                        prop(
                            "variant",
                            "ProbeVariant",
                            PropKind::UnitEnum { name: "ProbeVariant".into() },
                            false,
                            PropDefault::Expr {
                                expr: "ProbeVariant::Primary".into(),
                                ts: None,
                            },
                        ),
                        prop(
                            "links",
                            "Vec<(&'static str, &'static str)>",
                            PropKind::List {
                                item: Box::new(PropKind::Tuple {
                                    items: vec![PropKind::Text, PropKind::Text],
                                }),
                            },
                            false,
                            PropDefault::Required,
                        ),
                        prop(
                            "rows",
                            "Vec<Row>",
                            PropKind::List {
                                item: Box::new(PropKind::Struct { name: "Row".into() }),
                            },
                            false,
                            PropDefault::Required,
                        ),
                        prop("children", "Children", PropKind::Slot, false, PropDefault::Required),
                    ],
                ),
                component(
                    "Withheld",
                    Status::Quarantined { reason: "panics while rendering a sentinel".into() },
                    vec![prop("code", "AttrValue", PropKind::Text, false, PropDefault::Required)],
                ),
                component("Excluded", Status::Excluded { reason: "zero-prop demo section".into() }, vec![]),
            ],
            enums: vec![PropEnum {
                name: "ProbeVariant".into(),
                variants: vec!["Primary".into(), "Ghost".into()],
                default_variant: None,
                span: span("src/atoms/probe.rs", 5),
            }],
            structs: vec![PlainStruct {
                name: "Row".into(),
                fields: vec![
                    prop("href", "AttrValue", PropKind::Text, false, PropDefault::Required),
                    prop("note", "Option<AttrValue>", PropKind::Text, true, PropDefault::Required),
                    prop("tag", "&'static str", PropKind::Text, false, PropDefault::Required),
                ],
                span: span("src/molecules/row.rs", 4),
            }],
            src_hash: "0".repeat(64),
        }
    }

    fn overrides() -> BTreeMap<String, BTreeMap<String, String>> {
        let mut entry = BTreeMap::new();
        entry.insert("disposition".to_string(), "hand-port the thumb.".to_string());
        entry.insert("tracked_by".to_string(), "#809".to_string());
        BTreeMap::from([("Withheld".to_string(), entry)])
    }

    fn built() -> Examples {
        let tk = fixture();
        let abi = crate::abi::build_abi(&tk, &BTreeMap::new()).expect("the fixture must build");
        build(&abi, &tk, &overrides()).expect("the fixture must produce snippets")
    }

    fn probe() -> Example {
        built().get("Probe").expect("Probe is in the fixture").clone()
    }

    /// Totality, the property `abi.rs` argues for: a component cannot leave this
    /// document by being absent from a list nobody counts.
    #[test]
    fn every_component_reaches_the_table_including_the_excluded_ones() {
        let ex = built();
        let names: Vec<&str> = ex.components.iter().map(|e| e.component.as_str()).collect();
        assert_eq!(names, vec!["Excluded", "Probe", "Withheld"], "sorted, and all three");
    }

    /// The requirement the whole stage exists for: one prop added in Rust, both
    /// panes grow, with no second artifact to edit by hand.
    #[test]
    fn a_prop_added_in_rust_appears_in_both_snippets() {
        let before = probe();
        let mut tk = fixture();
        tk.components[0].props.push(prop(
            "subtitle",
            "AttrValue",
            PropKind::Text,
            false,
            PropDefault::Required,
        ));
        let abi = crate::abi::build_abi(&tk, &BTreeMap::new()).unwrap();
        let after = build(&abi, &tk, &overrides()).unwrap().get("Probe").unwrap().clone();

        assert!(!before.yew.snippet.contains("subtitle"));
        assert!(after.yew.snippet.contains("subtitle=\"Subtitle\""), "{}", after.yew.snippet);
        let ReactSnippet::Available { jsx, .. } = &after.react else { panic!("Probe is Ok") };
        assert!(jsx.contains("subtitle=\"Subtitle\""), "{jsx}");
    }

    /// A withheld component gets the Yew call and the human's disposition, never
    /// JSX: a React snippet for a component `index.ts` does not export is an
    /// import that does not resolve.
    #[test]
    fn a_withheld_component_gets_an_honest_note_and_still_gets_its_yew_call() {
        let ex = built();
        let w = ex.get("Withheld").unwrap();
        assert!(w.yew.snippet.starts_with("<Withheld code="), "{}", w.yew.snippet);
        match &w.react {
            ReactSnippet::Unavailable { why, disposition, tracked_by } => {
                assert!(why.contains("quarantined it: panics while rendering a sentinel"));
                assert_eq!(disposition, "hand-port the thumb.");
                assert_eq!(tracked_by, "#809");
            }
            ReactSnippet::Available { .. } => panic!("a quarantined component has no React half"),
        }
        // An exclusion is a rule, not a decision to record, so `overrides.toml`
        // carries no entry and the disposition is empty rather than invented.
        match &ex.get("Excluded").unwrap().react {
            ReactSnippet::Unavailable { why, disposition, .. } => {
                assert!(why.contains("out of scope by rule: zero-prop demo section"));
                assert!(disposition.is_empty());
            }
            ReactSnippet::Available { .. } => panic!("an excluded component has no React half"),
        }
    }

    /// Attributes come out in `AbiComponent::props` order, which is alphabetical
    /// — the ABI sorts, and this stage reads the ABI. Deliberate: it is the only
    /// order that does not move when someone reorders a `*Props` struct.
    #[test]
    fn every_shape_is_written_in_both_stacks() {
        let p = probe();
        assert_eq!(
            p.yew.snippet,
            "<Probe\n\
             \x20   href=\"#\"\n\
             \x20   label=\"Label\"\n\
             \x20   links={vec![(\"#\", \"About\")]}\n\
             \x20   open={true}\n\
             \x20   rows={vec![Row { href: \"#\".into(), note: None, tag: \"Tag\" }]}\n\
             \x20   variant={ProbeVariant::Ghost}\n\
             >\n\
             \x20   { \"Children go here.\" }\n\
             </Probe>"
        );
        let ReactSnippet::Available { jsx, import } = &p.react else { panic!("Probe is Ok") };
        assert_eq!(import, "import { Probe } from '@eona-x/ui-toolkit-react';");
        assert_eq!(
            jsx,
            "<Probe\n\
             \x20 href=\"#\"\n\
             \x20 label=\"Label\"\n\
             \x20 links={[[\"#\", \"About\"]]}\n\
             \x20 open\n\
             \x20 rows={[{ href: \"#\", tag: \"Tag\" }]}\n\
             \x20 variant=\"ghost\"\n\
             >\n\
             \x20 Children go here.\n\
             </Probe>"
        );
    }

    /// Three separate facts, all of which a hand-written snippet gets wrong
    /// sooner or later: `Option<T>` in *prop* position needs no `Some` (yew's
    /// blanket `IntoPropValue`), `Option<T>` in *field* position does and is
    /// written `None`, and `&'static str` must not carry `.into()`.
    #[test]
    fn optionality_and_into_follow_the_rust_type_not_the_prop_name() {
        let p = probe();
        assert!(p.yew.snippet.contains("href=\"#\""), "no Some(..) in prop position");
        assert!(p.yew.snippet.contains("note: None"), "an optional field is written out");
        assert!(p.yew.snippet.contains("href: \"#\".into()"), "AttrValue needs the conversion");
        assert!(p.yew.snippet.contains("tag: \"Tag\""), "&'static str must not carry .into()");
        let ReactSnippet::Available { jsx, .. } = &p.react else { panic!() };
        assert!(!jsx.contains("note"), "an optional field is omitted from the JS object");
    }

    /// `shown_bool` and `shown_variant` both pick the value that is *not* the
    /// default, because a prop rendered at its default documents nothing.
    #[test]
    fn a_prop_is_shown_at_the_value_that_is_not_its_default() {
        assert!(shown_bool(&AbiDefault::DefaultTrait), "prop_or_default on a bool is false");
        assert!(!shown_bool(&AbiDefault::Expr {
            expr: "true".into(),
            ts: Some("true".into())
        }));
        let tk = fixture();
        assert_eq!(
            shown_variant(
                "ProbeVariant",
                &PropDefault::Expr { expr: "ProbeVariant::Primary".into(), ts: None },
                &tk
            )
            .unwrap(),
            "Ghost"
        );
        assert_eq!(
            shown_variant("ProbeVariant", &PropDefault::Required, &tk).unwrap(),
            "Primary",
            "with no default, the first variant"
        );
    }

    /// `BadgeVariant::Kind(TermKind::Ontology)` is a real inhabitant that this
    /// stage must not write: `parse` never registered `TermKind`, so there is no
    /// span to derive a `use` line from and the snippet would not compile.
    #[test]
    fn a_variant_naming_an_unregistered_payload_type_is_skipped() {
        let mut tk = fixture();
        tk.enums[0].variants = vec![
            "Kind(TermKind::Ontology)".into(),
            "Primary".into(),
            "Ghost".into(),
        ];
        assert_eq!(
            shown_variant("ProbeVariant", &PropDefault::Required, &tk).unwrap(),
            "Primary",
            "the payload-carrying variant is unwritable, so the first writable one wins"
        );
        assert_eq!(payload_types("Kind(TermKind::Ontology)"), vec!["TermKind".to_string()]);
        assert!(payload_types("Primary").is_empty());
    }

    /// If every variant is unwritable there is nothing honest to emit, so the
    /// run fails naming the enum rather than shipping a snippet that will not
    /// compile.
    #[test]
    fn an_enum_with_no_writable_variant_is_an_error_not_a_guess() {
        let mut tk = fixture();
        tk.enums[0].variants = vec!["Kind(TermKind::Ontology)".into()];
        let err = shown_variant("ProbeVariant", &PropDefault::Required, &tk).unwrap_err();
        assert!(err.to_string().contains("no variant this stage can write"), "{err}");
    }

    /// `src/lib.rs` globs `atoms`, `molecules` and `organisms` and nothing else.
    /// Getting this wrong is a `use` line that does not resolve — which is what
    /// `ontology::LiteralValue` and `demo::DemoPage` would both be.
    #[test]
    fn module_paths_follow_lib_rs_re_exports() {
        assert_eq!(module_path(&span("src/atoms/button.rs", 5)), "eona_ui_toolkit");
        assert_eq!(module_path(&span("src/molecules/props_table.rs", 6)), "eona_ui_toolkit");
        assert_eq!(module_path(&span("src/organisms/main_nav.rs", 4)), "eona_ui_toolkit");
        assert_eq!(module_path(&span("src/ontology.rs", 41)), "eona_ui_toolkit::ontology");
        assert_eq!(module_path(&span("src/demo.rs", 61)), "eona_ui_toolkit::demo");
    }

    /// The `"` in `CodeBlock.code` and `UsageExample.source` is what makes this
    /// load-bearing: an escaped copy of Rust source is unreadable on a docs page
    /// and invites a transcription error when someone copies it.
    #[test]
    fn a_quoted_value_becomes_a_raw_string_with_enough_hashes() {
        assert_eq!(yew_text("Label"), "\"Label\"");
        assert_eq!(yew_text("<Button label=\"Go\" />"), "{r#\"<Button label=\"Go\" />\"#}");
        assert_eq!(jsx_text("<Button label=\"Go\" />"), "{\"<Button label=\\\"Go\\\" />\"}");
        // `hex="#040553"` closes a one-hash raw string early.
        assert_eq!(raw_str("hex=\"#040553\""), "r##\"hex=\"#040553\"\"##");
    }

    /// The consumable half has to be Rust the toolkit can compile; `syn` is
    /// already a dependency, so proving it parses costs nothing and catches a
    /// literal that broke out of its raw string.
    #[test]
    fn the_generated_rust_parses() {
        let rust = to_rust(&built()).expect("the fixture must emit");
        syn::parse_file(&rust).expect("bridge/examples.rs must be valid Rust");
        assert!(!rust.contains('\r'), "the generated artifacts are LF-only");
    }

    /// Deterministic, so `check` may treat any difference at all as drift.
    #[test]
    fn two_builds_of_the_same_input_are_byte_identical() {
        let (a, b) = (built(), built());
        assert_eq!(files(&a).unwrap(), files(&b).unwrap());
        assert_eq!(
            files(&a).unwrap().keys().copied().collect::<Vec<_>>(),
            vec!["abi/examples.json", "bridge/examples.rs"]
        );
    }

    /// The JSON is the reviewable half, so it has to survive a round trip — a
    /// document `check` can write but not read back would fail the gate on the
    /// next run for a reason nobody could diagnose.
    #[test]
    fn the_json_round_trips() {
        let ex = built();
        let back: Examples = serde_json::from_str(&to_json(&ex).unwrap()).unwrap();
        assert_eq!(back, ex);
    }

    #[test]
    fn type_text_is_peeled_without_cutting_a_generic_in_half() {
        assert_eq!(vec_item_ty("Vec<AttrValue>"), "AttrValue");
        assert_eq!(vec_item_ty("AttrValue"), "AttrValue");
        assert_eq!(
            tuple_element_tys("(&'static str, Vec<(u8, u8)>)"),
            vec!["&'static str", "Vec<(u8, u8)>"]
        );
        assert!(is_str_slice("&'static str"));
        assert!(!is_str_slice("AttrValue"));
    }

    #[test]
    fn singular_and_title_case_are_the_fallback_placeholder() {
        assert_eq!(singular("sectors"), "sector");
        assert_eq!(singular("s"), "s");
        assert_eq!(title_case("title_suffix"), "Title Suffix");
        assert_eq!(title_case("links.1"), "Links 1");
    }
}
