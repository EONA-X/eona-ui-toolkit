//! Stage: `svelte`. Renders the spliced tree to the Svelte 5 package on disk.
//!
//! A sibling of `emit`, not a translation of its output. Both stages read the
//! same `splice::Jsx` tree — the markup Yew's own SSR renderer produced, with the
//! props spliced back in — and spell it for their framework, so a Svelte
//! component and a React component for the same Rust component cannot disagree
//! about markup unless one of the two spellings is wrong. The parity suite in
//! `tests/bridge-parity` renders both and compares them.
//!
//! What this stage shares with `emit` rather than restating: the plan (which
//! component is written, exported or withheld, from `Status` alone), the TS type
//! of every prop, every default, and `types.ts`. What it owns: the `.svelte`
//! spelling, `index.ts`, the manifests, and the opt-in `wasm` entry point.
//!
//! # Whitespace
//!
//! Svelte collapses whitespace *between* nodes to one space, where JSX drops a
//! line break between elements entirely. A generated file that put each child
//! on its own line would therefore render a space Yew never wrote — between
//! `MetricTile`'s three `<span>`s, say. So no whitespace is ever written outside
//! a tag: line breaks go *inside* an element's own angle brackets
//! (`<span\n  >{props.label}</span\n>`), which Svelte's parser accepts and which
//! produces no text node. Text that has leading, trailing or repeated whitespace
//! is written as a string expression, which Svelte renders verbatim.
//!
//! # The `wasm` entry point
//!
//! `@eona-x/ui-toolkit-svelte/wasm` is the one part of the package that is not
//! derived from `src/`: a `use:yew` action and a `<YewMount>` component that
//! mount a Yew component compiled to WebAssembly, for components the generator
//! cannot reach (interactive ones, which live outside this crate). It is opt-in
//! — nothing in the default entry imports it — and written from the constants
//! at the bottom of this file so that `generatorHash` covers it and the drift
//! gate owns every byte of the package.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use anyhow::{bail, Context, Result};

use crate::emit::{self, Emitted, Init, Plan, GENERATOR};
use crate::ir::*;
use crate::splice::{self, js_prop_name, AttrValue, Element, Frag, Jsx, StyleEntry};

pub const PACKAGE_DIR: &str = "packages/svelte";
pub const PACKAGE_NAME: &str = "@eona-x/ui-toolkit-svelte";
/// Kept in step with `emit::PACKAGE_VERSION`: the two packages are cut from the
/// same render, so they are versioned together.
pub const PACKAGE_VERSION: &str = emit::PACKAGE_VERSION;
pub const COMPONENT_EXT: &str = ".svelte";

/// What the markup reads props through. Not `props`, which is what `splice`
/// writes: a component that declares a binding called `props` and also calls
/// the `$props()` rune makes Svelte read the call as a *store subscription* to
/// `props`, and `svelte-check` rejects all fifteen components with defaults.
/// [`rename_props`] rewrites every `props.<name>` head to `p.<name>`.
const PROPS: &str = "p";
/// The binding `$props()` lands in when a component has defaults to apply,
/// before they are applied into [`PROPS`].
const INCOMING: &str = "incoming";
/// The module-level helper an inline `style` object becomes. See [`STYLE_HELPER_TS`].
const STYLE_HELPER: &str = "styleText";

/// Elements HTML defines as void. Svelte 5 warns on `<div />` and renders it as
/// an open tag, so a childless non-void element is written open-and-closed.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

/// The whole package as relative path -> contents, plus operator warnings.
///
/// `jsx` is `emit`'s view of each component (notes, withheld status), `trees`
/// is splice's markup. Both are keyed by component name; `plan` checks the
/// first against the IR and this function checks the second against the first.
pub fn render_package(
    tk: &Toolkit,
    jsx: &BTreeMap<String, emit::Jsx>,
    trees: &BTreeMap<String, Jsx>,
) -> Result<(BTreeMap<String, String>, Vec<String>)> {
    let mut warnings = Vec::new();
    let plan = emit::plan(tk, jsx, &mut warnings)?;

    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for entry in &plan.emitted {
        let name = &entry.component.name;
        let Some(tree) = trees.get(name) else {
            bail!("splice handed emit a body for {name} but the svelte stage no tree");
        };
        files.insert(format!("src/{name}{COMPONENT_EXT}"), component_svelte(entry, tree, tk)?);
    }
    if let Some(types) = emit::types_ts(&plan, tk)? {
        files.insert("src/types.ts".into(), types);
    }
    files.insert("src/index.ts".into(), index_ts(&plan, tk)?);
    files.insert("src/wasm/index.ts".into(), WASM_INDEX_TS.into());
    files.insert("src/wasm/action.ts".into(), WASM_ACTION_TS.into());
    files.insert("src/wasm/YewMount.svelte".into(), WASM_MOUNT_SVELTE.into());
    files.insert("package.json".into(), package_json()?);
    files.insert("tsconfig.json".into(), tsconfig_json()?);
    files.insert("README.md".into(), readme_md(&plan, tk)?);

    for (rel, contents) in &files {
        if contents.contains('\r') {
            bail!("{PACKAGE_DIR}/{rel} contains a CR; the generated package is LF-only");
        }
    }
    Ok((files, warnings))
}

/// The `src/<Name>.svelte` files the package is supposed to contain — the same
/// rule as [`emit::expected_files`], spelled with this package's extension.
pub fn expected_files(tk: &Toolkit) -> BTreeSet<String> {
    emit::expected_files(tk)
        .into_iter()
        .map(|p| format!("{}{COMPONENT_EXT}", p.trim_end_matches(".tsx")))
        .collect()
}

/// The one line an operator reads to know whether the run produced a package.
pub fn report_package(files: &BTreeMap<String, String>, pkg: &std::path::Path) {
    println!(
        "svelte: {} files -> {} ({} component files, {} exported)",
        files.len(),
        pkg.display(),
        files.keys().filter(|k| k.starts_with("src/") && !k.starts_with("src/wasm/") && k.ends_with(COMPONENT_EXT)).count(),
        files["src/index.ts"].matches("\nexport { default as ").count(),
    );
}

// ---------------------------------------------------------------- components

fn component_svelte(e: &Emitted<'_>, tree: &Jsx, tk: &Toolkit) -> Result<String> {
    let c = e.component;
    let Some(props_ty) = c.props_ty.as_deref() else {
        bail!("{} has no props struct but reached the svelte stage ({})", c.name, emit::span(&c.span));
    };

    let slots: BTreeMap<String, bool> = c
        .props
        .iter()
        .filter(|p| p.kind == PropKind::Slot)
        .map(|p| (format!("props.{}", js_prop_name(&p.name)), !emit::optional_marker(p).is_empty()))
        .collect();
    // Walked twice: once with splice's own `props.` spelling, which is what the
    // declared-props guard below reads, and once with it renamed to `p.` — on
    // expressions only, so a text node that happens to say "props." is left as
    // written.
    let mut cx = Cx { component: &c.name, slots, uses_style: false, rename: false };
    let markup = node(&mut cx, tree, 0)?;

    // The same guard `emit` applies to its body, for the same reason: `splice`
    // and the interface spell prop names independently.
    let declared: BTreeSet<String> = c.props.iter().map(|p| js_prop_name(&p.name)).collect();
    for read in emit::props_read(&markup) {
        if !declared.contains(&read) {
            bail!(
                "{}'s markup reads `props.{read}`, which {props_ty} does not declare ({})",
                c.name,
                emit::span(&c.span)
            );
        }
    }
    // The body only ever reads `props.<name>` and the binders `splice` names
    // for its `.map()` bodies, so these two names are free unless a binder
    // takes one — checked rather than assumed.
    let mut taken = BTreeSet::new();
    binders(tree, &mut taken);
    for name in [PROPS, INCOMING] {
        if taken.contains(name) {
            bail!("{}'s markup binds `{name}`, which the svelte stage reserves", c.name);
        }
    }
    cx.rename = true;
    let markup = node(&mut cx, tree, 0)?;

    let mut s = String::new();
    writeln!(s, "<!-- Generated by `{GENERATOR}` from {}. Do not edit. -->", emit::span(&c.span))?;
    if let Some(why) = &e.withheld {
        writeln!(s, "<!-- Not re-exported from the package index: {} -->", comment_safe(why))?;
    }
    for note in &e.jsx.notes {
        writeln!(s, "<!-- Note: {} -->", comment_safe(note))?;
    }

    writeln!(s, "<script module lang=\"ts\">")?;
    let mut imports = String::new();
    if !cx.slots.is_empty() {
        imports.push_str("  import type { Snippet } from 'svelte';\n");
    }
    let structs = emit::struct_names(c.props.iter().map(|p| &p.kind), tk);
    if !structs.is_empty() {
        writeln!(
            imports,
            "  import type {{ {} }} from './types';",
            structs.iter().cloned().collect::<Vec<_>>().join(", ")
        )?;
    }
    if !imports.is_empty() {
        s.push_str(&imports);
        s.push('\n');
    }
    s.push_str(&indent(&props_interface(c, props_ty, tk)?, 1));
    if cx.uses_style {
        s.push('\n');
        s.push_str(&indent(STYLE_HELPER_TS, 1));
    }
    writeln!(s, "</script>")?;
    writeln!(s)?;

    let defaults: Vec<(String, String)> = c
        .props
        .iter()
        .filter_map(|p| match emit::initializer(p, tk) {
            Init::Value(v) => Some((js_prop_name(&p.name), v)),
            _ => None,
        })
        .collect();
    writeln!(s, "<script lang=\"ts\">")?;
    if defaults.is_empty() {
        writeln!(s, "  let {PROPS}: {props_ty} = $props();")?;
    } else {
        // `$derived`, not a plain object: `$props()` is reactive and a spread
        // into a `const` would freeze the first render's values. The defaults
        // are applied with `??` for the reason `emit::function` gives —
        // `#[prop_or]` fills an *absent* prop, not a falsy one.
        writeln!(s, "  let {INCOMING}: {props_ty} = $props();")?;
        writeln!(s, "  const {PROPS} = $derived({{")?;
        writeln!(s, "    ...{INCOMING},")?;
        for (name, v) in &defaults {
            writeln!(s, "    {name}: {INCOMING}.{name} ?? {v},")?;
        }
        writeln!(s, "  }});")?;
    }
    writeln!(s, "</script>")?;
    writeln!(s)?;
    s.push_str(&markup);
    s.push('\n');
    Ok(s)
}

fn props_interface(c: &Component, props_ty: &str, tk: &Toolkit) -> Result<String> {
    let mut s = String::new();
    writeln!(s, "export interface {props_ty} {{")?;
    for p in &c.props {
        let name = js_prop_name(&p.name);
        emit::reject_reserved(&name, &p.span)?;
        if let Init::Unresolved { why } = emit::initializer(p, tk) {
            writeln!(s, "  // FIXME(bridgegen): no default emitted — {why}")?;
        }
        writeln!(s, "  /** `{}` — {}{} */", p.rust_ty, emit::span(&p.span), emit::default_note(p))?;
        writeln!(s, "  {name}{}: {};", emit::optional_marker(p), svelte_ts(&emit::ts(&p.kind, p.optional, tk)))?;
    }
    writeln!(s, "}}")?;
    Ok(s)
}

/// `classify::ts_type` spells a slot `ReactNode`; Svelte 5 passes children as a
/// `Snippet`. Word-bounded so a struct called `ReactNodeList` would survive.
fn svelte_ts(ts: &str) -> String {
    let mut out = String::new();
    let mut rest = ts;
    while let Some(i) = rest.find("ReactNode") {
        let before = rest[..i].chars().last();
        let after = rest[i + "ReactNode".len()..].chars().next();
        let ident = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        out.push_str(&rest[..i]);
        out.push_str(if ident(before) || ident(after) { "ReactNode" } else { "Snippet" });
        rest = &rest[i + "ReactNode".len()..];
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------- markup

struct Cx<'a> {
    component: &'a str,
    /// `props.<slot>` -> whether the slot is optional.
    slots: BTreeMap<String, bool>,
    uses_style: bool,
    /// Spell `props.<name>` as `p.<name>` (see [`PROPS`]).
    rename: bool,
}

impl Cx<'_> {
    /// A JavaScript expression from the tree, as this pass spells it.
    fn js(&self, e: &str) -> String {
        if self.rename {
            rename_props(e)
        } else {
            e.to_string()
        }
    }
}

/// One node, with no whitespace before or after it. See the module header for
/// why every line break lives inside an element's own brackets.
fn node(cx: &mut Cx<'_>, n: &Jsx, ind: usize) -> Result<String> {
    Ok(match n {
        Jsx::Nothing => String::new(),
        Jsx::Text(t) => text(t),
        Jsx::Expr(e) => match cx.slots.get(e) {
            Some(true) => format!("{{@render {}?.()}}", cx.js(e)),
            Some(false) => format!("{{@render {}()}}", cx.js(e)),
            None => {
                if cx.slots.keys().any(|s| mentions(e, s)) {
                    bail!(
                        "{}: the expression `{e}` reads a slot other than by rendering it; a \
                         Svelte snippet is a function, not a node",
                        cx.component
                    );
                }
                format!("{{{}}}", cx.js(e))
            }
        },
        Jsx::Element(el) => element(cx, el, ind)?,
        Jsx::Fragment(items) => seq(cx, items, ind)?,
        Jsx::When { cond, then } => {
            format!("{{#if {}}}{}{{/if}}", cx.js(cond), node(cx, then, ind)?)
        }
        Jsx::Ternary { cond, then, other } => format!(
            "{{#if {}}}{}{{:else}}{}{{/if}}",
            cx.js(cond),
            node(cx, then, ind)?,
            node(cx, other, ind)?
        ),
        Jsx::Map { list, binder, index, body, sep } => {
            let mut s = format!("{{#each {} as {binder}, {index}}}", cx.js(list));
            if !sep.is_empty() {
                write!(s, "{{#if {index} > 0}}{}{{/if}}", seq(cx, sep, ind)?)?;
            }
            s.push_str(&seq(cx, body, ind)?);
            s.push_str("{/each}");
            s
        }
    })
}

fn seq(cx: &mut Cx<'_>, items: &[Jsx], ind: usize) -> Result<String> {
    let mut s = String::new();
    for it in items {
        s.push_str(&node(cx, it, ind)?);
    }
    Ok(s)
}

fn element(cx: &mut Cx<'_>, el: &Element, ind: usize) -> Result<String> {
    let mut s = format!("<{}", el.tag);
    for a in &el.attrs {
        let name = splice::html_attr(&a.name, el.svg)
            .with_context(|| format!("{}: <{}> attribute `{}`", cx.component, el.tag, a.name))?;
        s.push(' ');
        s.push_str(&attr(cx, &name, &a.value));
    }
    if el.children.is_empty() {
        if el.svg || VOID.contains(&el.tag.as_str()) {
            s.push_str(" />");
        } else {
            write!(s, "></{}>", el.tag)?;
        }
        return Ok(s);
    }
    write!(s, "\n{}>", pad(ind + 1))?;
    s.push_str(&seq(cx, &el.children, ind + 1)?);
    write!(s, "</{}\n{}>", el.tag, pad(ind))?;
    Ok(s)
}

fn attr(cx: &mut Cx<'_>, name: &str, v: &AttrValue) -> String {
    match v {
        AttrValue::True => name.to_string(),
        AttrValue::Lit(l) if !l.chars().any(|c| matches!(c, '"' | '{' | '}' | '&')) => {
            format!("{name}=\"{l}\"")
        }
        // A raw CSS string is exactly what an HTML `style` attribute takes; only
        // React needed `cssText` to parse it into an object.
        AttrValue::StyleText(e) => format!("{name}={{{}}}", cx.js(e)),
        other => format!("{name}={{{}}}", value(cx, other)),
    }
}

/// The JavaScript expression an attribute value denotes.
fn value(cx: &mut Cx<'_>, v: &AttrValue) -> String {
    match v {
        AttrValue::Lit(l) => splice::js_string(l),
        AttrValue::Expr(e) => cx.js(e),
        AttrValue::Template(frags) => splice::template(
            &frags
                .iter()
                .map(|f| match f {
                    Frag::Expr(e) => Frag::Expr(cx.js(e)),
                    lit => lit.clone(),
                })
                .collect::<Vec<_>>(),
        ),
        AttrValue::True => "true".into(),
        AttrValue::StyleText(e) => cx.js(e),
        // The same object React's `style` gets, keyed by the CSS property name
        // Yew wrote rather than its camel-cased DOM spelling.
        AttrValue::Style(entries) => {
            cx.uses_style = true;
            let decls = entries
                .iter()
                .map(|e| match e {
                    StyleEntry::Prop { key, value: v } => {
                        format!("{}: {}", splice::js_string(key), value(cx, v))
                    }
                    StyleEntry::Spread { cond, props } => {
                        let inner = props
                            .iter()
                            .map(|(k, v)| format!("{}: {}", splice::js_string(k), value(cx, v)))
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("...({} ? {{ {inner} }} : {{}})", cx.js(cond))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{STYLE_HELPER}({{ {decls} }})")
        }
        AttrValue::Cond { arms, fallback } => {
            let mut out = String::new();
            for (cond, v) in arms {
                let v = value(cx, v);
                write!(out, "{} ? {v} : ", cx.js(cond)).unwrap();
            }
            out.push_str(&match fallback {
                Some(f) => value(cx, f),
                None => "undefined".into(),
            });
            out
        }
    }
}

/// Character data. Bare when Svelte would render it unchanged, a string
/// expression otherwise — `{`/`}` open a tag, `<` an element, `&` an entity,
/// and any whitespace Svelte might collapse is only safe inside an expression.
fn text(t: &str) -> String {
    let bare = !t.is_empty()
        && t.trim() == t
        && !t.contains("  ")
        && !t.chars().any(|c| matches!(c, '{' | '}' | '<' | '>' | '&') || (c.is_whitespace() && c != ' '));
    if bare {
        t.to_string()
    } else {
        format!("{{{}}}", splice::js_string(t))
    }
}

/// Every name a `.map()` body binds, anywhere in the tree.
fn binders(n: &Jsx, out: &mut BTreeSet<String>) {
    match n {
        Jsx::Map { binder, index, body, sep, .. } => {
            out.insert(binder.clone());
            out.insert(index.clone());
            body.iter().chain(sep.iter()).for_each(|c| binders(c, out));
        }
        Jsx::Element(e) => e.children.iter().for_each(|c| binders(c, out)),
        Jsx::Fragment(items) => items.iter().for_each(|c| binders(c, out)),
        Jsx::When { then, .. } => binders(then, out),
        Jsx::Ternary { then, other, .. } => {
            binders(then, out);
            binders(other, out);
        }
        Jsx::Text(_) | Jsx::Expr(_) | Jsx::Nothing => {}
    }
}

/// `props.<name>` -> `p.<name>` at every *head* position, by the same rule
/// `emit::props_read` uses to find them: a `props.` preceded by an identifier
/// character or a `.` is something else (`item.props.x`, `$props.`).
fn rename_props(markup: &str) -> String {
    let b = markup.as_bytes();
    let mut out = String::with_capacity(markup.len());
    let mut last = 0;
    for (i, _) in markup.match_indices("props.") {
        let prev = if i > 0 { Some(b[i - 1]) } else { None };
        if prev.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'.') {
            continue;
        }
        out.push_str(&markup[last..i]);
        out.push_str(PROPS);
        out.push('.');
        last = i + "props.".len();
    }
    out.push_str(&markup[last..]);
    out
}

/// Whether `expr` reads `path` as a whole identifier path, not as a prefix of a
/// longer one (`props.children` vs `props.childrenCount`).
fn mentions(expr: &str, path: &str) -> bool {
    expr.match_indices(path).any(|(i, _)| {
        let before = expr[..i].chars().last();
        let after = expr[i + path.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '$')
            && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    })
}

/// A verify note quotes markup and sentinel escapes; the one thing that cannot
/// sit inside an HTML comment is the comment's own terminator.
fn comment_safe(s: &str) -> String {
    s.replace("-->", "-- >")
}

fn pad(n: usize) -> String {
    "  ".repeat(n)
}

fn indent(block: &str, n: usize) -> String {
    block
        .lines()
        .map(|l| if l.trim().is_empty() { String::new() } else { format!("{}{l}", pad(n)) })
        .map(|l| l + "\n")
        .collect()
}

// --------------------------------------------------------------------- index

fn index_ts(plan: &Plan<'_>, tk: &Toolkit) -> Result<String> {
    let mut s = String::new();
    writeln!(s, "// Generated by `{GENERATOR}` from the eona-ui-toolkit Yew crate. Do not edit.")?;
    writeln!(s, "// src_hash {}", tk.src_hash)?;
    writeln!(s, "//")?;
    writeln!(s, "// Zero-WebAssembly. The opt-in Yew/WASM mount is a separate entry point:")?;
    writeln!(s, "// `import {{ YewMount }} from '{PACKAGE_NAME}/wasm'`.")?;
    writeln!(s)?;
    for e in plan.emitted.iter().filter(|e| e.withheld.is_none()) {
        let name = &e.component.name;
        let props_ty = e.component.props_ty.as_deref().unwrap_or_default();
        writeln!(s, "export {{ default as {name} }} from './{name}{COMPONENT_EXT}';")?;
        writeln!(s, "export type {{ {props_ty} }} from './{name}{COMPONENT_EXT}';")?;
    }

    let types = emit::struct_names(
        plan.emitted.iter().flat_map(|e| e.component.props.iter().map(|p| &p.kind)),
        tk,
    );
    if !types.is_empty() {
        writeln!(s)?;
        writeln!(s, "export type {{ {} }} from './types';", types.iter().cloned().collect::<Vec<_>>().join(", "))?;
    }

    let withheld: Vec<(&str, &str)> = plan
        .emitted
        .iter()
        .filter_map(|e| e.withheld.as_deref().map(|w| (e.component.name.as_str(), w)))
        .chain(plan.omitted.iter().map(|(c, r)| (c.name.as_str(), r.as_str())))
        .collect();
    writeln!(s)?;
    writeln!(s, "// Deliberately not exported ({}):", withheld.len())?;
    if withheld.is_empty() {
        writeln!(s, "//   (none)")?;
    }
    for (name, why) in &withheld {
        let c = tk.component(name).map(|c| emit::span(&c.span)).unwrap_or_else(|| "?".into());
        writeln!(s, "//   {name} — {why} ({c})")?;
    }
    if !plan.excluded.is_empty() {
        let total: usize = plan.excluded.values().map(Vec::len).sum();
        writeln!(s, "//")?;
        writeln!(s, "// Out of scope by rule ({total}), never generated:")?;
        for (rule, names) in &plan.excluded {
            writeln!(s, "//   {:>2} {rule}: {}", names.len(), names.join(", "))?;
        }
    }
    Ok(s)
}

/// The component names `index.ts` re-exports: one
/// `export { default as Name } from './Name.svelte';` line each, as
/// [`index_ts`] writes them. A commented-out line is not a match.
pub fn exported_from_index(index: &str) -> BTreeSet<String> {
    index
        .lines()
        .filter_map(|l| l.strip_prefix("export { default as "))
        .filter_map(|rest| rest.split_whitespace().next())
        .map(|n| n.trim_end_matches('}').trim_end_matches(',').to_string())
        .collect()
}

// ------------------------------------------------------------------ manifests

fn package_json() -> Result<String> {
    let v = serde_json::json!({
        "//": format!(
            "Generated by `{GENERATOR}` from the eona-ui-toolkit Yew crate. Do not edit — the \
             next bridgegen overwrites this file."
        ),
        "name": PACKAGE_NAME,
        "version": PACKAGE_VERSION,
        "description": "Svelte 5 port of the EONA-X design system, generated from the eona-ui-toolkit Yew crate, with an opt-in mount for Yew components compiled to WebAssembly.",
        "license": "Apache-2.0",
        "repository": {
            "type": "git",
            "url": "git+https://github.com/EONA-X/eona-ui-toolkit.git",
            "directory": PACKAGE_DIR,
        },
        "type": "module",
        // The wasm entry has none either: the action only touches the DOM once a
        // component uses it, and the module it mounts is the caller's import.
        "sideEffects": false,
        "files": ["dist", "src", "README.md"],
        "svelte": "./dist/index.js",
        "types": "./dist/index.d.ts",
        "exports": {
            // `svelte-package` ships `.svelte` sources, compiled by the
            // consumer's own Svelte; `svelte` is the condition its bundler
            // plugins resolve.
            ".": {
                "types": "./dist/index.d.ts",
                "svelte": "./dist/index.js",
                "default": "./dist/index.js",
            },
            "./wasm": {
                "types": "./dist/wasm/index.d.ts",
                "svelte": "./dist/wasm/index.js",
                "default": "./dist/wasm/index.js",
            },
            "./package.json": "./package.json",
        },
        "scripts": {
            "build": "svelte-package -i src -o dist",
            "check": "svelte-check --tsconfig ./tsconfig.json --fail-on-warnings",
        },
        // Runes (`$props`, `$derived`), snippets and native `lang="ts"` are all
        // Svelte 5.0; nothing later is used.
        "peerDependencies": { "svelte": "^5.0.0" },
        // The stylesheet, at exactly the version cut from the same render.
        "dependencies": { emit::CSS_PACKAGE: PACKAGE_VERSION },
        "publishConfig": { "access": "public", "provenance": true },
        "devDependencies": {
            "@sveltejs/package": "^2.3.0",
            "svelte": "^5.0.0",
            "svelte-check": "^4.0.0",
            "typescript": "^5.5.0",
        },
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&v)?))
}

fn tsconfig_json() -> Result<String> {
    let v = serde_json::json!({
        "//": format!("Generated by `{GENERATOR}`. Do not edit."),
        "compilerOptions": {
            "target": "ES2021",
            "lib": ["ES2021", "DOM"],
            "module": "ESNext",
            "moduleResolution": "Bundler",
            "exactOptionalPropertyTypes": true,
            "strict": true,
            "verbatimModuleSyntax": true,
            "isolatedModules": true,
            "declaration": true,
            "noEmit": true,
            "skipLibCheck": true,
        },
        "include": ["src"],
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&v)?))
}

fn readme_md(plan: &Plan<'_>, tk: &Toolkit) -> Result<String> {
    let exported = plan.emitted.iter().filter(|e| e.withheld.is_none()).count();
    let mut s = String::new();
    writeln!(s, "<!-- Generated by `{GENERATOR}` from the eona-ui-toolkit Yew crate. Do not edit. -->")?;
    writeln!(s)?;
    writeln!(s, "# {PACKAGE_NAME}")?;
    writeln!(s)?;
    writeln!(s, "Svelte 5 components for the EONA-X design system. **This package is generated.** The")?;
    writeln!(s, "source of truth is the Yew crate in this repository (`src/`): each component was produced by")?;
    writeln!(s, "rendering the Rust component with Yew's own server renderer and splicing the props back")?;
    writeln!(s, "into the resulting markup — the same tree `@eona-x/ui-toolkit-react` is written from.")?;
    writeln!(s)?;
    writeln!(s, "Generated from src_hash `{}`. {exported} components exported.", tk.src_hash)?;
    writeln!(s)?;
    writeln!(s, "## Two entry points")?;
    writeln!(s)?;
    writeln!(s, "| Import | What it is | WebAssembly |")?;
    writeln!(s, "| --- | --- | --- |")?;
    writeln!(s, "| `{PACKAGE_NAME}` | the generated components, as `.svelte` | none |")?;
    writeln!(s, "| `{PACKAGE_NAME}/wasm` | `<YewMount>` and `use:yew`, to mount a Yew component compiled to WASM | yours, loaded on demand |")?;
    writeln!(s)?;
    writeln!(s, "Use the first for everything it exports: it server-renders, it tree-shakes, and its markup")?;
    writeln!(s, "is Yew's. Reach for the second only for a component the generator cannot reach — an")?;
    writeln!(s, "interactive one, which lives outside this crate — and accept its cost: one component over a")?;
    writeln!(s, "WASM bridge was measured at 74,593 B gzip against 112 B pre-rendered, it renders nothing")?;
    writeln!(s, "on the server, and a CSP must allow `wasm-unsafe-eval`.")?;
    writeln!(s)?;
    writeln!(s, "```sh")?;
    writeln!(s, "npm install {PACKAGE_NAME}")?;
    writeln!(s, "```")?;
    writeln!(s)?;
    writeln!(s, "```svelte")?;
    writeln!(s, "<script lang=\"ts\">")?;
    writeln!(s, "  import '{}';", emit::CSS_PACKAGE)?;
    writeln!(s, "  import {{ Button, Pill }} from '{PACKAGE_NAME}';")?;
    writeln!(s, "</script>")?;
    writeln!(s)?;
    writeln!(s, "<div class=\"eona-ui\">")?;
    writeln!(s, "  <Button label=\"Explore\" />")?;
    writeln!(s, "  <Pill>Mobility</Pill>")?;
    writeln!(s, "</div>")?;
    writeln!(s, "```")?;
    writeln!(s)?;
    writeln!(s, "`{}` comes with this package, at the same version. Every rule in it is scoped under", emit::CSS_PACKAGE)?;
    writeln!(s, "`.eona-ui`, so render the components inside an element with that class; it then touches")?;
    writeln!(s, "nothing else on the page.")?;
    writeln!(s)?;
    writeln!(s, "A Rust `Children`/`Html` prop is a Svelte 5 `Snippet`: pass `children` as element content")?;
    writeln!(s, "and any other slot (`ShapeCard`'s `demo`) as a `{{#snippet}}`.")?;
    writeln!(s)?;
    writeln!(s, "### Mounting a Yew component")?;
    writeln!(s)?;
    writeln!(s, "The crate you mount from exports one function, `mount(target, component, props)`, built with")?;
    writeln!(s, "`wasm-bindgen` and Yew's `csr` feature; `examples/svelte-wasm` in the toolkit repository is a")?;
    writeln!(s, "working one.")?;
    writeln!(s)?;
    writeln!(s, "```svelte")?;
    writeln!(s, "<script lang=\"ts\">")?;
    writeln!(s, "  import {{ YewMount }} from '{PACKAGE_NAME}/wasm';")?;
    writeln!(s, "  const load = () => import('my-yew-widgets').then(async (m) => (await m.default(), m));")?;
    writeln!(s, "</script>")?;
    writeln!(s)?;
    writeln!(s, "<YewMount module={{load}} component=\"Counter\" props={{{{ start: 3 }}}} />")?;
    writeln!(s, "```")?;
    writeln!(s)?;
    writeln!(s, "## Documentation")?;
    writeln!(s)?;
    writeln!(s, "- [Getting started](https://eona-x.github.io/eona-ui-toolkit/svelte/getting-started/) — install, a first component, a static site, theming")?;
    writeln!(s, "- [Components](https://eona-x.github.io/eona-ui-toolkit/svelte/) — every component, rendered live, with its call")?;
    writeln!(s)?;
    writeln!(s, "## Do not edit")?;
    writeln!(s)?;
    writeln!(s, "Every file here is overwritten by `cargo xtask bridgegen` and checked by `cargo xtask check`.")?;
    writeln!(s, "A bug in a component is a bug in the Rust component named in its header.")?;
    Ok(s)
}

// ----------------------------------------------------------------- runtime

/// `style={{...}}` in React is an object; in Svelte it is a string. A
/// declaration whose value is `null`/`undefined` is dropped, as React drops it,
/// and an empty result removes the attribute rather than rendering `style=""`.
const STYLE_HELPER_TS: &str = r#"function styleText(decls: Record<string, string | number | null | undefined>): string | undefined {
  const out = Object.entries(decls)
    .filter(([, v]) => v != null && v !== '')
    .map(([k, v]) => `${k}: ${v}`)
    .join('; ');
  return out === '' ? undefined : out;
}
"#;

const WASM_INDEX_TS: &str = r#"// Generated by `cargo xtask bridgegen`. Do not edit.
//
// The opt-in WebAssembly path. Nothing in the package's default entry imports
// this module, so a consumer who never imports `@eona-x/ui-toolkit-svelte/wasm`
// ships no WebAssembly and no loader.

export { default as YewMount } from './YewMount.svelte';
export { yew } from './action';
export type { YewHandle, YewModule, YewModuleSource, YewParams } from './action';
"#;

const WASM_ACTION_TS: &str = r#"// Generated by `cargo xtask bridgegen`. Do not edit.
//
// `use:yew` — mount a Yew component compiled to WebAssembly into an element
// Svelte owns. The contract with the Rust side is one exported function:
//
//   #[wasm_bindgen]
//   pub fn mount(target: web_sys::Element, component: &str, props: JsValue)
//       -> Result<Handle, JsValue>
//
// where `Handle` has `update(props)` and `destroy()` (`free()` is accepted for a
// handle wasm-bindgen generated without a `destroy`). `examples/svelte-wasm` in
// the toolkit repository implements it.

/** A mounted component, as the Rust side hands it back. */
export interface YewHandle {
  update?(props: unknown): void;
  destroy?(): void;
  free?(): void;
}

/** What the wasm-bindgen module must export, once initialised. */
export interface YewModule {
  mount(target: Element, component: string, props: unknown): YewHandle;
}

/**
 * The module, or how to get it. A function is called once per mount, so a
 * dynamic `import()` behind it keeps the WebAssembly out of the initial bundle.
 */
export type YewModuleSource = YewModule | Promise<YewModule> | (() => YewModule | Promise<YewModule>);

export interface YewParams {
  module: YewModuleSource;
  component: string;
  props?: unknown;
}

function resolve(source: YewModuleSource): Promise<YewModule> {
  return Promise.resolve(typeof source === 'function' ? source() : source);
}

// The Rust side signals a bad component name or bad props by throwing. That
// must not escape into the host app as an unhandled rejection, and an action
// has no error channel of its own, so it is reported here.
function report(component: string, err: unknown): void {
  console.error(`[@eona-x/ui-toolkit-svelte/wasm] could not mount ${component}`, err);
}

function release(handle: YewHandle | undefined): void {
  if (!handle) return;
  if (handle.destroy) handle.destroy();
  else handle.free?.();
}

export function yew(node: Element, params: YewParams) {
  let current = params;
  let handle: YewHandle | undefined;
  // Bumped on every (re)mount and on destroy, so a module that resolves after
  // the element is gone — or after the component name changed — mounts nothing.
  let generation = 0;

  function mount(p: YewParams): void {
    const mine = ++generation;
    resolve(p.module)
      .then((m) => {
        if (mine !== generation) return;
        handle = m.mount(node, p.component, p.props ?? {});
      })
      .catch((err: unknown) => {
        if (mine === generation) report(p.component, err);
      });
  }

  mount(params);

  return {
    update(next: YewParams): void {
      const remount = next.module !== current.module || next.component !== current.component;
      current = next;
      if (!remount && handle?.update) {
        try {
          handle.update(next.props ?? {});
        } catch (err) {
          report(next.component, err);
        }
        return;
      }
      release(handle);
      handle = undefined;
      mount(next);
    },
    destroy(): void {
      generation++;
      release(handle);
      handle = undefined;
    },
  };
}
"#;

const WASM_MOUNT_SVELTE: &str = r#"<!-- Generated by `cargo xtask bridgegen`. Do not edit. -->
<!--
  A Yew component compiled to WebAssembly, mounted client-side. The server
  render is the empty host element: WebAssembly does not run during SSR, so a
  component that has to appear in server-rendered HTML belongs in the package's
  default entry, not here.
-->
<script lang="ts">
  import { yew, type YewModuleSource } from './action';

  interface Props {
    /** The wasm-bindgen module, a promise of it, or a function returning either. */
    module: YewModuleSource;
    /** The name the Rust side's `mount` dispatches on. */
    component: string;
    /** Handed to the Rust side as-is; it decides how to read them. */
    props?: unknown;
    /** Class of the host element. */
    class?: string | undefined;
  }

  let { module, component, props = {}, class: className }: Props = $props();
</script>

<div class={className} data-yew-component={component} use:yew={{ module, component, props }}></div>
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn el(tag: &str, attrs: Vec<(&str, AttrValue)>, children: Vec<Jsx>) -> Jsx {
        Jsx::Element(Element {
            tag: tag.into(),
            svg: false,
            attrs: attrs
                .into_iter()
                .map(|(n, v)| splice::Attr { name: n.into(), value: v })
                .collect(),
            children,
            key: None,
        })
    }

    fn cx() -> Cx<'static> {
        Cx { component: "T", slots: BTreeMap::new(), uses_style: false, rename: false }
    }

    #[test]
    fn no_whitespace_is_written_between_nodes() {
        let tree = el(
            "div",
            vec![("className", AttrValue::Lit("a".into()))],
            vec![
                el("span", vec![], vec![Jsx::Expr("props.label".into())]),
                Jsx::Expr("props.value".into()),
            ],
        );
        let out = node(&mut cx(), &tree, 0).unwrap();
        assert_eq!(out, "<div class=\"a\"\n  ><span\n    >{props.label}</span\n  >{props.value}</div\n>");
    }

    #[test]
    fn a_slot_is_rendered_not_interpolated() {
        let mut c = cx();
        c.slots.insert("props.children".into(), true);
        c.slots.insert("props.demo".into(), false);
        assert_eq!(node(&mut c, &Jsx::Expr("props.children".into()), 0).unwrap(), "{@render props.children?.()}");
        assert_eq!(node(&mut c, &Jsx::Expr("props.demo".into()), 0).unwrap(), "{@render props.demo()}");
        assert!(node(&mut c, &Jsx::Expr("String(props.demo)".into()), 0).is_err());
        assert_eq!(node(&mut c, &Jsx::Expr("props.demoCount".into()), 0).unwrap(), "{props.demoCount}");
    }

    #[test]
    fn text_that_svelte_would_reinterpret_becomes_an_expression() {
        assert_eq!(text("Prop"), "Prop");
        assert_eq!(text(" props"), "{\" props\"}");
        assert_eq!(text("a {b}"), "{\"a {b}\"}");
        assert_eq!(text("R&D"), "{\"R&D\"}");
        assert_eq!(text("a  b"), "{\"a  b\"}");
    }

    #[test]
    fn attributes_take_their_html_spelling() {
        let tree = el(
            "label",
            vec![
                ("htmlFor", AttrValue::Expr("props.id".into())),
                ("tabIndex", AttrValue::Lit("0".into())),
                ("inert", AttrValue::True),
                ("title", AttrValue::Lit("say \"hi\"".into())),
            ],
            vec![],
        );
        assert_eq!(
            node(&mut cx(), &tree, 0).unwrap(),
            "<label for={props.id} tabindex=\"0\" inert title={\"say \\\"hi\\\"\"}></label>"
        );
    }

    #[test]
    fn an_inline_style_object_becomes_a_string() {
        let mut c = cx();
        let tree = el(
            "div",
            vec![(
                "style",
                AttrValue::Style(vec![
                    StyleEntry::Prop { key: "background".into(), value: AttrValue::Expr("props.bg".into()) },
                    StyleEntry::Spread {
                        cond: "props.border != null".into(),
                        props: vec![("border".into(), AttrValue::Expr("props.border".into()))],
                    },
                ]),
            )],
            vec![],
        );
        assert_eq!(
            node(&mut c, &tree, 0).unwrap(),
            "<div style={styleText({ \"background\": props.bg, ...(props.border != null ? { \"border\": props.border } : {}) })}></div>"
        );
        assert!(c.uses_style);
    }

    #[test]
    fn a_map_with_a_separator_guards_it_by_index() {
        let tree = Jsx::Map {
            list: "props.links".into(),
            binder: "item".into(),
            index: "i".into(),
            body: vec![Jsx::Expr("item[1]".into())],
            sep: vec![Jsx::Text(" · ".into())],
        };
        assert_eq!(
            node(&mut cx(), &tree, 0).unwrap(),
            "{#each props.links as item, i}{#if i > 0}{\" · \"}{/if}{item[1]}{/each}"
        );
    }

    #[test]
    fn only_a_head_props_is_renamed() {
        assert_eq!(
            rename_props("props.props.map(x) + item.props.y + $props.z + props.a"),
            "p.props.map(x) + item.props.y + $props.z + p.a"
        );
    }

    #[test]
    fn the_rename_touches_expressions_and_never_text() {
        let mut c = cx();
        c.rename = true;
        let tree = el(
            "p",
            vec![("title", AttrValue::Template(vec![Frag::Lit("props.".into()), Frag::Expr("props.t".into())]))],
            vec![Jsx::Text("see props.x".into()), Jsx::Expr("props.x".into())],
        );
        assert_eq!(
            node(&mut c, &tree, 0).unwrap(),
            "<p title={`props.${p.t}`}\n  >see props.x{p.x}</p\n>"
        );
    }

    #[test]
    fn react_node_becomes_snippet_only_as_a_whole_word() {
        assert_eq!(svelte_ts("ReactNode"), "Snippet");
        assert_eq!(svelte_ts("ReactNode | undefined"), "Snippet | undefined");
        assert_eq!(svelte_ts("ReactNodeList"), "ReactNodeList");
    }

    #[test]
    fn the_index_reader_reads_what_the_index_writer_writes() {
        let index = "export { default as Button } from './Button.svelte';\n\
                     export type { ButtonProps } from './Button.svelte';\n\
                     //   Swatch — quarantined\n";
        assert_eq!(exported_from_index(index), BTreeSet::from(["Button".to_string()]));
    }
}
