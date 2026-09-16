//! Stage: `splice`. See xtask/README.md for the pipeline contract.
//!
//! `render` produced, for every matrix cell, the SSR markup of a zero-prop
//! wrapper whose props were Unicode private-use sentinels. This stage turns that
//! family of markups back into one React component body: a sentinel becomes the
//! prop expression that produced it, and the *differences between cells* become
//! the conditionals, ternaries and `.map()` calls the Yew source expressed as
//! `if`, `match` and `for`.
//!
//! Two rules shape everything here.
//!
//! Nothing is guessed. When two cells differ in a way no matrix axis explains,
//! `splice` fails naming the component, the two cells and the markup that
//! differs, and the component quarantines. A conditional attached to the wrong
//! prop is invisible until runtime, so an error is strictly better than a tree.
//!
//! Sentinel indices are read from the harness plan, never re-derived. They shift
//! per cell as the number of text leaves changes — compare `PropsTable`'s
//! `props_n3__props_item_default_none` (index 7 is `props[2].name`) with
//! `..._some` (index 7 is `props[1].default`) — so the traversal order that
//! assigned them is the harness's to know and this stage's to read.
#![allow(dead_code, unused_imports)]

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use anyhow::{anyhow, bail, Context, Result};
use html5ever::tendril::TendrilSink;
use html5ever::{local_name, namespace_url, ns, parse_fragment, ParseOpts, QualName};
use markup5ever_rcdom::{Handle, NodeData, RcDom};

use crate::harness::{CellPlan, HarnessPlan, SLOT_ATTR, SLOT_TAG};
use crate::ir::{Component, Span, Status, Toolkit};
use crate::matrix::{Cell, Selection};
use crate::render::RenderResult;
use crate::verify::Alphabet;

// ---------------------------------------------------------------- the tree

/// One JSX node. `emit` renders a whole component by wrapping
/// [`Jsx::to_tsx`] in a function declaration; the rendering lives here so that
/// the tree and its spelling cannot drift apart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Jsx {
    /// Literal character data, already entity-decoded. [`Jsx::to_tsx`] decides
    /// whether it is safe as bare JSX text or has to be a string expression.
    Text(String),
    /// `{expr}` — a prop, or anything else expressed as JavaScript.
    Expr(String),
    Element(Element),
    /// Transparent: renders its children in place, with no wrapper.
    Fragment(Vec<Jsx>),
    /// `{cond && (...)}`
    When { cond: String, then: Box<Jsx> },
    /// `{cond ? (...) : (...)}`
    Ternary { cond: String, then: Box<Jsx>, other: Box<Jsx> },
    /// `{list.map((binder, index) => (...))}`. `sep` is the markup Yew emitted
    /// *between* elements; it is rendered guarded by `index > 0`.
    Map {
        list: String,
        binder: String,
        index: String,
        body: Vec<Jsx>,
        sep: Vec<Jsx>,
    },
    /// Renders nothing — the else arm of a ternary whose other side is empty.
    Nothing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    pub tag: String,
    /// SVG elements keep their own attribute spelling (`viewBox`), which the
    /// html5ever tree builder restores for foreign content.
    pub svg: bool,
    pub attrs: Vec<Attr>,
    pub children: Vec<Jsx>,
    /// `key={i}` for an element that is the root of a `.map()` body.
    pub key: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attr {
    /// Already mapped to its React spelling (`className`, `htmlFor`).
    pub name: String,
    pub value: AttrValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttrValue {
    Lit(String),
    Expr(String),
    /// A sentinel embedded in a longer value — `src/atoms/code_block.rs:38`
    /// renders `class={format!("language-{}", props.language)}`, so the value is
    /// a template literal, not a whole-attribute substitution.
    Template(Vec<Frag>),
    /// `style={{ ... }}`.
    Style(Vec<StyleEntry>),
    /// The whole `style` attribute is one prop, as at
    /// `src/molecules/logo_download_card.rs:30` where `preview_style` is an
    /// `AttrValue` of raw CSS text. React rejects a string here, so the emitted
    /// code calls [`STYLE_HELPER`], whose source is [`STYLE_HELPER_TS`].
    StyleText(String),
    /// A boolean attribute present unconditionally.
    True,
    /// `attr={cond ? a : b}`; a `None` fallback renders `undefined`, which React
    /// drops, and is how an attribute present in only some cells is expressed.
    Cond {
        arms: Vec<(String, AttrValue)>,
        fallback: Option<Box<AttrValue>>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frag {
    Lit(String),
    Expr(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StyleEntry {
    Prop { key: String, value: AttrValue },
    /// `...(cond ? { border: x } : {})` — a declaration Yew only emits in some
    /// cells (`src/molecules/bg_check_item.rs` adds `border` under `Some`).
    Spread { cond: String, props: Vec<(String, AttrValue)> },
}

/// The helper `AttrValue::StyleText` calls. `emit` must place [`STYLE_HELPER_TS`]
/// in the generated package's runtime module; there is no way to hand React a
/// CSS string, and inlining the parse at every use site would be unreadable.
pub const STYLE_HELPER: &str = "cssText";

/// Splits on `;` and `:` only at CSS top level — outside `url(...)` and outside
/// quoted strings — because `src/molecules/logo_download_card.rs:30` hands the
/// `preview_style` prop straight to Yew's `style` attribute, and a caller may
/// legally put a `data:` URI or a quoted font name in it. A naive `split(";")`
/// truncated `background-image:url(data:image/svg+xml;base64,AAAA)` at the
/// first semicolon and silently dropped the payload.
///
/// Property names are lowercased before the camel-case fold: CSS property names
/// are case-insensitive and Yew writes the attribute verbatim, but React
/// re-hyphenates before every capital, so an unlowered `BACKGROUND` became
/// `-b-a-c-k-g-r-o-u-n-d` and styled nothing. Custom properties (`--brand`) are
/// left alone — those *are* case-sensitive.
pub const STYLE_HELPER_TS: &str = r#"export function cssText(css: string | undefined): React.CSSProperties {
  const out: Record<string, string> = {};
  const s = css ?? "";
  let depth = 0;
  let quote = "";
  let start = 0;
  let colon = -1;
  const flush = (end: number) => {
    if (colon >= 0) {
      const raw = s.slice(start, colon).trim();
      if (raw) {
        const key = raw.startsWith("--")
          ? raw
          : raw.toLowerCase().replace(/-([a-z])/g, (_, c: string) => c.toUpperCase());
        out[key] = s.slice(colon + 1, end).trim();
      }
    }
    start = end + 1;
    colon = -1;
  };
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (quote) {
      if (c === "\\" ) i++;
      else if (c === quote) quote = "";
    } else if (c === '"' || c === "'") {
      quote = c;
    } else if (c === "(") {
      depth++;
    } else if (c === ")") {
      if (depth > 0) depth--;
    } else if (depth === 0) {
      if (c === ":" && colon < 0) colon = i;
      else if (c === ";") flush(i);
    }
  }
  flush(s.length);
  return out as React.CSSProperties;
}"#;

// ---------------------------------------------------------------- rendering

impl Jsx {
    pub fn to_tsx(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0);
        out
    }

    /// What belongs inside a `return ( ... )`.
    ///
    /// A return takes an *expression*, and child position is not expression
    /// position: `to_tsx` wraps everything that is not an element or text in a
    /// pair of braces, so a root-level ternary would come out as
    /// `return ({cond ? a : b})` — a block containing a labelled statement to
    /// the parser, not a ternary. `src/atoms/button.rs:32-34` and
    /// `src/atoms/pill_button.rs` are the two components in this crate whose
    /// root really is a conditional, because each writes two `html!` blocks.
    ///
    /// Elements and fragments are already valid expressions and are written
    /// unwrapped, so the other 33 components are byte-for-byte unaffected.
    pub fn to_return_body(&self) -> String {
        match self {
            Jsx::Text(_) | Jsx::Element(_) | Jsx::Fragment(_) => self.to_tsx(),
            _ => {
                let mut out = String::new();
                self.write_value(&mut out, 0);
                out
            }
        }
    }

    /// Child position: JSX elements and text stand on their own, everything
    /// else is an embedded expression and gets exactly one pair of braces.
    fn write(&self, out: &mut String, ind: usize) {
        match self {
            Jsx::Text(t) => {
                pad(out, ind);
                if text_needs_expr(t) {
                    out.push('{');
                    out.push_str(&js_string(t));
                    out.push('}');
                } else {
                    out.push_str(t);
                }
            }
            Jsx::Element(e) => e.write(out, ind),
            Jsx::Fragment(items) => write_seq(out, ind, items),
            _ => {
                pad(out, ind);
                out.push('{');
                self.write_value(out, ind);
                out.push('}');
            }
        }
    }

    /// Expression position — inside a ternary arm, a `&&`, or a `.map()` body.
    /// Nothing here may add braces: it is already inside a pair, and a second
    /// one would be an object literal.
    fn write_value(&self, out: &mut String, ind: usize) {
        match self {
            Jsx::Nothing => out.push_str("null"),
            Jsx::Text(t) => out.push_str(&js_string(t)),
            Jsx::Expr(e) => out.push_str(e),
            Jsx::Element(e) => {
                out.push_str("(\n");
                e.write(out, ind + 1);
                out.push('\n');
                pad(out, ind);
                out.push(')');
            }
            Jsx::Fragment(items) => {
                out.push_str("(\n");
                pad(out, ind + 1);
                out.push_str("<>\n");
                write_seq(out, ind + 2, items);
                out.push('\n');
                pad(out, ind + 1);
                out.push_str("</>\n");
                pad(out, ind);
                out.push(')');
            }
            Jsx::When { cond, then } => {
                out.push_str(cond);
                out.push_str(" && ");
                then.write_value(out, ind);
            }
            Jsx::Ternary { cond, then, other } => {
                out.push_str(cond);
                out.push_str(" ? ");
                then.write_value(out, ind);
                out.push_str(" : ");
                other.write_value(out, ind);
            }
            Jsx::Map { list, binder, index, body, sep } => {
                out.push_str(list);
                out.push_str(&format!(".map(({binder}, {index}) => "));
                let mut items: Vec<Jsx> = Vec::new();
                if !sep.is_empty() {
                    items.push(Jsx::When {
                        cond: format!("{index} > 0"),
                        then: Box::new(one(sep.clone())),
                    });
                }
                items.extend(body.iter().cloned());
                if items.len() == 1 {
                    items[0].write_value(out, ind);
                } else {
                    // A multi-node body needs one wrapper to carry the key.
                    out.push_str("(\n");
                    pad(out, ind + 1);
                    out.push_str(&format!("<React.Fragment key={{{index}}}>\n"));
                    write_seq(out, ind + 2, &items);
                    out.push('\n');
                    pad(out, ind + 1);
                    out.push_str("</React.Fragment>\n");
                    pad(out, ind);
                    out.push(')');
                }
                out.push(')');
            }
        }
    }
}

fn write_seq(out: &mut String, ind: usize, items: &[Jsx]) {
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        it.write(out, ind);
    }
}

impl Element {
    fn write(&self, out: &mut String, ind: usize) {
        pad(out, ind);
        out.push('<');
        out.push_str(&self.tag);
        if let Some(k) = &self.key {
            out.push_str(&format!(" key={{{k}}}"));
        }
        for a in &self.attrs {
            out.push(' ');
            out.push_str(&a.name);
            match &a.value {
                AttrValue::True => {}
                other => {
                    out.push('=');
                    out.push_str(&other.to_tsx());
                }
            }
        }
        if self.children.is_empty() {
            out.push_str(" />");
            return;
        }
        out.push_str(">\n");
        write_seq(out, ind + 1, &self.children);
        out.push('\n');
        pad(out, ind);
        out.push_str(&format!("</{}>", self.tag));
    }
}

impl AttrValue {
    pub fn to_tsx(&self) -> String {
        match self {
            AttrValue::Lit(v) => js_string(v),
            AttrValue::Expr(e) => format!("{{{e}}}"),
            AttrValue::Template(frags) => format!("{{{}}}", template(frags)),
            AttrValue::True => "{true}".into(),
            AttrValue::StyleText(e) => format!("{{{STYLE_HELPER}({e})}}"),
            AttrValue::Style(entries) => format!("{{{}}}", style_object(entries)),
            AttrValue::Cond { .. } => format!("{{{}}}", self.as_value()),
        }
    }

    /// The bare JavaScript expression, without the JSX braces — what a ternary
    /// arm or a style value needs.
    fn as_value(&self) -> String {
        match self {
            AttrValue::Lit(v) => js_string(v),
            AttrValue::Expr(e) => e.clone(),
            AttrValue::Template(frags) => template(frags),
            AttrValue::True => "true".into(),
            AttrValue::StyleText(e) => format!("{STYLE_HELPER}({e})"),
            AttrValue::Style(entries) => style_object(entries),
            AttrValue::Cond { arms, fallback } => {
                let mut out = String::new();
                for (cond, v) in arms {
                    out.push_str(&format!("{cond} ? {} : ", v.as_value()));
                }
                out.push_str(&match fallback {
                    Some(f) => f.as_value(),
                    None => "undefined".into(),
                });
                out
            }
        }
    }
}

fn style_object(entries: &[StyleEntry]) -> String {
    let body = entries
        .iter()
        .map(|e| match e {
            StyleEntry::Prop { key, value } => format!("{}: {}", style_key(key), value.as_value()),
            StyleEntry::Spread { cond, props } => {
                let inner = props
                    .iter()
                    .map(|(k, v)| format!("{}: {}", style_key(k), v.as_value()))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("...({cond} ? {{ {inner} }} : {{}})")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("{{ {body} }}")
}

fn template(frags: &[Frag]) -> String {
    let mut out = String::from("`");
    for f in frags {
        match f {
            Frag::Lit(l) => {
                for ch in l.chars() {
                    match ch {
                        '`' => out.push_str("\\`"),
                        '\\' => out.push_str("\\\\"),
                        '$' => out.push_str("\\$"),
                        _ => out.push(ch),
                    }
                }
            }
            Frag::Expr(e) => out.push_str(&format!("${{{e}}}")),
        }
    }
    out.push('`');
    out
}

fn style_key(k: &str) -> String {
    if k.starts_with("--") || !k.contains('-') {
        js_string(k)
    } else {
        js_string(&camel(k))
    }
}

fn pad(out: &mut String, n: usize) {
    for _ in 0..n {
        out.push_str("  ");
    }
}

/// JSX text is not HTML: braces are code, and `&` starts an entity that JSX
/// decodes. Anything ambiguous is emitted as a string expression instead, which
/// is always exact.
fn text_needs_expr(s: &str) -> bool {
    s.trim() != s
        || s.trim().is_empty()
        || s.chars().any(|c| matches!(c, '{' | '}' | '<' | '>' | '&'))
}

fn js_string(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------- names

/// A Rust prop or field name as React spells it. `emit` must use this same
/// function for the props interface, or the generated component reads props
/// nobody passes.
pub fn js_prop_name(rust: &str) -> String {
    camel(rust)
}

fn camel(s: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for ch in s.chars() {
        if ch == '_' || ch == '-' {
            up = !out.is_empty();
        } else if up {
            out.extend(ch.to_uppercase());
            up = false;
        } else {
            out.push(ch);
        }
    }
    out
}

/// The TypeScript union member one `Selection::Variant` corresponds to.
///
/// This must agree with `classify::variant_literals`, which is private to that
/// stage: `variant="class"` means `BadgeVariant::Kind(TermKind::Class)`, so the
/// leaf segment of the payload is the literal, not the wrapper's own name
/// (`src/atoms/onto_badge.rs:20`). `parse` expands the wrapper at parse time, so
/// every recorded variant here carries a concrete payload and maps to exactly
/// one literal.
pub fn variant_literal(variant: &str) -> String {
    let Some(open) = variant.find('(') else {
        return kebab(variant);
    };
    let head = &variant[..open];
    let payload = variant[open + 1..].trim_end_matches(')').trim();
    let leaf = payload.rsplit("::").next().unwrap_or(payload).trim();
    if leaf.is_empty() {
        kebab(head)
    } else {
        kebab(leaf)
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

// ---------------------------------------------------------------- attributes

/// HTML attribute -> React property. Explicit, and exhaustive for this crate:
/// an unknown name is an error rather than a pass-through, because React
/// silently ignores a misspelled property and the page is merely subtly wrong.
const ATTR_MAP: &[(&str, &str)] = &[
    // HTML, the renamed ones first.
    ("class", "className"),
    ("for", "htmlFor"),
    ("tabindex", "tabIndex"),
    ("colspan", "colSpan"),
    ("rowspan", "rowSpan"),
    ("maxlength", "maxLength"),
    ("minlength", "minLength"),
    ("readonly", "readOnly"),
    ("autocomplete", "autoComplete"),
    ("autofocus", "autoFocus"),
    ("crossorigin", "crossOrigin"),
    ("referrerpolicy", "referrerPolicy"),
    ("novalidate", "noValidate"),
    ("datetime", "dateTime"),
    ("contenteditable", "contentEditable"),
    ("spellcheck", "spellCheck"),
    ("srcset", "srcSet"),
    ("srclang", "srcLang"),
    ("enctype", "encType"),
    ("formnovalidate", "formNoValidate"),
    ("accesskey", "accessKey"),
    ("usemap", "useMap"),
    ("frameborder", "frameBorder"),
    ("allowfullscreen", "allowFullScreen"),
    ("playsinline", "playsInline"),
    ("itemprop", "itemProp"),
    ("itemscope", "itemScope"),
    ("itemtype", "itemType"),
    // HTML, unchanged.
    ("id", "id"),
    ("href", "href"),
    ("src", "src"),
    ("alt", "alt"),
    ("title", "title"),
    ("type", "type"),
    ("role", "role"),
    ("scope", "scope"),
    ("style", "style"),
    ("download", "download"),
    ("placeholder", "placeholder"),
    ("name", "name"),
    ("value", "value"),
    ("target", "target"),
    ("rel", "rel"),
    ("width", "width"),
    ("height", "height"),
    ("disabled", "disabled"),
    ("checked", "checked"),
    ("selected", "selected"),
    ("required", "required"),
    ("open", "open"),
    ("inert", "inert"),
    ("hidden", "hidden"),
    ("lang", "lang"),
    ("dir", "dir"),
    ("loading", "loading"),
    ("decoding", "decoding"),
    ("sizes", "sizes"),
    ("media", "media"),
    ("content", "content"),
    ("multiple", "multiple"),
    ("min", "min"),
    ("max", "max"),
    ("step", "step"),
    ("pattern", "pattern"),
    ("rows", "rows"),
    ("cols", "cols"),
    ("wrap", "wrap"),
    ("list", "list"),
    ("form", "form"),
    ("headers", "headers"),
    ("abbr", "abbr"),
    ("cite", "cite"),
    ("start", "start"),
    ("reversed", "reversed"),
    ("controls", "controls"),
    ("autoplay", "autoPlay"),
    ("loop", "loop"),
    ("muted", "muted"),
    ("poster", "poster"),
    ("preload", "preload"),
    ("kind", "kind"),
    ("default", "default"),
    ("accept", "accept"),
    ("capture", "capture"),
    ("action", "action"),
    ("method", "method"),
    ("label", "label"),
    ("slot", "slot"),
    ("draggable", "draggable"),
    ("translate", "translate"),
];

/// SVG attributes reachable from this crate's markup (`src/organisms/hero.rs`
/// renders an inline mesh). Hyphenated SVG names camel-case; the rest keep the
/// spelling html5ever's foreign-content adjustment restored.
const SVG_ATTR_MAP: &[(&str, &str)] = &[
    ("viewBox", "viewBox"),
    ("preserveAspectRatio", "preserveAspectRatio"),
    ("xmlns", "xmlns"),
    ("cx", "cx"),
    ("cy", "cy"),
    ("r", "r"),
    ("rx", "rx"),
    ("ry", "ry"),
    ("x", "x"),
    ("y", "y"),
    ("x1", "x1"),
    ("x2", "x2"),
    ("y1", "y1"),
    ("y2", "y2"),
    ("d", "d"),
    ("points", "points"),
    ("fill", "fill"),
    ("opacity", "opacity"),
    ("stroke", "stroke"),
    ("transform", "transform"),
    ("offset", "offset"),
    ("mask", "mask"),
    ("filter", "filter"),
    ("gradientUnits", "gradientUnits"),
    ("gradientTransform", "gradientTransform"),
    ("patternUnits", "patternUnits"),
    ("markerWidth", "markerWidth"),
    ("markerHeight", "markerHeight"),
    ("refX", "refX"),
    ("refY", "refY"),
    ("clipPath", "clipPath"),
    ("stroke-width", "strokeWidth"),
    ("stroke-opacity", "strokeOpacity"),
    ("stroke-linecap", "strokeLinecap"),
    ("stroke-linejoin", "strokeLinejoin"),
    ("stroke-dasharray", "strokeDasharray"),
    ("stroke-dashoffset", "strokeDashoffset"),
    ("fill-opacity", "fillOpacity"),
    ("fill-rule", "fillRule"),
    ("clip-rule", "clipRule"),
    ("clip-path", "clipPath"),
    ("stop-color", "stopColor"),
    ("stop-opacity", "stopOpacity"),
    ("text-anchor", "textAnchor"),
    ("dominant-baseline", "dominantBaseline"),
    ("font-size", "fontSize"),
    ("font-family", "fontFamily"),
    ("font-weight", "fontWeight"),
];

/// Attributes whose HTML form is `name="name"` and whose React form is a
/// boolean. Yew renders `inert="inert"`, so presence — not the value — is the
/// signal (`src/molecules/modal.rs:22`, `src/molecules/nav_dropdown.rs:35`).
/// `aria-*` is deliberately absent: `aria-expanded="false"` is the string
/// `"false"` in React, not the boolean.
const BOOL_ATTRS: &[&str] = &[
    "disabled", "checked", "selected", "readonly", "required", "multiple", "autofocus",
    "autoplay", "controls", "loop", "muted", "novalidate", "open", "hidden", "inert",
    "default", "reversed", "async", "defer", "itemscope", "playsinline", "formnovalidate",
    "allowfullscreen",
];

fn react_attr(name: &str, svg: bool, component: &str, tag: &str) -> Result<String> {
    if name.starts_with("aria-") || name.starts_with("data-") {
        return Ok(name.to_string());
    }
    if svg {
        if let Some((_, react)) = SVG_ATTR_MAP.iter().find(|(h, _)| *h == name) {
            return Ok((*react).to_string());
        }
    }
    if let Some((_, react)) = ATTR_MAP.iter().find(|(h, _)| *h == name) {
        return Ok((*react).to_string());
    }
    bail!(
        "{component}: no React spelling is registered for the `{name}` attribute on <{tag}>; \
         add it to splice::ATTR_MAP (or SVG_ATTR_MAP) rather than letting it through — React \
         drops an unknown property silently"
    )
}

// ---------------------------------------------------------------- parsing

/// One parsed node of a cell's markup, with the sentinels still present as
/// indices into the cell's plan.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Node {
    Text(Vec<Piece>),
    /// `<template data-eona-slot="N">` — where a `Slot` prop was rendered.
    Slot(usize),
    Elem(ElemPat),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ElemPat {
    tag: String,
    svg: bool,
    attrs: Vec<(String, Vec<Piece>)>,
    children: Vec<Node>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Piece {
    Lit(String),
    /// Sentinel index; the prop it stands for is in the cell's plan.
    Ref(usize),
}

fn parse_html(html: &str, component: &str, cell: &str) -> Result<Vec<Node>> {
    let dom = parse_fragment(
        RcDom::default(),
        ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("body")),
        vec![],
    )
    .one(html.to_string());

    // `parse_fragment` wraps the fragment in a synthetic `html` element.
    let doc = dom.document.children.borrow();
    let root = doc
        .first()
        .ok_or_else(|| anyhow!("{component}/{cell}: the parser produced no nodes"))?;
    let mut out = Vec::new();
    for child in root.children.borrow().iter() {
        if let Some(n) = convert(child, component, cell)? {
            out.push(n);
        }
    }
    if out.is_empty() {
        bail!("{component}/{cell}: rendered no markup at all");
    }
    Ok(out)
}

fn convert(h: &Handle, component: &str, cell: &str) -> Result<Option<Node>> {
    match &h.data {
        NodeData::Text { contents } => {
            Ok(Some(Node::Text(pieces(&contents.borrow(), component, cell)?)))
        }
        // Yew's `hydratable(false)` renderer emits no comments; anything here is
        // not ours and dropping it would be a silent edit.
        NodeData::Comment { contents } => {
            bail!("{component}/{cell}: unexpected comment in the SSR output: {contents:?}")
        }
        NodeData::Element { name, attrs, .. } => {
            let svg = name.ns == ns!(svg);
            let tag = name.local.to_string();
            let mut pairs = Vec::new();
            for a in attrs.borrow().iter() {
                if let Some(prefix) = &a.name.prefix {
                    bail!(
                        "{component}/{cell}: namespaced attribute `{prefix}:{}` on <{tag}> has no \
                         React spelling",
                        a.name.local
                    );
                }
                pairs.push((a.name.local.to_string(), pieces(&a.value, component, cell)?));
            }
            if tag == SLOT_TAG {
                if let Some((_, v)) = pairs.iter().find(|(n, _)| n == SLOT_ATTR) {
                    let text: String = v
                        .iter()
                        .map(|p| match p {
                            Piece::Lit(l) => l.clone(),
                            Piece::Ref(_) => String::new(),
                        })
                        .collect();
                    let idx: usize = text.trim().parse().with_context(|| {
                        format!("{component}/{cell}: slot marker index {text:?} is not a number")
                    })?;
                    return Ok(Some(Node::Slot(idx)));
                }
            }
            let mut children = Vec::new();
            for c in h.children.borrow().iter() {
                if let Some(n) = convert(c, component, cell)? {
                    children.push(n);
                }
            }
            Ok(Some(Node::Elem(ElemPat { tag, svg, attrs: pairs, children })))
        }
        NodeData::Document | NodeData::Doctype { .. } | NodeData::ProcessingInstruction { .. } => {
            Ok(None)
        }
    }
}

/// Split text on well-formed sentinels. A bare delimiter means the component
/// took a prop apart (`src/molecules/dataset_card.rs:43`); `verify` quarantines
/// for that, and reaching here means the gate let one through.
fn pieces(s: &str, component: &str, cell: &str) -> Result<Vec<Piece>> {
    let (open, close) = (Alphabet::Primary.open(), Alphabet::Primary.close());
    let mut out: Vec<Piece> = Vec::new();
    let mut lit = String::new();
    let mut rest = s;
    while let Some(at) = rest.find(open) {
        lit.push_str(&rest[..at]);
        let after = &rest[at + open.len_utf8()..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        let tail = &after[digits.len()..];
        match (digits.parse::<usize>(), tail.strip_prefix(close)) {
            (Ok(id), Some(tail)) => {
                if !lit.is_empty() {
                    out.push(Piece::Lit(std::mem::take(&mut lit)));
                }
                out.push(Piece::Ref(id));
                rest = tail;
            }
            _ => bail!(
                "{component}/{cell}: a bare sentinel delimiter at byte {at} of {s:?} — the \
                 component split a prop apart, so no splice of it can be faithful"
            ),
        }
    }
    lit.push_str(rest);
    if !lit.is_empty() || out.is_empty() {
        out.push(Piece::Lit(lit));
    }
    Ok(out)
}

// ---------------------------------------------------------------- scopes

/// A `.map()` binding in force: inside it, `values[2].language` and the axis
/// `values[].language` both name `item.language`.
#[derive(Debug)]
struct Scope {
    parent: Option<Rc<Scope>>,
    /// `values[2]` — the concrete prefix the harness plan uses.
    concrete: String,
    /// `values[]` — the element-uniform prefix the matrix uses.
    axis: String,
    binder: String,
}

fn strip_path<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = path.strip_prefix(prefix)?;
    if rest.is_empty() || rest.starts_with('.') || rest.starts_with('[') {
        Some(rest)
    } else {
        None
    }
}

fn resolve(path: &str, scope: Option<&Rc<Scope>>) -> String {
    let mut cur = scope;
    while let Some(s) = cur {
        if let Some(rest) = strip_path(path, &s.concrete).or_else(|| strip_path(path, &s.axis)) {
            return format!("{}{}", s.binder, js_suffix(rest));
        }
        cur = s.parent.as_ref();
    }
    let head_end = path.find(['.', '[']).unwrap_or(path.len());
    format!("props.{}{}", camel(&path[..head_end]), js_suffix(&path[head_end..]))
}

/// `.name` -> `.name`, `.0` -> `[0]` (a tuple field, `src/molecules/nav_dropdown.rs:18`),
/// `[2]` -> `[2]`, `[]` -> nothing (already bound by the enclosing `.map()`).
fn js_suffix(rest: &str) -> String {
    let mut out = String::new();
    let mut r = rest;
    while !r.is_empty() {
        if let Some(tail) = r.strip_prefix('[') {
            let end = tail.find(']').unwrap_or(tail.len());
            let idx = &tail[..end];
            if !idx.is_empty() {
                out.push_str(&format!("[{idx}]"));
            }
            r = &tail[(end + 1).min(tail.len())..];
        } else if let Some(tail) = r.strip_prefix('.') {
            let end = tail.find(['.', '[']).unwrap_or(tail.len());
            let seg = &tail[..end];
            if seg.chars().all(|c| c.is_ascii_digit()) && !seg.is_empty() {
                out.push_str(&format!("[{seg}]"));
            } else {
                out.push('.');
                out.push_str(&camel(seg));
            }
            r = &tail[end..];
        } else {
            break;
        }
    }
    out
}

// ---------------------------------------------------------------- context

struct CellCtx<'a> {
    cell: &'a Cell,
    /// Sentinel index -> the concrete prop path the harness wrote there.
    binds: BTreeMap<usize, (&'a str, &'a Span)>,
    slots: BTreeMap<usize, &'a str>,
    roots: Vec<Node>,
}

struct Ctx<'a> {
    component: &'a str,
    cells: Vec<CellCtx<'a>>,
}

impl<'a> Ctx<'a> {
    fn path_of(&self, cell: usize, idx: usize) -> Result<&'a str> {
        self.cells[cell]
            .binds
            .get(&idx)
            .map(|(p, _)| *p)
            .ok_or_else(|| {
                anyhow!(
                    "{}/{}: sentinel {idx} is in the markup but not in the harness plan",
                    self.component,
                    self.cells[cell].cell.id
                )
            })
    }

    fn slot_of(&self, cell: usize, idx: usize) -> Result<&'a str> {
        self.cells[cell].slots.get(&idx).copied().ok_or_else(|| {
            anyhow!(
                "{}/{}: slot marker {idx} is in the markup but not in the harness plan",
                self.component,
                self.cells[cell].cell.id
            )
        })
    }

    fn id(&self, cell: usize) -> &str {
        &self.cells[cell].cell.id
    }

    /// Axis paths usable as a condition for this group: present in every cell of
    /// it, ordered so a dominator splits before what it dominates.
    ///
    /// Coverage over the *whole* component is the dominance signal:
    /// `DatasetCard.badge_class` only exists in cells where `badge` is `Some`
    /// (`src/molecules/dataset_card.rs:33,36`), so it covers fewer cells and
    /// sorts after `badge`. Splitting the other way round would produce a flat
    /// branch table with an unsatisfiable arm.
    fn axes(&self, group: &[usize]) -> Vec<String> {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for c in &self.cells {
            for ch in &c.cell.choices {
                *counts.entry(ch.path.to_string()).or_default() += 1;
            }
        }
        let mut out: Vec<String> = counts
            .keys()
            .filter(|p| group.iter().all(|&i| self.cells[i].cell.get(p).is_some()))
            .cloned()
            .collect();
        out.sort_by(|a, b| {
            counts[b]
                .cmp(&counts[a])
                .then(a.matches(['.', '[']).count().cmp(&b.matches(['.', '[']).count()))
                .then(a.cmp(b))
        });
        out
    }
}

// ---------------------------------------------------------------- entry point

/// Splice one component's cells into a single JSX tree.
///
/// `plan` is the harness's own record of which sentinel index carried which
/// prop. It is an addition to the signature the brief gave (`component`,
/// `cells`): the index -> path map is assigned by `harness`'s traversal and
/// re-deriving it here would be a second implementation of that traversal, free
/// to disagree with the one that actually planted the text.
pub fn splice(component: &Component, cells: &[(Cell, String)], plan: &HarnessPlan) -> Result<Jsx> {
    if cells.is_empty() {
        bail!("{}: no rendered cells to splice", component.name);
    }
    let mut ctx = Ctx { component: &component.name, cells: Vec::new() };
    for (cell, html) in cells {
        let key = format!("{}/{}", component.name, cell.id);
        let p: &CellPlan = plan
            .cell(&key)
            .ok_or_else(|| anyhow!("{key}: rendered, but the harness plan has no such cell"))?;
        ctx.cells.push(CellCtx {
            cell,
            binds: p.sentinels.iter().map(|b| (b.index, (b.path.as_str(), &b.span))).collect(),
            slots: p.slots.iter().map(|b| (b.index, b.path.as_str())).collect(),
            roots: parse_html(html, &component.name, &cell.id)?,
        });
    }

    let group: Vec<SrcList> = (0..ctx.cells.len())
        .map(|i| SrcList { cell: i, nodes: &ctx.cells[i].roots[..], scope: None })
        .collect();
    let roots = align(&ctx, &group)
        .with_context(|| format!("splicing {}", component.name))?;
    Ok(one(roots))
}

/// Every component the verify gate passed, spliced. Quarantined and excluded
/// components are skipped by status, and a component whose cells cannot be
/// reconciled is returned as a failure rather than aborting the run — one
/// unsplicable component is a quarantine, not a broken pipeline.
pub fn splice_toolkit(
    tk: &Toolkit,
    cells: &BTreeMap<String, Vec<Cell>>,
    plan: &HarnessPlan,
    renders: &BTreeMap<String, RenderResult>,
) -> Result<SpliceReport> {
    let mut report = SpliceReport::default();
    for c in &tk.components {
        match c.status {
            Status::Ok | Status::NeedsOverride { .. } => {}
            Status::Quarantined { .. } | Status::Excluded { .. } => continue,
        }
        let Some(list) = cells.get(&c.name) else { continue };
        let mut pairs: Vec<(Cell, String)> = Vec::new();
        let mut missing = Vec::new();
        for cell in list {
            let key = format!("{}/{}#{}", c.name, cell.id, Alphabet::Primary.name());
            match renders.get(&key).and_then(|r| r.html()) {
                Some(html) => pairs.push((cell.clone(), html.to_string())),
                None => missing.push(cell.id.clone()),
            }
        }
        if !missing.is_empty() {
            report.failures.push((
                c.name.clone(),
                format!(
                    "no markup for {} cell(s): {} — {}:{}",
                    missing.len(),
                    missing.join(", "),
                    c.span.file,
                    c.span.line
                ),
            ));
            continue;
        }
        match splice(c, &pairs, plan) {
            Ok(jsx) => report.components.push((c.name.clone(), jsx)),
            Err(e) => report.failures.push((c.name.clone(), format!("{e:#}"))),
        }
    }
    Ok(report)
}

#[derive(Default)]
pub struct SpliceReport {
    pub components: Vec<(String, Jsx)>,
    /// Component -> why it could not be spliced. Each of these quarantines.
    pub failures: Vec<(String, String)>,
}

// ---------------------------------------------------------------- the diff

#[derive(Clone)]
struct Src<'a> {
    cell: usize,
    node: &'a Node,
    scope: Option<Rc<Scope>>,
}

#[derive(Clone)]
struct SrcList<'a> {
    cell: usize,
    nodes: &'a [Node],
    scope: Option<Rc<Scope>>,
}

/// A node's shape, for lining up two cells' child lists. Tag plus the literal
/// part of `class` is enough to tell `MainNav`'s back link from an ordinary one
/// (`src/organisms/main_nav.rs`) without being so specific that a conditional
/// attribute breaks the match.
fn key(n: &Node) -> String {
    match n {
        Node::Text(p) => format!("t:{}", skeleton(p)),
        Node::Slot(i) => format!("s:{i}"),
        Node::Elem(e) => {
            let class = e
                .attrs
                .iter()
                .find(|(k, _)| k == "class")
                .map(|(_, v)| skeleton(v))
                .unwrap_or_default();
            format!("e:{}:{class}", e.tag)
        }
    }
}

/// A node's full fingerprint, with every sentinel already resolved to the
/// expression it stands for. Two cells whose subtrees have the same signature
/// render identically, whatever axis separates them.
fn sig(ctx: &Ctx, s: &Src) -> Result<String> {
    fn walk(ctx: &Ctx, cell: usize, n: &Node, scope: Option<&Rc<Scope>>, out: &mut String) -> Result<()> {
        match n {
            Node::Text(p) => {
                out.push_str("t(");
                for f in frags_of(ctx, cell, p, scope)? {
                    match f {
                        Frag::Lit(l) => out.push_str(&format!("{l:?}")),
                        Frag::Expr(e) => out.push_str(&format!("${{{e}}}")),
                    }
                }
                out.push(')');
            }
            Node::Slot(i) => out.push_str(&format!("slot({})", ctx.slot_of(cell, *i)?)),
            Node::Elem(e) => {
                out.push_str(&format!("<{}", e.tag));
                for (k, v) in &e.attrs {
                    out.push_str(&format!(" {k}="));
                    for f in frags_of(ctx, cell, v, scope)? {
                        match f {
                            Frag::Lit(l) => out.push_str(&format!("{l:?}")),
                            Frag::Expr(x) => out.push_str(&format!("${{{x}}}")),
                        }
                    }
                }
                out.push('>');
                for c in &e.children {
                    walk(ctx, cell, c, scope, out)?;
                }
                out.push_str(&format!("</{}>", e.tag));
            }
        }
        Ok(())
    }
    let mut out = String::new();
    walk(ctx, s.cell, s.node, s.scope.as_ref(), &mut out)?;
    Ok(out)
}

fn skeleton(p: &[Piece]) -> String {
    p.iter()
        .map(|x| match x {
            Piece::Lit(l) => l.clone(),
            Piece::Ref(_) => "\u{1}".into(),
        })
        .collect()
}

fn one(mut items: Vec<Jsx>) -> Jsx {
    match items.len() {
        0 => Jsx::Nothing,
        1 => items.pop().unwrap(),
        _ => Jsx::Fragment(items),
    }
}

/// Reconcile one node position across every cell in the group.
fn unify(ctx: &Ctx, group: &[Src]) -> Result<Jsx> {
    let first = group[0].node;
    match first {
        Node::Slot(_) => {
            let mut paths = BTreeSet::new();
            for s in group {
                let Node::Slot(i) = s.node else { return split_node(ctx, group) };
                paths.insert(ctx.slot_of(s.cell, *i)?);
            }
            if paths.len() != 1 {
                bail!(
                    "{}: one position renders different slots across cells ({})",
                    ctx.component,
                    paths.into_iter().collect::<Vec<_>>().join(", ")
                );
            }
            let path = paths.into_iter().next().unwrap();
            Ok(Jsx::Expr(if path == "children" {
                "props.children".into()
            } else {
                resolve(path, group[0].scope.as_ref())
            }))
        }
        Node::Text(_) => {
            let mut frags: Option<Vec<Frag>> = None;
            for s in group {
                let Node::Text(p) = s.node else { return split_node(ctx, group) };
                let f = frags_of(ctx, s.cell, p, s.scope.as_ref())?;
                match &frags {
                    None => frags = Some(f),
                    Some(prev) if *prev == f => {}
                    Some(_) => return split_node(ctx, group),
                }
            }
            Ok(one(frags
                .unwrap()
                .into_iter()
                .map(|f| match f {
                    Frag::Lit(l) => Jsx::Text(l),
                    Frag::Expr(e) => Jsx::Expr(e),
                })
                .collect()))
        }
        Node::Elem(e0) => {
            let mut elems = Vec::with_capacity(group.len());
            for s in group {
                match s.node {
                    Node::Elem(e) if e.tag == e0.tag && e.svg == e0.svg => {
                        elems.push((s.cell, s.scope.clone(), e))
                    }
                    _ => return split_node(ctx, group),
                }
            }
            let attrs = unify_attrs(ctx, &elems)?;
            let kids: Vec<SrcList> = elems
                .iter()
                .map(|(c, sc, e)| SrcList { cell: *c, nodes: &e.children[..], scope: sc.clone() })
                .collect();
            let children = align(ctx, &kids)?;
            Ok(Jsx::Element(Element {
                tag: e0.tag.clone(),
                svg: e0.svg,
                attrs,
                children,
                key: None,
            }))
        }
    }
}

/// The cells at this position disagree about what the node even is. Find the
/// axis that explains the disagreement and emit a ternary here — as deep in the
/// tree as the difference actually is.
fn split_node(ctx: &Ctx, group: &[Src]) -> Result<Jsx> {
    // `key` is the shape, which is what separates a `<button>` cell from an
    // `<a>` one. It is deliberately blind to which prop a leaf carries, so when
    // it cannot tell the cells apart — `annotation.rs:35` renders the same
    // `<span class="badge">` from `language` in one cell and `datatype` in
    // another — fall back to the fully resolved fingerprint. Without the
    // fallback the single class would be the whole group and this would recurse
    // into itself forever.
    let mut classes = classify_by(group, |s| key(s.node));
    if classes.len() < 2 {
        let mut sigs = Vec::with_capacity(group.len());
        for s in group {
            sigs.push(sig(ctx, s)?);
        }
        classes = classify_by(&sigs, |s| s.clone());
    }
    if classes.len() < 2 {
        bail!(
            "{}: cells {} render the same markup here, so the difference that stopped them \
             unifying is deeper than this node",
            ctx.component,
            group.iter().map(|s| ctx.id(s.cell)).collect::<Vec<_>>().join(", ")
        );
    }
    let cell_classes: Vec<Vec<usize>> =
        classes.iter().map(|c| c.iter().map(|&i| group[i].cell).collect()).collect();
    let cells: Vec<usize> = group.iter().map(|s| s.cell).collect();
    let (axis, sels) = explain(ctx, &cells, &cell_classes).ok_or_else(|| {
        anyhow!(
            "{}: cells {} render different markup here and no matrix axis explains it — {}",
            ctx.component,
            classes
                .iter()
                .map(|c| ctx.id(group[c[0]].cell).to_string())
                .collect::<Vec<_>>()
                .join(" vs "),
            classes
                .iter()
                .map(|c| format!("{:?}", key(group[c[0]].node)))
                .collect::<Vec<_>>()
                .join(" vs ")
        )
    })?;
    let mut arms: Vec<(Vec<Selection>, Jsx)> = Vec::new();
    for (ci, class) in classes.iter().enumerate() {
        let sub: Vec<Src> = class.iter().map(|&i| group[i].clone()).collect();
        arms.push((sels[ci].clone(), unify(ctx, &sub)?));
    }
    chain(ctx, &axis, group[0].scope.as_ref(), arms)
}

/// Build the ternary chain for a multi-way split. The class holding the lowest
/// cell index — the matrix's baseline cell — becomes the final `else`, so the
/// generated code reads as "the default, unless".
fn chain(
    ctx: &Ctx,
    axis: &str,
    scope: Option<&Rc<Scope>>,
    mut arms: Vec<(Vec<Selection>, Jsx)>,
) -> Result<Jsx> {
    if arms.len() == 1 {
        return Ok(arms.pop().unwrap().1);
    }
    // `classify_by` keeps first-seen order, so arms[0] holds the baseline cell.
    let mut out = arms.remove(0).1;
    for (sels, jsx) in arms.into_iter().rev() {
        let cond = cond_for(ctx, axis, &sels, scope)?;
        out = match (&jsx, &out) {
            (_, Jsx::Nothing) => Jsx::When { cond, then: Box::new(jsx) },
            _ => Jsx::Ternary { cond, then: Box::new(jsx), other: Box::new(out) },
        };
    }
    Ok(out)
}

/// Partition the group, keeping first-seen order so the baseline cell's class
/// stays first.
fn classify_by<T, F: Fn(&T) -> String>(items: &[T], f: F) -> Vec<Vec<usize>> {
    let mut keys: Vec<String> = Vec::new();
    let mut out: Vec<Vec<usize>> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let k = f(it);
        match keys.iter().position(|x| *x == k) {
            Some(p) => out[p].push(i),
            None => {
                keys.push(k);
                out.push(vec![i]);
            }
        }
    }
    out
}

/// Find one axis whose selections partition `cells` exactly the way `classes`
/// does. Returns the axis and, per class, the selections that reach it.
fn explain(
    ctx: &Ctx,
    cells: &[usize],
    classes: &[Vec<usize>],
) -> Option<(String, Vec<Vec<Selection>>)> {
    'axis: for axis in ctx.axes(cells) {
        let mut per_class: Vec<Vec<Selection>> = vec![Vec::new(); classes.len()];
        let mut seen: Vec<(Selection, usize)> = Vec::new();
        for (ci, class) in classes.iter().enumerate() {
            for &cell in class {
                let sel = ctx.cells[cell].cell.get(&axis)?.clone();
                if let Some((_, other)) = seen.iter().find(|(s, _)| *s == sel) {
                    if *other != ci {
                        continue 'axis;
                    }
                } else {
                    seen.push((sel.clone(), ci));
                    per_class[ci].push(sel);
                }
            }
        }
        if per_class.iter().any(|s| s.is_empty()) {
            continue;
        }
        return Some((axis, per_class));
    }
    None
}

/// The JavaScript test for "this cell's selection at `axis` is one of `sels`".
///
/// Only two arity predicates are sound. The matrix renders lists at lengths 0..3
/// but a caller may pass seven elements, so `length === 2` would be a claim the
/// renders never supported; empty versus non-empty is all the evidence licenses.
fn cond_for(ctx: &Ctx, axis: &str, sels: &[Selection], scope: Option<&Rc<Scope>>) -> Result<String> {
    let expr = resolve(axis, scope);
    let all_variants = sels.iter().all(|s| matches!(s, Selection::Variant(_)));
    if all_variants {
        let lits: Vec<String> = sels
            .iter()
            .map(|s| match s {
                Selection::Variant(v) => variant_literal(v),
                _ => unreachable!(),
            })
            .collect();
        return Ok(if lits.len() == 1 {
            format!("{expr} === {}", js_string(&lits[0]))
        } else {
            format!(
                "[{}].includes({expr})",
                lits.iter().map(|l| js_string(l)).collect::<Vec<_>>().join(", ")
            )
        });
    }
    if sels.len() == 1 {
        return match &sels[0] {
            Selection::Present => Ok(format!("{expr} != null")),
            Selection::Absent => Ok(format!("{expr} == null")),
            Selection::Bool(true) => Ok(expr),
            Selection::Bool(false) => Ok(format!("!{expr}")),
            Selection::Arity(0) => Ok(format!("{expr}.length === 0")),
            Selection::Arity(n) => bail!(
                "{}: a branch would have to test `{axis}` for exactly {n} elements, which the \
                 0..3 render matrix cannot justify for a list of any length",
                ctx.component
            ),
            Selection::Variant(_) => unreachable!(),
        };
    }
    if sels.iter().all(|s| matches!(s, Selection::Arity(n) if *n > 0)) {
        return Ok(format!("{expr}.length > 0"));
    }
    bail!(
        "{}: no single condition covers the selections {:?} at `{axis}`",
        ctx.component,
        sels.iter().map(|s| s.to_string()).collect::<Vec<_>>()
    )
}

// ---------------------------------------------------------------- sequences

fn align(ctx: &Ctx, group: &[SrcList]) -> Result<Vec<Jsx>> {
    if group.is_empty() {
        return Ok(Vec::new());
    }
    let mut start = vec![0usize; group.len()];
    let mut end: Vec<usize> = group.iter().map(|g| g.nodes.len()).collect();
    let mut prefix: Vec<Jsx> = Vec::new();
    let mut suffix: Vec<Jsx> = Vec::new();

    loop {
        if (0..group.len()).any(|i| start[i] >= end[i]) {
            break;
        }
        let k0 = key(&group[0].nodes[start[0]]);
        if (1..group.len()).any(|i| key(&group[i].nodes[start[i]]) != k0) {
            break;
        }
        let heads: Vec<Src> = (0..group.len())
            .map(|i| Src {
                cell: group[i].cell,
                node: &group[i].nodes[start[i]],
                scope: group[i].scope.clone(),
            })
            .collect();
        push(&mut prefix, unify(ctx, &heads)?);
        for s in start.iter_mut() {
            *s += 1;
        }
    }

    loop {
        if (0..group.len()).any(|i| start[i] >= end[i]) {
            break;
        }
        let k0 = key(&group[0].nodes[end[0] - 1]);
        if (1..group.len()).any(|i| key(&group[i].nodes[end[i] - 1]) != k0) {
            break;
        }
        let tails: Vec<Src> = (0..group.len())
            .map(|i| Src {
                cell: group[i].cell,
                node: &group[i].nodes[end[i] - 1],
                scope: group[i].scope.clone(),
            })
            .collect();
        let mut tail = Vec::new();
        push(&mut tail, unify(ctx, &tails)?);
        tail.extend(std::mem::take(&mut suffix));
        suffix = tail;
        for e in end.iter_mut() {
            *e -= 1;
        }
    }

    let middle: Vec<SrcList> = (0..group.len())
        .map(|i| SrcList {
            cell: group[i].cell,
            nodes: &group[i].nodes[start[i]..end[i]],
            scope: group[i].scope.clone(),
        })
        .collect();
    prefix.extend(reconcile(ctx, &middle)?);
    prefix.extend(suffix);
    Ok(prefix)
}

fn push(out: &mut Vec<Jsx>, j: Jsx) {
    match j {
        // A text node carrying a sentinel unifies to several JSX nodes; a
        // fragment here is a grouping artefact, not structure.
        Jsx::Fragment(items) => out.extend(items),
        Jsx::Nothing => {}
        other => out.push(other),
    }
}

/// The part of a child list the cells do not agree on.
fn reconcile(ctx: &Ctx, group: &[SrcList]) -> Result<Vec<Jsx>> {
    if group.iter().all(|g| g.nodes.is_empty()) {
        return Ok(Vec::new());
    }
    let len = group[0].nodes.len();
    if group.iter().all(|g| g.nodes.len() == len) {
        let mut out = Vec::new();
        let mut ok = true;
        for j in 0..len {
            let col: Vec<Src> = group
                .iter()
                .map(|g| Src { cell: g.cell, node: &g.nodes[j], scope: g.scope.clone() })
                .collect();
            match unify(ctx, &col) {
                Ok(j) => push(&mut out, j),
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return Ok(out);
        }
    }
    if let Some(list) = try_list(ctx, group)? {
        return Ok(list);
    }
    split_seq(ctx, group)
}

/// Recover a `.map()`: the region repeats once per element of some list prop.
fn try_list(ctx: &Ctx, group: &[SrcList]) -> Result<Option<Vec<Jsx>>> {
    let cells: Vec<usize> = group.iter().map(|g| g.cell).collect();
    for axis in ctx.axes(&cells) {
        let mut arity = Vec::with_capacity(group.len());
        let mut usable = true;
        for g in group {
            match ctx.cells[g.cell].cell.get(&axis) {
                Some(Selection::Arity(n)) => arity.push(*n),
                _ => {
                    usable = false;
                    break;
                }
            }
        }
        if !usable || arity.iter().collect::<BTreeSet<_>>().len() < 2 {
            continue;
        }
        // len(n) = k*n + sep*(n-1) for n >= 1, and 0 for n == 0: `align` has
        // already stripped everything outside the repeating region, so a
        // non-zero length at arity 0 means this axis is not what varies here.
        let mut by_n: BTreeMap<usize, usize> = BTreeMap::new();
        let mut consistent = true;
        for (i, g) in group.iter().enumerate() {
            if *by_n.entry(arity[i]).or_insert(g.nodes.len()) != g.nodes.len() {
                consistent = false;
                break;
            }
        }
        if !consistent || by_n.get(&0).copied().unwrap_or(0) != 0 {
            continue;
        }
        let positive: Vec<(usize, usize)> =
            by_n.iter().filter(|(n, _)| **n > 0).map(|(n, l)| (*n, *l)).collect();
        if positive.is_empty() {
            continue;
        }
        let (n1, l1) = positive[0];
        let (k, sep) = if positive.len() >= 2 {
            let (n2, l2) = positive[1];
            if l2 < l1 || (l2 - l1) % (n2 - n1) != 0 {
                continue;
            }
            let step = (l2 - l1) / (n2 - n1);
            if step * n1 < l1 {
                continue;
            }
            let k = l1 - step * (n1 - 1);
            (k, step.saturating_sub(k))
        } else {
            if l1 % n1 != 0 {
                continue;
            }
            (l1 / n1, 0)
        };
        if k == 0 || positive.iter().any(|(n, l)| *l != k * n + sep * (n - 1)) {
            continue;
        }

        // The slices have to belong to the elements they sit at: a sentinel in
        // element 2's slice must be a `<axis>[2]` path. Without this check a
        // static run of k nodes could be mistaken for a list body.
        let mut items: Vec<SrcList> = Vec::new();
        let mut seps: Vec<SrcList> = Vec::new();
        let concrete_root = axis.trim_end_matches("[]").to_string();
        for (i, g) in group.iter().enumerate() {
            for e in 0..arity[i] {
                let at = e * (k + sep);
                for idx in refs_in(&g.nodes[at..at + k]) {
                    let path = ctx.path_of(g.cell, idx)?;
                    if let Some(rest) = path.strip_prefix(&concrete_root) {
                        let want = format!("[{e}]");
                        if rest.starts_with('[') && !rest.starts_with(&want) {
                            usable = false;
                        }
                    }
                }
                let scope = Rc::new(Scope {
                    parent: g.scope.clone(),
                    concrete: format!("{concrete_root}[{e}]"),
                    axis: format!("{concrete_root}[]"),
                    binder: binder_for(g.scope.as_ref()),
                });
                items.push(SrcList {
                    cell: g.cell,
                    nodes: &g.nodes[at..at + k],
                    scope: Some(scope.clone()),
                });
                if sep > 0 && e + 1 < arity[i] {
                    seps.push(SrcList {
                        cell: g.cell,
                        nodes: &g.nodes[at + k..at + k + sep],
                        scope: Some(scope),
                    });
                }
            }
        }
        if !usable || items.is_empty() {
            continue;
        }
        let binder = binder_for(group[0].scope.as_ref());
        let index = index_for(group[0].scope.as_ref());
        let mut body = align(ctx, &items)?;
        let sep_body = if seps.is_empty() { Vec::new() } else { align(ctx, &seps)? };
        if body.len() == 1 {
            if let Jsx::Element(e) = &mut body[0] {
                e.key = Some(index.clone());
            }
        }
        return Ok(Some(vec![Jsx::Map {
            list: resolve(&concrete_root, group[0].scope.as_ref()),
            binder,
            index,
            body,
            sep: sep_body,
        }]));
    }
    Ok(None)
}

fn binder_for(scope: Option<&Rc<Scope>>) -> String {
    match depth(scope) {
        0 => "item".into(),
        n => format!("item{}", n + 1),
    }
}

fn index_for(scope: Option<&Rc<Scope>>) -> String {
    match depth(scope) {
        0 => "i".into(),
        n => format!("i{}", n + 1),
    }
}

fn depth(scope: Option<&Rc<Scope>>) -> usize {
    let mut n = 0;
    let mut cur = scope;
    while let Some(s) = cur {
        n += 1;
        cur = s.parent.as_ref();
    }
    n
}

fn refs_in(nodes: &[Node]) -> Vec<usize> {
    fn walk(n: &Node, out: &mut Vec<usize>) {
        match n {
            Node::Text(p) => out.extend(p.iter().filter_map(|x| match x {
                Piece::Ref(i) => Some(*i),
                Piece::Lit(_) => None,
            })),
            Node::Slot(_) => {}
            Node::Elem(e) => {
                for (_, v) in &e.attrs {
                    out.extend(v.iter().filter_map(|x| match x {
                        Piece::Ref(i) => Some(*i),
                        Piece::Lit(_) => None,
                    }));
                }
                for c in &e.children {
                    walk(c, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    for n in nodes {
        walk(n, &mut out);
    }
    out
}

/// Neither a common run nor a list: split the whole region on one axis and
/// reconcile the branches against each other afterwards, so that a region which
/// is partly conditional and partly a list (`src/organisms/main_nav.rs` renders
/// an optional back link before its link list) keeps both shapes rather than
/// duplicating the list into two ternary arms.
fn split_seq(ctx: &Ctx, group: &[SrcList]) -> Result<Vec<Jsx>> {
    let cells: Vec<usize> = group.iter().map(|g| g.cell).collect();
    let mut last: Option<anyhow::Error> = None;
    for axis in ranked_axes(ctx, group, &cells)? {
        let classes = classify_by(group, |g| {
            ctx.cells[g.cell].cell.get(&axis).map(|s| s.to_string()).unwrap_or_default()
        });
        if classes.len() < 2 {
            continue;
        }
        let mut parts: Vec<(Vec<Selection>, Vec<Jsx>)> = Vec::new();
        let mut failed = None;
        for class in &classes {
            let sub: Vec<SrcList> = class.iter().map(|&i| group[i].clone()).collect();
            let sels: Vec<Selection> = {
                let mut v: Vec<Selection> = Vec::new();
                for &i in class {
                    let s = ctx.cells[group[i].cell].cell.get(&axis).unwrap().clone();
                    if !v.contains(&s) {
                        v.push(s);
                    }
                }
                v
            };
            match align(ctx, &sub) {
                Ok(j) => parts.push((sels, j)),
                Err(e) => {
                    failed = Some(e);
                    break;
                }
            }
        }
        match failed {
            Some(e) => last = Some(e),
            None => return merge_parts(ctx, &axis, group[0].scope.as_ref(), parts),
        }
    }
    Err(last.unwrap_or_else(|| {
        anyhow!(
            "{}: cells {} render different child sequences ({}) and no matrix axis explains the \
             difference",
            ctx.component,
            group.iter().map(|g| ctx.id(g.cell)).collect::<Vec<_>>().join(", "),
            group
                .iter()
                .map(|g| format!("{} node(s)", g.nodes.len()))
                .collect::<Vec<_>>()
                .join(" vs ")
        )
    }))
}

/// Order the candidate axes by how much of the disagreement each one settles.
///
/// Dominance alone is not enough when two axes cover the same cells. At
/// `src/molecules/annotation.rs:35` `language` wins over `datatype`, so
/// splitting on `language` leaves one branch uniform and the other with a
/// single question left; splitting on `datatype` — which sorts first
/// alphabetically — leaves both branches still divided and duplicates the
/// `<span class="badge">` into each. Scoring by the number of distinct
/// sequences each branch still contains picks the dominant one without anyone
/// having to name it.
fn ranked_axes(ctx: &Ctx, group: &[SrcList], cells: &[usize]) -> Result<Vec<String>> {
    let order = ctx.axes(cells);
    let mut scored: Vec<(usize, usize, String)> = Vec::new();
    for (rank, axis) in order.into_iter().enumerate() {
        let classes = classify_by(group, |g| {
            ctx.cells[g.cell].cell.get(&axis).map(|s| s.to_string()).unwrap_or_default()
        });
        if classes.len() < 2 {
            continue;
        }
        let mut score = 0usize;
        for class in &classes {
            let mut sigs: BTreeSet<String> = BTreeSet::new();
            for &i in class {
                sigs.insert(seq_sig(ctx, &group[i])?);
            }
            score += sigs.len();
        }
        scored.push((score, rank, axis));
    }
    scored.sort();
    Ok(scored.into_iter().map(|(_, _, a)| a).collect())
}

fn seq_sig(ctx: &Ctx, g: &SrcList) -> Result<String> {
    let mut out = String::new();
    for n in g.nodes {
        out.push_str(&sig(ctx, &Src { cell: g.cell, node: n, scope: g.scope.clone() })?);
        out.push('\u{1}');
    }
    Ok(out)
}

/// Re-align the branches of a split against one another: whatever they share
/// stays unconditional, and only what actually differs becomes a conditional.
fn merge_parts(
    ctx: &Ctx,
    axis: &str,
    scope: Option<&Rc<Scope>>,
    parts: Vec<(Vec<Selection>, Vec<Jsx>)>,
) -> Result<Vec<Jsx>> {
    let mut lo = 0usize;
    let min = parts.iter().map(|(_, j)| j.len()).min().unwrap_or(0);
    while lo < min && parts.iter().all(|(_, j)| j[lo] == parts[0].1[lo]) {
        lo += 1;
    }
    let mut hi = 0usize;
    while hi < min - lo
        && parts.iter().all(|(_, j)| j[j.len() - 1 - hi] == parts[0].1[parts[0].1.len() - 1 - hi])
    {
        hi += 1;
    }
    let mut out: Vec<Jsx> = parts[0].1[..lo].to_vec();
    let middles: Vec<(Vec<Selection>, Vec<Jsx>)> = parts
        .iter()
        .map(|(s, j)| (s.clone(), j[lo..j.len() - hi].to_vec()))
        .collect();
    if middles.iter().any(|(_, m)| !m.is_empty()) {
        let arms: Vec<(Vec<Selection>, Jsx)> =
            middles.iter().map(|(s, m)| (s.clone(), one(m.clone()))).collect();
        // The baseline branch is `arms[0]`; `chain` uses it as the else, which
        // turns an "absent here, present there" pair into `{cond && (...)}`.
        let jsx = if arms.len() == 2 && matches!(arms[0].1, Jsx::Nothing) {
            let cond = cond_for(ctx, axis, &arms[1].0, scope)?;
            Jsx::When { cond, then: Box::new(arms[1].1.clone()) }
        } else {
            chain(ctx, axis, scope, arms)?
        };
        push(&mut out, jsx);
    }
    let tail = &parts[0].1;
    out.extend(tail[tail.len() - hi..].to_vec());
    Ok(out)
}

// ---------------------------------------------------------------- attributes

fn frags_of(
    ctx: &Ctx,
    cell: usize,
    pieces: &[Piece],
    scope: Option<&Rc<Scope>>,
) -> Result<Vec<Frag>> {
    let mut out = Vec::new();
    for p in pieces {
        match p {
            Piece::Lit(l) => {
                if !l.is_empty() {
                    out.push(Frag::Lit(l.clone()));
                }
            }
            Piece::Ref(i) => out.push(Frag::Expr(resolve(ctx.path_of(cell, *i)?, scope))),
        }
    }
    if out.is_empty() {
        out.push(Frag::Lit(String::new()));
    }
    Ok(out)
}

fn value_of(frags: &[Frag]) -> AttrValue {
    match frags {
        [Frag::Lit(l)] => AttrValue::Lit(l.clone()),
        [Frag::Expr(e)] => AttrValue::Expr(e.clone()),
        _ => AttrValue::Template(frags.to_vec()),
    }
}

type ElemGroup<'a> = (usize, Option<Rc<Scope>>, &'a ElemPat);

fn unify_attrs(ctx: &Ctx, group: &[ElemGroup]) -> Result<Vec<Attr>> {
    let tag = group[0].2.tag.clone();
    let svg = group[0].2.svg;
    let mut names: Vec<String> = Vec::new();
    for (_, _, e) in group {
        for (n, _) in &e.attrs {
            if n != SLOT_ATTR && !names.contains(n) {
                names.push(n.clone());
            }
        }
    }

    let mut out = Vec::new();
    for name in names {
        let mut have: Vec<usize> = Vec::new();
        let mut values: Vec<Vec<Frag>> = Vec::new();
        for (i, (cell, scope, e)) in group.iter().enumerate() {
            if let Some((_, v)) = e.attrs.iter().find(|(n, _)| *n == name) {
                have.push(i);
                values.push(frags_of(ctx, *cell, v, scope.as_ref())?);
            }
        }
        let react = react_attr(&name, svg, ctx.component, &tag)?;
        let boolean = BOOL_ATTRS.contains(&name.as_str());

        if have.len() == group.len() {
            let value = if name == "style" {
                unify_style(ctx, group, &values)?
            } else if boolean {
                AttrValue::True
            } else {
                unify_values(ctx, group, &have, &values)?
            };
            out.push(Attr { name: react, value });
            continue;
        }

        // Present in some cells only. The condition is whatever axis separates
        // the cells that have it from the cells that do not.
        let cells: Vec<usize> = group.iter().map(|(c, _, _)| *c).collect();
        let present: Vec<usize> = have.iter().map(|&i| group[i].0).collect();
        let absent: Vec<usize> =
            (0..group.len()).filter(|i| !have.contains(i)).map(|i| group[i].0).collect();
        let (axis, sels) = explain(ctx, &cells, &[absent, present.clone()]).ok_or_else(|| {
            anyhow!(
                "{}: `{name}` on <{tag}> is rendered in cells {} but not in the others, and no \
                 matrix axis separates them",
                ctx.component,
                present.iter().map(|&c| ctx.id(c)).collect::<Vec<_>>().join(", ")
            )
        })?;
        let cond = cond_for(ctx, &axis, &sels[1], group[0].1.as_ref())?;
        let value = if boolean {
            AttrValue::Expr(cond)
        } else {
            let inner = if name == "style" {
                let sub: Vec<ElemGroup> = have.iter().map(|&i| group[i].clone()).collect();
                unify_style(ctx, &sub, &values)?
            } else {
                unify_values(ctx, group, &have, &values)?
            };
            AttrValue::Cond { arms: vec![(cond, inner)], fallback: None }
        };
        out.push(Attr { name: react, value });
    }
    Ok(out)
}

/// One attribute, present everywhere, whose value differs between cells.
fn unify_values(
    ctx: &Ctx,
    group: &[ElemGroup],
    have: &[usize],
    values: &[Vec<Frag>],
) -> Result<AttrValue> {
    let classes = classify_by(values, |v| format!("{v:?}"));
    if classes.len() == 1 {
        return Ok(value_of(&values[0]));
    }
    let cells: Vec<usize> = have.iter().map(|&i| group[i].0).collect();
    let cell_classes: Vec<Vec<usize>> =
        classes.iter().map(|c| c.iter().map(|&i| group[have[i]].0).collect()).collect();
    let (axis, sels) = explain(ctx, &cells, &cell_classes).ok_or_else(|| {
        anyhow!(
            "{}: an attribute on <{}> takes {} different values across cells and no matrix axis \
             explains which is which ({})",
            ctx.component,
            group[0].2.tag,
            classes.len(),
            classes
                .iter()
                .map(|c| format!("{:?}", values[c[0]]))
                .collect::<Vec<_>>()
                .join(" vs ")
        )
    })?;
    let mut arms: Vec<(String, AttrValue)> = Vec::new();
    for (ci, class) in classes.iter().enumerate().skip(1) {
        let cond = cond_for(ctx, &axis, &sels[ci], group[0].1.as_ref())?;
        arms.push((cond, value_of(&values[class[0]])));
    }
    Ok(AttrValue::Cond {
        arms,
        fallback: Some(Box::new(value_of(&values[classes[0][0]]))),
    })
}

/// `style="a:b;c:d"` -> a React style object. A whole-attribute sentinel is a
/// raw CSS prop instead (`src/molecules/logo_download_card.rs:30`), which React
/// cannot take as a string.
fn unify_style(ctx: &Ctx, group: &[ElemGroup], values: &[Vec<Frag>]) -> Result<AttrValue> {
    if values.iter().all(|v| matches!(v.as_slice(), [Frag::Expr(_)])) {
        let exprs: BTreeSet<&str> = values
            .iter()
            .map(|v| match &v[0] {
                Frag::Expr(e) => e.as_str(),
                Frag::Lit(_) => unreachable!(),
            })
            .collect();
        if exprs.len() == 1 {
            return Ok(AttrValue::StyleText(exprs.into_iter().next().unwrap().to_string()));
        }
    }
    let decls: Vec<Vec<(String, Vec<Frag>)>> =
        values.iter().map(|v| declarations(v)).collect::<Result<_>>()?;
    let mut keys: Vec<String> = Vec::new();
    for d in &decls {
        for (k, _) in d {
            if !keys.contains(k) {
                keys.push(k.clone());
            }
        }
    }
    let mut entries = Vec::new();
    for k in keys {
        let mut have = Vec::new();
        let mut vals = Vec::new();
        for (i, d) in decls.iter().enumerate() {
            if let Some((_, v)) = d.iter().find(|(n, _)| *n == k) {
                have.push(i);
                vals.push(v.clone());
            }
        }
        let value = unify_values(ctx, group, &have, &vals)?;
        if have.len() == group.len() {
            entries.push(StyleEntry::Prop { key: k, value });
        } else {
            let cells: Vec<usize> = group.iter().map(|(c, _, _)| *c).collect();
            let present: Vec<usize> = have.iter().map(|&i| group[i].0).collect();
            let absent: Vec<usize> =
                (0..group.len()).filter(|i| !have.contains(i)).map(|i| group[i].0).collect();
            let (axis, sels) =
                explain(ctx, &cells, &[absent, present]).ok_or_else(|| {
                    anyhow!(
                        "{}: the `{k}` style declaration is rendered in some cells only and no \
                         matrix axis separates them",
                        ctx.component
                    )
                })?;
            let cond = cond_for(ctx, &axis, &sels[1], group[0].1.as_ref())?;
            entries.push(StyleEntry::Spread { cond, props: vec![(k, value)] });
        }
    }
    Ok(AttrValue::Style(entries))
}

fn declarations(frags: &[Frag]) -> Result<Vec<(String, Vec<Frag>)>> {
    let mut out: Vec<(String, Vec<Frag>)> = Vec::new();
    let mut decl: Vec<Frag> = Vec::new();
    let flush = |decl: &mut Vec<Frag>, out: &mut Vec<(String, Vec<Frag>)>| -> Result<()> {
        if decl.iter().all(|f| matches!(f, Frag::Lit(l) if l.trim().is_empty())) {
            decl.clear();
            return Ok(());
        }
        let Some(Frag::Lit(head)) = decl.first().cloned() else {
            bail!("a style declaration starts with an expression, so its property name is not known")
        };
        let Some(colon) = head.find(':') else {
            bail!("style declaration {decl:?} has no `:`")
        };
        let key = head[..colon].trim().to_string();
        let mut value: Vec<Frag> = Vec::new();
        let rest = head[colon + 1..].trim_start().to_string();
        if !rest.is_empty() {
            value.push(Frag::Lit(rest));
        }
        value.extend(decl[1..].iter().cloned());
        // Trailing whitespace is html formatting, not a value.
        if let Some(Frag::Lit(l)) = value.last_mut() {
            *l = l.trim_end().to_string();
            if l.is_empty() && value.len() > 1 {
                value.pop();
            }
        }
        if value.is_empty() {
            value.push(Frag::Lit(String::new()));
        }
        out.push((key, value));
        decl.clear();
        Ok(())
    };
    for f in frags {
        match f {
            Frag::Expr(_) => decl.push(f.clone()),
            Frag::Lit(l) => {
                let mut rest = l.as_str();
                while let Some(at) = rest.find(';') {
                    if at > 0 {
                        decl.push(Frag::Lit(rest[..at].to_string()));
                    }
                    flush(&mut decl, &mut out)?;
                    rest = &rest[at + 1..];
                }
                if !rest.is_empty() {
                    decl.push(Frag::Lit(rest.to_string()));
                }
            }
        }
    }
    flush(&mut decl, &mut out)?;
    Ok(out)
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::{SENTINEL_CLOSE, SENTINEL_OPEN};
    use std::path::{Path as FsPath, PathBuf};

    fn out_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("out")
    }

    /// The artefacts `render` leaves behind. Absent on a clean checkout, so the
    /// end-to-end tests skip rather than fail: `cargo xtask render` produces them.
    fn artefacts() -> Option<(Toolkit, HarnessPlan, BTreeMap<String, RenderResult>)> {
        let dir = out_dir();
        let (ir, plan, renders) =
            (dir.join("ir.json"), dir.join("harness/plan.json"), dir.join("renders.json"));
        if !ir.exists() || !plan.exists() || !renders.exists() {
            return None;
        }
        let mut tk: Toolkit =
            serde_json::from_str(&std::fs::read_to_string(&ir).unwrap()).unwrap();
        // `ir.json` is rewritten by `dump-ir` as well as by `render`, so it may
        // predate the gate. quarantine.json is the verdict, and folding it in is
        // what `main` does before calling this stage.
        let q = dir.join("quarantine.json");
        if q.exists() {
            let file: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&q).unwrap()).unwrap();
            for v in file["quarantined"].as_array().into_iter().flatten() {
                let name = v["component"].as_str().unwrap();
                let reason = v["reason"].as_str().unwrap_or("quarantined").to_string();
                if let Some(c) = tk.components.iter_mut().find(|c| c.name == name) {
                    c.status = Status::Quarantined { reason };
                }
            }
        }
        let plan: HarnessPlan =
            serde_json::from_str(&std::fs::read_to_string(&plan).unwrap()).unwrap();
        let renders =
            crate::render::parse_results(&std::fs::read_to_string(&renders).unwrap(), &dir).unwrap();
        Some((tk, plan, renders))
    }

    fn sent(n: usize) -> String {
        format!("{SENTINEL_OPEN}{n}{SENTINEL_CLOSE}")
    }

    #[test]
    fn the_alphabet_this_stage_scans_for_is_the_one_the_harness_plants() {
        // Two spellings of the same delimiter is how `verify` and `harness`
        // drifted apart once already; this pins them together.
        assert_eq!(Alphabet::Primary.open(), SENTINEL_OPEN);
        assert_eq!(Alphabet::Primary.close(), SENTINEL_CLOSE);
    }

    #[test]
    fn sentinels_split_out_of_text_and_attributes() {
        let p = pieces(&format!("language-{}", sent(1)), "T", "base").unwrap();
        assert_eq!(p, vec![Piece::Lit("language-".into()), Piece::Ref(1)]);
        let p = pieces(&sent(12), "T", "base").unwrap();
        assert_eq!(p, vec![Piece::Ref(12)]);
    }

    #[test]
    fn a_root_level_conditional_is_written_in_expression_position() {
        // `return ({cond ? a : b})` parses as a block, not a ternary.
        // `src/atoms/button.rs:32-34` and `src/atoms/pill_button.rs` are the two
        // components whose root really is a conditional, because each writes two
        // `html!` blocks rather than one with a conditional attribute.
        let tree = Jsx::Ternary {
            cond: "props.href != null".into(),
            then: Box::new(Jsx::Text("a".into())),
            other: Box::new(Jsx::Text("b".into())),
        };
        let body = tree.to_return_body();
        assert!(!body.trim_start().starts_with('{'), "{body}");
        assert!(body.starts_with("props.href != null ?"), "{body}");
        // An element root is already an expression and must not pick up a
        // wrapper, or all 33 other components change shape for nothing.
        let el = Jsx::Element(Element {
            tag: "span".into(),
            attrs: Vec::new(),
            children: Vec::new(),
            svg: false,
            key: None,
        });
        assert_eq!(el.to_return_body(), el.to_tsx());
    }

    #[test]
    fn a_bare_delimiter_is_an_error_not_a_literal() {
        // `src/molecules/dataset_card.rs:43` uppercases the first char of a prop,
        // which leaves an opening delimiter with no id. verify quarantines for
        // it; if one ever reached here, splicing it would invent markup.
        let err = pieces(&format!("{SENTINEL_OPEN}x"), "DatasetCard", "base").unwrap_err();
        assert!(err.to_string().contains("bare sentinel delimiter"), "{err}");
    }

    #[test]
    fn prop_paths_become_javascript() {
        assert_eq!(resolve("title", None), "props.title");
        assert_eq!(resolve("download_name", None), "props.downloadName");
        assert_eq!(resolve("back.href", None), "props.back.href");
        assert_eq!(resolve("props[1].default", None), "props.props[1].default");
        let scope = Rc::new(Scope {
            parent: None,
            concrete: "values[2]".into(),
            axis: "values[]".into(),
            binder: "item".into(),
        });
        assert_eq!(resolve("values[2].language", Some(&scope)), "item.language");
        // The matrix spells the same axis element-uniformly; a condition built
        // from it has to land on the same binder as the value does.
        assert_eq!(resolve("values[].language", Some(&scope)), "item.language");
        assert_eq!(resolve("label", Some(&scope)), "props.label");
    }

    #[test]
    fn tuple_fields_are_indexed_not_dotted() {
        // `src/molecules/nav_dropdown.rs:18` is a Vec<(&str, &str)>; TypeScript
        // spells the second element `item[1]`, not `item.1`.
        let scope = Rc::new(Scope {
            parent: None,
            concrete: "links[0]".into(),
            axis: "links[]".into(),
            binder: "item".into(),
        });
        assert_eq!(resolve("links[0].1", Some(&scope)), "item[1]");
    }

    #[test]
    fn variant_literals_match_the_typescript_union() {
        // classify::variant_literals is private; these are the cases that matter,
        // and `variant="class"` is BadgeVariant::Kind(TermKind::Class), not a
        // variant called `Class` (src/atoms/onto_badge.rs:20).
        assert_eq!(variant_literal("Primary"), "primary");
        assert_eq!(variant_literal("GlassLight"), "glass-light");
        assert_eq!(variant_literal("Kind(TermKind::ObjectProperty)"), "object-property");
        assert_eq!(variant_literal("Kind(TermKind::Class)"), "class");
        assert_eq!(variant_literal("Lang"), "lang");
    }

    #[test]
    fn unknown_attributes_are_an_error_not_a_pass_through() {
        assert_eq!(react_attr("class", false, "T", "div").unwrap(), "className");
        assert_eq!(react_attr("for", false, "T", "label").unwrap(), "htmlFor");
        assert_eq!(react_attr("aria-current", false, "T", "a").unwrap(), "aria-current");
        assert_eq!(react_attr("data-close", false, "T", "button").unwrap(), "data-close");
        assert_eq!(react_attr("viewBox", true, "T", "svg").unwrap(), "viewBox");
        assert_eq!(react_attr("stroke-width", true, "T", "line").unwrap(), "strokeWidth");
        let err = react_attr("onclick", false, "T", "button").unwrap_err();
        assert!(err.to_string().contains("no React spelling"), "{err}");
    }

        /// The two defects a differential render found in this hand-written helper,
    /// asserted on its source because a Rust test cannot execute TypeScript.
    ///
    /// Re-verify the behaviour itself with node after any edit — extract the
    /// constant, strip the type annotations, and run the cases in
    /// xtask/README.md's "Changing the style helper" section. Both defects were
    /// found that way and both are covered there:
    ///
    /// - `background-image:url(data:image/svg+xml;base64,AAAA)` used to be
    ///   truncated at the semicolon inside `url(...)`;
    /// - `BACKGROUND:#fff` used to reach React as `BACKGROUND`, which React
    ///   re-hyphenates to `-b-a-c-k-g-r-o-u-n-d` and which styles nothing.
    #[test]
    fn the_style_helper_does_not_split_naively_and_does_not_keep_case() {
        assert!(
            !STYLE_HELPER_TS.contains(r#".split(";")"#),
            "a bare split on `;` truncates any value containing one: {STYLE_HELPER_TS}"
        );
        // Paren depth is what keeps `url(a;b)` in one piece.
        assert!(STYLE_HELPER_TS.contains("depth"), "{STYLE_HELPER_TS}");
        // Quote tracking is what keeps `"Foo; Bar"` in one piece.
        assert!(STYLE_HELPER_TS.contains("quote"), "{STYLE_HELPER_TS}");
        // CSS property names are case-insensitive; React's serialiser is not.
        assert!(STYLE_HELPER_TS.contains("toLowerCase()"), "{STYLE_HELPER_TS}");
        // ...but custom properties ARE case-sensitive, so they must skip it.
        assert!(STYLE_HELPER_TS.contains(r#"startsWith("--")"#), "{STYLE_HELPER_TS}");
    }

#[test]
    fn style_declarations_become_object_entries() {
        let frags = vec![
            Frag::Lit("background:".into()),
            Frag::Expr("props.bg".into()),
            Frag::Lit(";color:".into()),
            Frag::Expr("props.fg".into()),
            Frag::Lit(";".into()),
        ];
        let decls = declarations(&frags).unwrap();
        assert_eq!(decls.len(), 2);
        assert_eq!(decls[0].0, "background");
        assert_eq!(decls[0].1, vec![Frag::Expr("props.bg".into())]);
        assert_eq!(decls[1].0, "color");
    }

    #[test]
    fn hyphenated_style_keys_camel_case_but_custom_properties_do_not() {
        assert_eq!(style_key("background"), "\"background\"");
        assert_eq!(style_key("font-size"), "\"fontSize\"");
        assert_eq!(style_key("--eona-gold"), "\"--eona-gold\"");
    }

    #[test]
    fn jsx_text_that_could_be_read_as_code_is_emitted_as_a_string() {
        assert!(!text_needs_expr("Download SVG"));
        assert!(text_needs_expr("  EONA-X"));
        assert!(text_needs_expr("a & b"));
        assert!(text_needs_expr("{x}"));
    }

    // ------------------------------------------------------------ end to end

    fn splice_one(name: &str) -> Option<Jsx> {
        let (tk, plan, renders) = artefacts()?;
        let c = tk.component(name).expect("component in the IR");
        let cells = crate::harness::cells_for(&tk).unwrap();
        let list = cells.get(name).expect("cells for the component");
        let pairs: Vec<(Cell, String)> = list
            .iter()
            .map(|cell| {
                let key = format!("{name}/{}#primary", cell.id);
                (cell.clone(), renders[&key].html().expect("markup").to_string())
            })
            .collect();
        Some(splice(c, &pairs, &plan).unwrap())
    }

    #[test]
    fn button_recovers_its_tag_swap_and_its_variant() {
        let Some(jsx) = splice_one("Button") else { return };
        let tsx = jsx.to_tsx();
        // src/atoms/button.rs renders an <a> when `href` is Some and a <button>
        // otherwise, so the split has to be at the root, not inside it.
        assert!(tsx.starts_with("{props.href != null ? ("), "{tsx}");
        assert!(tsx.contains("<a"), "{tsx}");
        assert!(tsx.contains("<button"), "{tsx}");
        assert!(tsx.contains("props.variant === \"ghost\""), "{tsx}");
        assert!(tsx.contains("{props.label}"), "{tsx}");
        assert!(!tsx.contains('\u{E000}'), "a sentinel survived into the output: {tsx}");
    }

    #[test]
    fn props_table_keeps_both_arms_of_its_nested_axis() {
        let Some(jsx) = splice_one("PropsTable") else { return };
        let tsx = jsx.to_tsx();
        // props_table.rs:59 branches `if let Some(default)`. Both arms must be
        // in the output: hard-coding one is the silent wrongness the nested-axis
        // rule exists to prevent.
        assert!(tsx.contains("props-required"), "{tsx}");
        assert!(tsx.contains("item.default"), "{tsx}");
        // The rows are a list template, not three copies.
        assert!(tsx.contains("props.props.map((item, i)"), "{tsx}");
        assert_eq!(tsx.matches("<tr").count(), 2, "one header row and one template row: {tsx}");
        assert!(tsx.contains("key={i}"), "{tsx}");
    }

    #[test]
    fn onto_annotation_keeps_the_language_over_datatype_precedence() {
        let Some(jsx) = splice_one("OntoAnnotation") else { return };
        let tsx = jsx.to_tsx();
        // annotation.rs:35 prefers `language`; `datatype` is only reachable when
        // `language` is None, so the two conditionals have to nest.
        assert!(tsx.contains("item.language"), "{tsx}");
        assert!(tsx.contains("item.datatype"), "{tsx}");
        let lang = tsx.find("item.language != null").expect("language test");
        let dt = tsx.find("item.datatype != null").expect("datatype test");
        assert!(lang < dt, "datatype must be tested inside the language else arm: {tsx}");
    }

    #[test]
    fn code_block_splices_inside_an_attribute_value() {
        let Some(jsx) = splice_one("CodeBlock") else { return };
        let tsx = jsx.to_tsx();
        // code_block.rs:38 is format!("language-{}", props.language): the
        // sentinel arrives as a substring, so the value is a template literal.
        assert!(tsx.contains("className={`language-${props.language}`}"), "{tsx}");
    }

    #[test]
    fn nav_dropdown_writes_its_id_into_every_place_yew_did() {
        let Some(jsx) = splice_one("NavDropdown") else { return };
        let tsx = jsx.to_tsx();
        // nav_dropdown.rs:27,29,35,43 renders `id` four times. verify carries
        // this as a note precisely because a one-sentinel-one-slot splice would
        // write it once.
        assert_eq!(tsx.matches("{props.id}").count(), 4, "{tsx}");
        // Yew writes `inert="inert"`; React wants the boolean, and a JSX
        // attribute with no value is `true`.
        assert!(tsx.contains(" inert "), "{tsx}");
        assert!(tsx.contains("style={{ \"position\": \"relative\" }}"), "{tsx}");
        // A Vec<(&str, &str)> element is a TypeScript tuple.
        assert!(tsx.contains("href={item[0]}"), "{tsx}");
    }

    #[test]
    fn accordion_item_turns_a_boolean_attribute_back_into_a_prop() {
        let Some(jsx) = splice_one("AccordionItem") else { return };
        let tsx = jsx.to_tsx();
        // Yew renders `open="open"` only in the open cell; React wants the bool.
        assert!(tsx.contains("open={props.open}"), "{tsx}");
        assert!(tsx.contains("{props.children}"), "{tsx}");
    }

    #[test]
    fn main_nav_keeps_its_optional_link_outside_the_list() {
        let Some(jsx) = splice_one("MainNav") else { return };
        let tsx = jsx.to_tsx();
        // The back link and the link list share a parent; splitting on `back`
        // must not duplicate the list into both ternary arms.
        assert_eq!(tsx.matches("props.links.map").count(), 1, "{tsx}");
        assert!(tsx.contains("props.back != null &&"), "{tsx}");
    }

    #[test]
    fn site_header_conditions_a_list_element_attribute_on_its_own_field() {
        let Some(jsx) = splice_one("SiteHeader") else { return };
        let tsx = jsx.to_tsx();
        // `aria-*` keeps its HTML spelling, and an attribute Yew only rendered
        // in some cells becomes `undefined` in the others — React drops it.
        assert!(tsx.contains("aria-current={item.current ? \"page\" : undefined}"), "{tsx}");
    }

    #[test]
    fn every_component_the_gate_passed_splices() {
        let Some((tk, plan, renders)) = artefacts() else { return };
        let cells = crate::harness::cells_for(&tk).unwrap();
        let report = splice_toolkit(&tk, &cells, &plan, &renders).unwrap();
        assert!(
            report.failures.is_empty(),
            "{} component(s) could not be spliced:\n{}",
            report.failures.len(),
            report
                .failures
                .iter()
                .map(|(n, e)| format!("  {n}: {e}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        // 38 in scope minus the four the gate quarantined: Swatch,
        // DatasetCard, PaletteGroup, and OntoAnnotation
        // (src/molecules/annotation.rs:24 shortens the datatype IRI).
        assert_eq!(report.components.len(), 34, "unexpected number of spliced components");
        for (name, jsx) in &report.components {
            let tsx = jsx.to_tsx();
            assert!(!tsx.contains('\u{E000}'), "{name}: a sentinel survived into the output");
            assert!(!tsx.contains("data-eona-slot"), "{name}: a slot marker survived");
        }
    }

    #[test]
    #[ignore = "prints the two trees the brief asks to see; run with --nocapture"]
    fn dump_button_and_props_table() {
        for name in ["Button", "PropsTable", "OntoAnnotation", "SiteHeader", "NavDropdown", "MainNav", "BgCheckItem", "LogoDownloadCard", "TextField", "OntoBadge", "Hero"] {
            if let Some(jsx) = splice_one(name) {
                println!("=== {name} ===\n{}\n", jsx.to_tsx());
            }
        }
    }
}
