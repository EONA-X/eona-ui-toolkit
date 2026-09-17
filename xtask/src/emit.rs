//! Stage: `emit`. Renders the spliced JSX to the npm package on disk.
//!
//! Everything here is a pure function of `(Toolkit, BTreeMap<String, Jsx>)`:
//! `render_package` builds the whole package in memory as path -> contents and
//! `emit` is the only part that touches the filesystem. That split exists
//! because the drift gate (`cargo xtask check`) diffs this output — any
//! nondeterminism makes CI fail at random — so the bytes have to be testable
//! without a temp directory, and every container that reaches the output is a
//! `BTreeMap`/`BTreeSet` or a `Vec` filled in the IR's own declaration order.
//! No timestamps, no absolute paths (every span printed is the repo-relative
//! one the IR carries), no `\r`.
//!
//! What this stage does *not* do: decide markup. The JSX bodies come from
//! `splice`, which recovered them from the HTML Yew's own SSR renderer emitted.
//! Because that HTML is flat, a generated component never imports another
//! generated component — `NewsCard` renders `TagLabel`'s markup inline
//! (`src/molecules/news_card.rs:21`), it does not import it.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::classify::ts_type;
use crate::splice::js_prop_name;
use crate::ir::*;

/// Where the package is written, relative to `emit`'s `out`.
pub const PACKAGE_DIR: &str = "packages/react";
pub const PACKAGE_NAME: &str = "@eona-x/ui-toolkit-react";
/// The stylesheet package the class names in the generated markup resolve
/// against. A dependency rather than a peer: the markup is meaningless without
/// it, and it carries no code, so there is nothing to deduplicate.
pub const CSS_PACKAGE: &str = "@eona-x/ui-toolkit-css";
/// Mirrors the toolkit crate's own version. The IR carries `src_hash` but not
/// the crate version, so this is the one string here that has to be kept in
/// step with the root `Cargo.toml` by hand.
pub const PACKAGE_VERSION: &str = "0.1.0";

const GENERATOR: &str = "cargo xtask bridgegen";

/// One component's spliced JSX, as `splice` hands it to `emit`.
///
/// Deliberately narrow: `emit` reads the IR for everything it can (names,
/// types, defaults, spans, status) and takes only the markup from here, so the
/// two stages cannot disagree about a prop's type or its default.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Jsx {
    /// Matches `Component::name`; `emit` checks it against the map key.
    pub component: String,
    /// Module-level declarations emitted verbatim between the imports and the
    /// component — a variant -> class lookup, say, which is how the twelve
    /// `BadgeVariant` inhabitants at `src/atoms/onto_badge.rs:40-50` are most
    /// naturally expressed.
    #[serde(default)]
    pub helpers: Vec<String>,
    /// The single JSX expression the component returns. Re-indented here, so
    /// splice's own indentation does not leak into the diff.
    pub body: String,
    /// Findings `splice` honoured, from `verify::ComponentVerdict::notes` —
    /// e.g. that `src/molecules/nav_dropdown.rs:27,29,35,43` renders `id` four
    /// times. Emitted into the file header so the reason the markup looks
    /// redundant is readable next to the markup.
    #[serde(default)]
    pub notes: Vec<String>,
}

impl Jsx {
    pub fn new(component: impl Into<String>, body: impl Into<String>) -> Self {
        Self { component: component.into(), body: body.into(), ..Self::default() }
    }
}

/// Write the package under `out/packages/react`. `out` is the directory that
/// holds the `packages/` tree — the toolkit root when run from `bridgegen`.
pub fn emit(tk: &Toolkit, jsx: &BTreeMap<String, Jsx>, out: &Path) -> Result<()> {
    let (files, warnings) = render_package(tk, jsx)?;
    let pkg = out.join(PACKAGE_DIR);
    write_files(&files, &pkg)?;

    for w in &warnings {
        eprintln!("emit: {w}");
    }
    report_package(&files, &pkg);
    Ok(())
}

/// The one line an operator reads to know whether the run produced a package.
pub fn report_package(files: &BTreeMap<String, String>, pkg: &Path) {
    println!(
        "emit: {} files -> {} ({} component files, {} exported)",
        files.len(),
        pkg.display(),
        files.keys().filter(|k| k.ends_with(".tsx")).count(),
        // What `index.ts` actually re-exports, which is not the file count: a
        // `NeedsOverride` component is written and withheld.
        files["src/index.ts"].matches("\nexport { ").count(),
    );
}

/// Write a rendered package map to `pkg`, sweeping anything stale out of `src/`.
///
/// Split out of [`emit`] so `check --write` can publish the same bytes it just
/// diffed, rather than re-deriving them by a second route that could disagree.
pub fn write_files(files: &BTreeMap<String, String>, pkg: &Path) -> Result<()> {
    for (rel, contents) in files {
        let path = pkg.join(rel);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        fs::write(&path, contents).with_context(|| format!("writing {}", path.display()))?;
    }

    // A component that drops out of scope must not leave its `.tsx` behind: the
    // file would still typecheck, still be importable by path, and the drift
    // gate would report it as an extra rather than as the regression it is.
    // Only `src/` is swept, and only files this stage owns.
    for stale in stale_sources(pkg, files)? {
        let path = pkg.join(&stale);
        fs::remove_file(&path).with_context(|| format!("removing the stale {}", path.display()))?;
        eprintln!("emit: removed stale {stale}");
    }
    Ok(())
}

/// The `src/<Name>.tsx` files the package is supposed to contain, derived from
/// `Status` alone.
///
/// Derived, not recorded: this is what lets `check` catch an `abi/` that claims
/// a component is quarantined while its `.tsx` is still on disk and still
/// exported. A recorded list would be rewritten by `--write` along with the
/// tampered status and the divergence would certify itself.
///
/// The rule is [`plan`]'s: `Ok` is written and exported, `NeedsOverride` is
/// written and withheld from `index.ts`, and nothing else is written at all.
pub fn expected_files(tk: &Toolkit) -> BTreeSet<String> {
    tk.components
        .iter()
        .filter(|c| matches!(c.status, Status::Ok | Status::NeedsOverride { .. }))
        .map(|c| format!("src/{}.tsx", c.name))
        .collect()
}

/// The components `index.ts` is supposed to re-export — [`Toolkit::generated`],
/// i.e. `Status::Ok` and nothing else.
pub fn expected_exports(tk: &Toolkit) -> BTreeSet<String> {
    tk.generated().map(|c| c.name.clone()).collect()
}

/// The whole package as relative path -> contents, plus warnings for the
/// operator. Pure, so the drift gate and the tests can compare bytes.
pub fn render_package(
    tk: &Toolkit,
    jsx: &BTreeMap<String, Jsx>,
) -> Result<(BTreeMap<String, String>, Vec<String>)> {
    let mut warnings = Vec::new();
    let plan = plan(tk, jsx, &mut warnings)?;

    let mut files: BTreeMap<String, String> = BTreeMap::new();
    for entry in &plan.emitted {
        let tsx = component_tsx(entry, tk)?;
        files.insert(format!("src/{}.tsx", entry.component.name), tsx);
    }
    if let Some(types) = types_ts(&plan, tk)? {
        files.insert("src/types.ts".into(), types);
    }
    files.insert("src/index.ts".into(), index_ts(&plan, tk)?);
    files.insert("package.json".into(), package_json()?);
    files.insert("tsconfig.json".into(), tsconfig_json()?);
    files.insert("README.md".into(), readme_md(&plan, tk)?);

    // CRLF anywhere would make the next run on another platform diff against
    // this one for no semantic reason; splice's body is the only input this
    // stage does not construct itself, so it is the only place it can enter.
    for (rel, contents) in &files {
        if contents.contains('\r') {
            bail!("{rel} contains a CR; the generated package is LF-only");
        }
    }
    Ok((files, warnings))
}

/// A component that gets a `.tsx`, with the reason it is not re-exported when
/// that applies.
struct Emitted<'a> {
    component: &'a Component,
    jsx: &'a Jsx,
    /// `None` when the component is exported from `index.ts`.
    withheld: Option<String>,
}

struct Plan<'a> {
    emitted: Vec<Emitted<'a>>,
    /// Components in scope that produce no file: quarantined, plus anything
    /// `splice` could not hand over. `(component, reason)`.
    omitted: Vec<(&'a Component, String)>,
    /// Exclusion rule -> the components it excluded, for the index summary.
    excluded: BTreeMap<String, Vec<&'a str>>,
}

fn plan<'a>(
    tk: &'a Toolkit,
    jsx: &'a BTreeMap<String, Jsx>,
    warnings: &mut Vec<String>,
) -> Result<Plan<'a>> {
    let known: BTreeSet<&str> = tk.components.iter().map(|c| c.name.as_str()).collect();
    let unknown: Vec<&str> =
        jsx.keys().map(String::as_str).filter(|k| !known.contains(k)).collect();
    if !unknown.is_empty() {
        bail!(
            "splice produced JSX for {} component(s) the IR does not know: {}",
            unknown.len(),
            unknown.join(", ")
        );
    }
    for (key, j) in jsx {
        if !j.component.is_empty() && &j.component != key {
            bail!("splice keyed {:?}'s JSX under {key:?}", j.component);
        }
    }

    let mut plan =
        Plan { emitted: Vec::new(), omitted: Vec::new(), excluded: BTreeMap::new() };
    let mut missing: Vec<String> = Vec::new();

    // `tk.components` is in parse order; sort by name so the package's file set
    // and every list printed from it is stable against a source reshuffle.
    let mut components: Vec<&Component> = tk.components.iter().collect();
    components.sort_by(|a, b| a.name.cmp(&b.name));

    for c in components {
        match &c.status {
            Status::Ok => match jsx.get(&c.name) {
                Some(j) => plan.emitted.push(Emitted { component: c, jsx: j, withheld: None }),
                // Not a warning: an `Ok` component with no markup means the
                // pipeline lost it between verify and splice, and shipping a
                // package that is silently one component short is the failure
                // this whole generator exists to avoid.
                None => missing.push(format!("{} ({})", c.name, span(&c.span))),
            },
            Status::NeedsOverride { reason } => match jsx.get(&c.name) {
                // Still written, so the port is in the diff and reviewable, but
                // kept out of `index.ts`: an override is pending, and until it
                // lands the component can only ever render one of its states.
                Some(j) => plan.emitted.push(Emitted {
                    component: c,
                    jsx: j,
                    withheld: Some(format!("needs an override — {reason}")),
                }),
                None => plan.omitted.push((c, format!("needs an override — {reason}"))),
            },
            Status::Quarantined { reason } => {
                if jsx.contains_key(&c.name) {
                    warnings.push(format!(
                        "splice produced JSX for the quarantined {} ({}); dropped — {reason}",
                        c.name,
                        span(&c.span)
                    ));
                }
                plan.omitted.push((c, format!("quarantined — {reason}")));
            }
            Status::Excluded { reason } => {
                if jsx.contains_key(&c.name) {
                    bail!(
                        "splice produced JSX for {}, which is excluded ({reason}) at {}",
                        c.name,
                        span(&c.span)
                    );
                }
                plan.excluded.entry(reason.clone()).or_default().push(&c.name);
            }
        }
    }

    if !missing.is_empty() {
        bail!(
            "splice produced no JSX for {} component(s) the IR marks `Ok`: {}",
            missing.len(),
            missing.join(", ")
        );
    }
    Ok(plan)
}

// ---------------------------------------------------------------- components

/// Markers that would make the file a client component. These are presentational
/// ports of server-rendered markup, so any of them is a bug in `splice` rather
/// than something to paper over with a `"use client"` banner — a single one of
/// those in the package would opt every consumer's tree out of RSC.
const CLIENT_MARKERS: &[&str] = &[
    "use client",
    "useState(",
    "useEffect(",
    "useLayoutEffect(",
    "useReducer(",
    "useRef(",
    "useContext(",
];

fn component_tsx(e: &Emitted<'_>, tk: &Toolkit) -> Result<String> {
    let c = e.component;
    let Some(props_ty) = c.props_ty.as_deref() else {
        bail!("{} has no props struct but reached emit ({})", c.name, span(&c.span));
    };
    if e.jsx.body.trim().is_empty() {
        bail!("splice handed emit an empty body for {} ({})", c.name, span(&c.span));
    }
    for source in
        std::iter::once(&e.jsx.body).chain(e.jsx.helpers.iter()).chain(e.jsx.notes.iter())
    {
        // `str::lines` would swallow a CRLF in the body before the file-level
        // guard in `render_package` ever saw it, and the generated package must
        // be LF-only on every platform that runs the drift gate.
        if source.contains('\r') {
            bail!("{}'s JSX contains a CR ({})", c.name, span(&c.span));
        }
        for marker in CLIENT_MARKERS {
            if source.contains(marker) {
                bail!(
                    "{}'s JSX contains {marker:?}; the React package must render in a server \
                     component ({})",
                    c.name,
                    span(&c.span)
                );
            }
        }
    }

    // Every `props.x` the body reads has to be a prop this file declares.
    //
    // The two stages spell prop names independently — `splice` resolves a
    // sentinel path through `js_prop_name` (`splice.rs:831`), `emit` writes the
    // interface from `Prop::name` — and nothing else makes them agree. When
    // they disagreed, the generated package was every file of valid-looking TSX
    // reading `props.downloadName` off an interface that declared
    // `download_name`. `tsc` catches it, but the generator must not need a
    // Node toolchain to know it emitted something broken.
    let declared: BTreeSet<String> = c.props.iter().map(|p| js_prop_name(&p.name)).collect();
    for read in props_read(&e.jsx.body) {
        if !declared.contains(&read) {
            bail!(
                "{}'s JSX reads `props.{read}`, which {props_ty} does not declare ({}); \
                 `splice` and `emit` disagree about how a prop name is spelled",
                c.name,
                span(&c.span)
            );
        }
    }

    let mut sections: Vec<String> = Vec::new();

    let mut header = format!("// Generated by `{GENERATOR}` from {}. Do not edit.\n", span(&c.span));
    if let Some(why) = &e.withheld {
        header.push_str(&format!("// Not re-exported from the package index: {why}\n"));
    }
    for note in &e.jsx.notes {
        header.push_str(&format!("// Note: {note}\n"));
    }
    sections.push(header);

    let mut imports = String::new();
    // `splice::STYLE_HELPER_TS` types its return as `React.CSSProperties`
    // (`src/molecules/logo_download_card.rs:30` hands React a raw CSS string).
    // Without this the file resolves `React` off the UMD global, which is an
    // error under `module: ESNext` in every @types/react that declares it as
    // UMD rather than global.
    if e.jsx.helpers.iter().any(|h| h.contains("React.")) {
        imports.push_str("import type * as React from 'react';\n");
    }
    if c.props.iter().any(|p| uses_react_node(&p.kind)) {
        imports.push_str("import type { ReactNode } from 'react';\n");
    }
    let structs = struct_names(c.props.iter().map(|p| &p.kind), tk);
    if !structs.is_empty() {
        imports.push_str(&format!(
            "import type {{ {} }} from './types';\n",
            structs.iter().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    if !imports.is_empty() {
        sections.push(imports);
    }

    sections.push(props_interface(c, props_ty, tk)?);
    for helper in &e.jsx.helpers {
        sections.push(format!("{}\n", helper.trim_end()));
    }
    sections.push(function(c, props_ty, &e.jsx.body, tk)?);

    Ok(sections.join("\n"))
}

/// The head identifier of every `props.<name>` in a JSX body.
///
/// Only the head: `props.props[0].default` reads the prop `props`, and what
/// follows is the business of `types.ts`. `$props` is skipped — that is the raw
/// parameter `function` introduces, not something `splice` wrote.
fn props_read(body: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let b = body.as_bytes();
    for (i, _) in body.match_indices("props.") {
        // A preceding identifier char means this is `$props.` or `myprops.`; a
        // preceding `.` means it is a *field* called `props`, which is how
        // `props.props.map(...)` reads the `Vec<PropSpec>` at
        // `src/molecules/props_table.rs:36` — the head there is the first
        // `props.`, already matched, and `.map` is not a prop.
        if i > 0
            && (b[i - 1].is_ascii_alphanumeric()
                || b[i - 1] == b'_'
                || b[i - 1] == b'$'
                || b[i - 1] == b'.')
        {
            continue;
        }
        let rest = &body[i + "props.".len()..];
        let end = rest
            .find(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '$')
            .unwrap_or(rest.len());
        if end > 0 {
            out.insert(rest[..end].to_string());
        }
    }
    out
}

fn props_interface(c: &Component, props_ty: &str, tk: &Toolkit) -> Result<String> {
    let mut s = String::new();
    writeln!(s, "export interface {props_ty} {{")?;
    for p in &c.props {
        reject_reserved(&js_prop_name(&p.name), &p.span)?;
        if let Init::Unresolved { why } = initializer(p, tk) {
            writeln!(s, "  // FIXME(bridgegen): no default emitted — {why}")?;
        }
        writeln!(s, "  /** `{}` — {}{} */", p.rust_ty, span(&p.span), default_note(p))?;
        // `js_prop_name`, not `p.name`: `splice` resolves every sentinel path
        // through that function (`splice.rs:831`), so a prop declared here as
        // `download_name` would be read by the body as `props.downloadName` and
        // nobody would be passing it.
        writeln!(
            s,
            "  {}{}: {};",
            js_prop_name(&p.name),
            optional_marker(p),
            ts(&p.kind, p.optional, tk)
        )?;
    }
    writeln!(s, "}}")?;
    Ok(s)
}

fn function(c: &Component, props_ty: &str, body: &str, tk: &Toolkit) -> Result<String> {
    // Props that carry a default, in declaration order.
    let defaults: Vec<(String, String)> = c
        .props
        .iter()
        .filter_map(|p| match initializer(p, tk) {
            Init::Value(v) => Some((js_prop_name(&p.name), v)),
            _ => None,
        })
        .collect();

    let mut s = String::new();
    if defaults.is_empty() {
        writeln!(s, "export function {}(props: {props_ty}) {{", c.name)?;
    } else {
        // Not destructuring. `splice` resolves every prop read to `props.x`
        // (`splice.rs:831`), so a default applied in a destructuring pattern
        // would never be seen: `{ variant = 'primary' }` binds a local called
        // `variant` and leaves `props.variant` undefined for every caller who
        // omitted it, which is how `src/atoms/button.rs:13`'s class would come
        // out as `btn btn-undefined`. The defaults therefore have to be visible
        // through a binding actually called `props`.
        //
        // `$props` cannot collide with a prop: no Rust identifier starts with
        // `$`, so no `js_prop_name` output can either.
        writeln!(s, "export function {}($props: {props_ty}) {{", c.name)?;
        writeln!(s, "  const props = {{")?;
        writeln!(s, "    ...$props,")?;
        for (name, v) in &defaults {
            // `??` rather than `||`: `#[prop_or]` supplies the default when the
            // prop is absent, not when it is falsy. `src/molecules/text_field.rs:18`
            // defaults `placeholder` to the empty string, and `||` would then
            // replace a caller's deliberate `""` with `""` harmlessly but a
            // caller's deliberate `false` or `0` with the default.
            writeln!(s, "    {name}: $props.{name} ?? {v},")?;
        }
        writeln!(s, "  }};")?;
    }
    writeln!(s, "  return (")?;
    for line in reindent(body, 4).lines() {
        writeln!(s, "{line}")?;
    }
    writeln!(s, "  );")?;
    writeln!(s, "}}")?;
    Ok(s)
}

/// Strip the body's own common indentation and re-apply ours, so a change in
/// how `splice` formats its output is not a diff in every file.
fn reindent(body: &str, indent: usize) -> String {
    let lines: Vec<&str> = body.trim_matches('\n').lines().collect();
    let base = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    lines
        .iter()
        .map(|l| {
            if l.trim().is_empty() {
                String::new()
            } else {
                format!("{}{}", " ".repeat(indent), &l[base.min(l.len())..].trim_end())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ------------------------------------------------------------------ defaults

/// What, if anything, goes after the `=` in the destructuring.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Init {
    /// The prop is required, or its Rust default already *is* `undefined`.
    None,
    Value(String),
    /// The IR does not carry enough to write the default down. Never guessed:
    /// a wrong default is invisible at the call site and only shows up as
    /// markup that differs from the Yew render.
    Unresolved { why: String },
}

fn initializer(p: &Prop, tk: &Toolkit) -> Init {
    match &p.default {
        PropDefault::Required => Init::None,
        // `Option<T>::default()` is `None`, which *is* JavaScript's `undefined`.
        // Writing `= undefined` would be noise, and writing `= ''` would be a
        // lie: `src/atoms/button.rs:26` renders a `<button>` when `href` is
        // `None` and an `<a>` when it is `Some("")`.
        PropDefault::DefaultTrait if p.optional => Init::None,
        PropDefault::DefaultTrait => match &p.kind {
            // `#[prop_or_default]` on a non-`Option` prop is `T::default()`,
            // which for these is a value, not an absence — `text_field.rs:18`'s
            // `placeholder` really does render as an empty string.
            PropKind::Text => Init::Value("''".into()),
            PropKind::Bool => Init::Value("false".into()),
            PropKind::Num { .. } => Init::Value("0".into()),
            PropKind::List { .. } => Init::Value("[]".into()),
            // `Children::default()` is empty, and React already renders nothing
            // for an absent `children`.
            PropKind::Slot => Init::None,
            // `#[prop_or_default]` on an enum is `T::default()`, i.e. the
            // variant carrying `#[default]`. Routed through `enum_literal` —
            // the same positional mapping `#[prop_or(..)]` uses — so the two
            // spellings of "which union member is this variant" cannot diverge.
            PropKind::UnitEnum { name } => match tk
                .unit_enum(name)
                .and_then(|e| e.default_variant.as_deref())
                .and_then(|d| enum_literal(&format!("{name}::{d}"), name, tk))
            {
                Some(lit) => Init::Value(format!("'{lit}'")),
                None => Init::Unresolved {
                    why: format!(
                        "`#[prop_or_default]` on enum `{name}` ({}): it does not derive \
                         `Default`, or its `#[default]` variant does not map onto exactly one \
                         union member",
                        tk.unit_enum(name).map(|e| span(&e.span)).unwrap_or_else(|| "?".into())
                    ),
                },
            },
            PropKind::Struct { name } => Init::Unresolved {
                why: format!(
                    "`#[prop_or_default]` on struct `{name}` ({}): the IR does not record its \
                     `Default` impl",
                    tk.plain_struct(name).map(|s| span(&s.span)).unwrap_or_else(|| "?".into())
                ),
            },
            PropKind::Tuple { .. } => Init::Unresolved {
                why: format!("`#[prop_or_default]` on the tuple `{}`", p.rust_ty),
            },
            PropKind::Callback { .. } => Init::Unresolved {
                why: format!("`{}` cannot be pre-rendered", p.rust_ty),
            },
        },
        PropDefault::Expr { expr, ts } => match ts {
            // `classify` already evaluated the expression to a TypeScript
            // literal — `AttrValue::Static("rust")` -> `"rust"`
            // (`src/atoms/code_block.rs:19`).
            Some(lit) => Init::Value(requote(lit)),
            None => match &p.kind {
                PropKind::UnitEnum { name } => match enum_literal(expr, name, tk) {
                    Some(lit) => Init::Value(format!("'{lit}'")),
                    None => Init::Unresolved {
                        why: format!(
                            "`#[prop_or({expr})]` does not name a variant of `{name}` that maps \
                             onto exactly one union member"
                        ),
                    },
                },
                _ => Init::Unresolved {
                    why: format!("`#[prop_or({expr})]` is not a constant the generator evaluated"),
                },
            },
        },
    }
}

/// The union member `expr` denotes, e.g. `ButtonVariant::Primary` -> `'primary'`.
///
/// Derived *positionally* against the union `classify::ts_type` prints rather
/// than by re-implementing its kebab-casing: `classify::ts_union` builds the
/// union by walking `PropEnum::variants` in order, so when the two have the same
/// length the mapping is one-to-one and this cannot drift from what the
/// interface above declares. When they differ — a variant that expands to
/// several members, or two that collapse onto one — there is no single answer
/// and the caller reports it instead of picking.
fn enum_literal(expr: &str, enum_name: &str, tk: &Toolkit) -> Option<String> {
    let e = tk.unit_enum(enum_name)?;
    let members = union_members(&ts_type(&PropKind::UnitEnum { name: enum_name.into() }, false, tk));
    if members.len() != e.variants.len() {
        return None;
    }
    let wanted = squash(expr.trim().strip_prefix(&format!("{enum_name}::")).unwrap_or(expr.trim()));
    let idx = e.variants.iter().position(|v| squash(v) == wanted)?;
    members.get(idx).cloned()
}

/// Whitespace-insensitive comparison of two Rust variant spellings, so
/// `Kind(TermKind::Class)` matches `Kind( TermKind :: Class )`.
fn squash(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn union_members(ts: &str) -> Vec<String> {
    ts.split('|')
        .map(|m| m.trim().trim_matches(|c| c == '"' || c == '\'').to_string())
        .filter(|m| !m.is_empty())
        .collect()
}

/// `classify::ts_type` quotes union members with `"`; the package is written
/// with `'`. Only rewritten when the string holds no quote or escape of its
/// own, which is every type this crate produces (union members are kebab-cased
/// ASCII) and is checked rather than assumed.
fn requote(ts: &str) -> String {
    if ts.contains('\'') || ts.contains('\\') {
        ts.to_string()
    } else {
        ts.replace('"', "'")
    }
}

fn ts(kind: &PropKind, optional: bool, tk: &Toolkit) -> String {
    requote(&ts_type(kind, optional, tk))
}

/// A prop is optional in TypeScript when Yew would let the caller leave it out
/// (`#[prop_or*]`) *or* when its Rust type is already `Option<T>` — passing
/// `None` explicitly in Rust is `undefined` in TypeScript, and forcing a caller
/// to write `href={undefined}` buys nothing.
fn optional_marker(p: &Prop) -> &'static str {
    if p.optional || !matches!(p.default, PropDefault::Required) {
        "?"
    } else {
        ""
    }
}

fn default_note(p: &Prop) -> String {
    match &p.default {
        PropDefault::Required => String::new(),
        PropDefault::DefaultTrait if p.optional => "; `#[prop_or_default]`, so `None`".into(),
        PropDefault::DefaultTrait => "; `#[prop_or_default]`".into(),
        PropDefault::Expr { expr, .. } => format!("; `#[prop_or({expr})]`"),
    }
}

/// JavaScript reserved words that cannot be a destructuring binding. No prop in
/// the crate hits this today; it is checked because the failure mode otherwise
/// is a generated package that does not parse, with no hint of which Rust prop
/// caused it.
const RESERVED: &[&str] = &[
    "break", "case", "catch", "class", "const", "continue", "debugger", "default", "delete", "do",
    "else", "enum", "export", "extends", "false", "finally", "for", "function", "if", "import",
    "in", "instanceof", "new", "null", "return", "super", "switch", "this", "throw", "true", "try",
    "typeof", "var", "void", "while", "with", "yield",
];

fn reject_reserved(name: &str, at: &Span) -> Result<()> {
    if RESERVED.contains(&name) {
        bail!("the prop `{name}` at {} is a JavaScript reserved word", span(at));
    }
    Ok(())
}

// --------------------------------------------------------------------- types

fn uses_react_node(kind: &PropKind) -> bool {
    match kind {
        PropKind::Slot => true,
        PropKind::List { item } => uses_react_node(item),
        PropKind::Tuple { items } => items.iter().any(uses_react_node),
        _ => false,
    }
}

/// Every plain struct a set of props reaches, transitively.
fn struct_names<'a>(
    kinds: impl Iterator<Item = &'a PropKind>,
    tk: &Toolkit,
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for k in kinds {
        collect_structs(k, tk, &mut out);
    }
    out
}

fn collect_structs(kind: &PropKind, tk: &Toolkit, out: &mut BTreeSet<String>) {
    match kind {
        PropKind::Struct { name } => {
            if out.insert(name.clone()) {
                if let Some(s) = tk.plain_struct(name) {
                    for f in &s.fields {
                        collect_structs(&f.kind, tk, out);
                    }
                }
            }
        }
        PropKind::List { item } => collect_structs(item, tk, out),
        PropKind::Tuple { items } => {
            for i in items {
                collect_structs(i, tk, out);
            }
        }
        _ => {}
    }
}

fn types_ts(plan: &Plan<'_>, tk: &Toolkit) -> Result<Option<String>> {
    let names = struct_names(
        plan.emitted.iter().flat_map(|e| e.component.props.iter().map(|p| &p.kind)),
        tk,
    );
    if names.is_empty() {
        return Ok(None);
    }

    let mut s = String::new();
    writeln!(
        s,
        "// Generated by `{GENERATOR}` from the eona-ui-toolkit Yew crate. Do not edit."
    )?;
    writeln!(s, "//")?;
    writeln!(
        s,
        "// The plain structs a generated component's props reach. Field names are camel-cased from"
    )?;
    writeln!(
        s,
        "// Rust; each carries the `file:line` it came from, and is optional when"
    )?;
    writeln!(s, "// the Rust field is `Option<T>`.")?;
    for name in &names {
        let Some(st) = tk.plain_struct(name) else {
            bail!("a generated prop reaches the struct `{name}`, which the IR does not carry");
        };
        writeln!(s)?;
        writeln!(s, "/** `{name}` — {} */", span(&st.span))?;
        writeln!(s, "export interface {name} {{")?;
        for f in &st.fields {
            reject_reserved_field(&js_prop_name(&f.name), &f.span)?;
            writeln!(s, "  /** `{}` — {} */", f.rust_ty, span(&f.span))?;
            // Struct fields go through `js_prop_name` for the same reason props
            // do: `splice::js_suffix` camel-cases every path segment, so
            // `item.declType` has to find a `declType` here. Every field the
            // crate defines today is a single word, so this is currently a
            // no-op — which is exactly why it would otherwise be missed.
            writeln!(
                s,
                "  {}{}: {};",
                js_prop_name(&f.name),
                optional_marker(f),
                ts(&f.kind, f.optional, tk)
            )?;
        }
        writeln!(s, "}}")?;
    }
    Ok(Some(s))
}

/// An interface *property* may be any string, so unlike a destructured binding
/// a field named `default` (`src/molecules/props_table.rs:26`) is fine. Only a
/// name that is not a plain identifier needs quoting, and none occur.
fn reject_reserved_field(name: &str, at: &Span) -> Result<()> {
    let ok = !name.is_empty()
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !ok {
        bail!("the struct field `{name}` at {} is not a plain identifier", span(at));
    }
    Ok(())
}

// --------------------------------------------------------------------- index

fn index_ts(plan: &Plan<'_>, tk: &Toolkit) -> Result<String> {
    let mut s = String::new();
    writeln!(
        s,
        "// Generated by `{GENERATOR}` from the eona-ui-toolkit Yew crate. Do not edit."
    )?;
    writeln!(s, "// src_hash {}", tk.src_hash)?;
    writeln!(s)?;

    for e in plan.emitted.iter().filter(|e| e.withheld.is_none()) {
        let name = &e.component.name;
        let props_ty = e.component.props_ty.as_deref().unwrap_or_default();
        writeln!(s, "export {{ {name}, type {props_ty} }} from './{name}';")?;
    }

    let types = struct_names(
        plan.emitted.iter().flat_map(|e| e.component.props.iter().map(|p| &p.kind)),
        tk,
    );
    if !types.is_empty() {
        writeln!(s)?;
        writeln!(
            s,
            "export type {{ {} }} from './types';",
            types.iter().cloned().collect::<Vec<_>>().join(", ")
        )?;
    }

    // The omissions are written into the file rather than only into a report so
    // that a component losing its export shows up as a line *added* here, in the
    // same diff as the export it lost.
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
        let c = tk.component(name).map(|c| span(&c.span)).unwrap_or_else(|| "?".into());
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

// ------------------------------------------------------------------ manifests

fn package_json() -> Result<String> {
    // Built through `serde_json` so the file is valid JSON by construction;
    // `preserve_order` (xtask/Cargo.toml) keeps this insertion order, which is
    // what makes the bytes reproducible.
    let v = serde_json::json!({
        // package.json has no comments; `"//"` is npm's convention for one.
        "//": format!(
            "Generated by `{GENERATOR}` from the eona-ui-toolkit Yew crate. Do not edit — the \
             next bridgegen overwrites this file."
        ),
        "name": PACKAGE_NAME,
        "version": PACKAGE_VERSION,
        "description": "React port of the EONA-X design system, generated from the eona-ui-toolkit Yew crate.",
        "license": "Apache-2.0",
        "repository": {
            "type": "git",
            "url": "https://gitlab.eona-x.org/eona-x/web/eona-ui-toolkit",
            "directory": PACKAGE_DIR,
        },
        "type": "module",
        // Pure presentational output: no CSS import, no module-level effect, so
        // a bundler may drop anything the consumer does not reference.
        "sideEffects": false,
        "files": ["dist", "src", "README.md"],
        "types": "./dist/index.d.ts",
        "main": "./dist/index.cjs",
        // `dist/index.js`, not `dist/index.mjs`. Under `"type": "module"` tsup
        // writes the ESM bundle to `index.js` and only the CJS one gets a
        // non-default extension, so the previous `.mjs` spelling named a file
        // the `build` script below never produces and every ESM consumer failed
        // to resolve. Verified by running the script and listing `dist/`.
        "module": "./dist/index.js",
        "exports": {
            ".": {
                // Condition-first form so each entry carries its own types:
                // tsup emits `index.d.ts` beside the ESM bundle and
                // `index.d.cts` beside the CJS one, and pointing `require` at
                // the `.d.ts` makes a CJS consumer resolve ESM-shaped types.
                "import": {
                    "types": "./dist/index.d.ts",
                    "default": "./dist/index.js",
                },
                "require": {
                    "types": "./dist/index.d.cts",
                    "default": "./dist/index.cjs",
                },
            },
            "./package.json": "./package.json",
        },
        "scripts": {
            "build": "tsup src/index.ts --format esm,cjs --dts --out-dir dist",
            "typecheck": "tsc --noEmit",
        },
        // >=19, not >=18. `src/molecules/nav_dropdown.rs:29` and
        // `src/molecules/modal.rs:22` render `inert`, which React only added to
        // its DOM attribute types in 19; against `@types/react@18` the emitted
        // package does not typecheck. Measured, not assumed — those two files
        // are the only errors `tsc` reports under 18.
        "peerDependencies": { "react": ">=19" },
        // No `dependencies`. This used to declare `CSS_PACKAGE` as
        // `"workspace:*"`, which is a pnpm/yarn protocol npm rejects outright
        // (`EUNSUPPORTEDPROTOCOL`) — and there is no such package anywhere in
        // this repo to resolve it against: `packages/` holds only `react`, and
        // there is no root `package.json` or `pnpm-workspace.yaml`. So the
        // published manifest could never be installed. The stylesheet ships
        // from the Yew crate's `assets/` today; README.md says so and says what
        // to import once it is packaged.
        "devDependencies": {
            "@types/react": ">=19",
            "tsup": "^8",
            "typescript": "^5",
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
            // `classify::ts_type` widens an optional prop to `T | undefined`
            // precisely so this can stay on (xtask/src/classify.rs:88).
            "exactOptionalPropertyTypes": true,
            "strict": true,
            "jsx": "react-jsx",
            "declaration": true,
            "noEmit": true,
            "skipLibCheck": true,
        },
        "include": ["src"],
    });
    Ok(format!("{}\n", serde_json::to_string_pretty(&v)?))
}

fn readme_md(plan: &Plan<'_>, tk: &Toolkit) -> Result<String> {
    let mut s = String::new();
    writeln!(s, "<!-- Generated by `{GENERATOR}` from the eona-ui-toolkit Yew crate. Do not edit. -->")?;
    writeln!(s)?;
    writeln!(s, "# {PACKAGE_NAME}")?;
    writeln!(s)?;
    writeln!(
        s,
        "React components for the EONA-X design system. **This package is generated.** The source"
    )?;
    writeln!(
        s,
        "of truth is the Yew crate in this repository (`src/`): each component here was produced by"
    )?;
    writeln!(
        s,
        "rendering the Rust component with Yew's own server renderer and splicing the props back"
    )?;
    writeln!(
        s,
        "into the resulting markup, so the React and Yew renders are the same HTML by construction."
    )?;
    writeln!(s)?;
    writeln!(s, "Generated from src_hash `{}`.", tk.src_hash)?;
    writeln!(s)?;
    writeln!(s, "## Do not edit")?;
    writeln!(s)?;
    writeln!(
        s,
        "Every file under `src/` is overwritten by the next `{GENERATOR}`, and `cargo xtask check`"
    )?;
    writeln!(
        s,
        "re-runs the generator and diffs the result, so a hand edit is reported rather than"
    )?;
    writeln!(
        s,
        "surviving quietly. (It is a command, not yet a pipeline: the crate has no CI"
    )?;
    writeln!(s, "configuration — eona-x/backlog#808.)")?;
    writeln!(s, "To change a component, change the Rust component it came from — the `file:line` is")?;
    writeln!(s, "in the header of every generated file — and regenerate.")?;
    writeln!(s)?;
    writeln!(s, "## Install")?;
    writeln!(s)?;
    writeln!(
        s,
        "**Not published yet.** Packaging and publishing `{PACKAGE_NAME}` is Layer 4 of"
    )?;
    writeln!(
        s,
        "eona-x/backlog#809. Until it lands, copy `packages/react/src/*.tsx` into the app, or"
    )?;
    writeln!(
        s,
        "vendor this repository and run `npm install && npm run build` in `packages/react` first —"
    )?;
    writeln!(
        s,
        "every entry point in `package.json` is under `dist/`, which is generated and not committed."
    )?;
    writeln!(s)?;
    writeln!(s, "```sh")?;
    writeln!(s, "npm install {PACKAGE_NAME}")?;
    writeln!(s, "```")?;
    writeln!(s)?;
    writeln!(
        s,
        "`react >= 19` is a peer dependency: `NavDropdown` renders `inert`"
    )?;
    writeln!(
        s,
        "(`src/molecules/nav_dropdown.rs:29`), which React only added to its DOM attribute types"
    )?;
    writeln!(s, "in 19.")?;
    writeln!(s)?;
    writeln!(
        s,
        "The markup carries the design system's class names and is unstyled on its own. There is"
    )?;
    writeln!(
        s,
        "no `{CSS_PACKAGE}` package yet, so this package declares **no runtime dependency**."
    )?;
    writeln!(
        s,
        "Do not import the Yew crate's `assets/components.css` whole into an app meanwhile: it"
    )?;
    writeln!(
        s,
        "opens with a page-shell reset on `*`, `html`, `body`, `img`, `a`, `h1`-`h4`, `p` and"
    )?;
    writeln!(
        s,
        "`code` (`assets/components.css:13-30`) and sets `html > body {{ padding-top: 156px }}` at"
    )?;
    writeln!(
        s,
        "`:393`, so it resets the host's typography and displaces its layout. The published"
    )?;
    writeln!(
        s,
        "`{CSS_PACKAGE}` drops those rules and scopes the rest under `.eona-ui`, which the markup"
    )?;
    writeln!(
        s,
        "will then need as an ancestor (eona-x/backlog#808 Layer 3). `tokens.css` is safe as-is:"
    )?;
    writeln!(s, "it declares nothing but `:root` custom properties.")?;
    writeln!(s)?;
    writeln!(s, "```tsx")?;
    writeln!(s, "import {{ Button }} from '{PACKAGE_NAME}';")?;
    writeln!(s)?;
    writeln!(s, "export default function Page() {{")?;
    writeln!(s, "  return <Button label=\"Get started\" href=\"/docs\" />;")?;
    writeln!(s, "}}")?;
    writeln!(s, "```")?;
    writeln!(s)?;
    writeln!(
        s,
        "One component needs a file this package does not carry: `SiteHeader` hardcodes"
    )?;
    writeln!(
        s,
        "`logo/logo-transparent.svg` (`src/organisms/site_header.rs:101,105`) and must find it at"
    )?;
    writeln!(
        s,
        "that path relative to the page, or it renders a broken image where the EONA-X mark goes."
    )?;
    writeln!(s)?;
    writeln!(
        s,
        "`RuleListItem` needs an ancestor rather than an asset: every rule that styles it is"
    )?;
    writeln!(
        s,
        "scoped under `.rules-list`, so render it inside a `<ul className=\"rules-list\">`."
    )?;
    writeln!(s)?;
    writeln!(s, "## Server components")?;
    writeln!(s)?;
    writeln!(
        s,
        "Every component is presentational and holds no state, so none of them carries a"
    )?;
    writeln!(
        s,
        "client-component directive — the string does not occur anywhere in the package — and all"
    )?;
    writeln!(
        s,
        "of them render inside a React Server Component. The"
    )?;
    writeln!(
        s,
        "toolkit's stateful ontology browser is not here to port: it lives in `eona-vocabulary-ui`,"
    )?;
    writeln!(
        s,
        "which owns it outright. Anything browser-only belongs in a hand-written client component."
    )?;
    writeln!(s)?;
    writeln!(s, "## SSR output is equivalent, not byte-identical")?;
    writeln!(s)?;
    writeln!(
        s,
        "The elements, attributes and class names match the Yew render. The *serialisation* does"
    )?;
    writeln!(
        s,
        "not, in four measured ways, none of which changes what a browser shows. Do not write a"
    )?;
    writeln!(s, "snapshot test that diffs this package's SSR bytes against the Yew portal's:")?;
    writeln!(s)?;
    writeln!(
        s,
        "1. **Trailing `;` in `style`.** Yew emits the author's string verbatim"
    )?;
    writeln!(
        s,
        "   (`style=\"position:relative;\"`, `src/molecules/nav_dropdown.rs:24`); React serialises a"
    )?;
    writeln!(s, "   style *object* and writes no trailing semicolon.")?;
    writeln!(
        s,
        "2. **Whitespace in `style`.** React normalises `background :  #fff ` to `background:#fff`."
    )?;
    writeln!(
        s,
        "3. **Whitespace in `class`.** Yew's `Classes` collapses runs of whitespace; React does not,"
    )?;
    writeln!(
        s,
        "   so a prop containing a newline reaches the class attribute intact"
    )?;
    writeln!(s, "   (`src/atoms/code_block.rs:38` interpolates `language`).")?;
    writeln!(
        s,
        "4. **React's own additions.** React 19 hoists a `<link rel=\"preload\" as=\"image\">` for"
    )?;
    writeln!(
        s,
        "   every `<img src>` it renders, and omits `src=\"\"` entirely (with a warning) where Yew"
    )?;
    writeln!(s, "   emits it.")?;
    writeln!(s)?;

    let exported: Vec<&str> = plan
        .emitted
        .iter()
        .filter(|e| e.withheld.is_none())
        .map(|e| e.component.name.as_str())
        .collect();
    writeln!(s, "## Components ({})", exported.len())?;
    writeln!(s)?;
    for name in &exported {
        let at = tk.component(name).map(|c| span(&c.span)).unwrap_or_else(|| "?".into());
        writeln!(s, "- `{name}` — `{at}`")?;
    }

    let withheld: Vec<(&str, &str)> = plan
        .emitted
        .iter()
        .filter_map(|e| e.withheld.as_deref().map(|w| (e.component.name.as_str(), w)))
        .chain(plan.omitted.iter().map(|(c, r)| (c.name.as_str(), r.as_str())))
        .collect();
    if !withheld.is_empty() {
        writeln!(s)?;
        writeln!(s, "## Not exported ({})", withheld.len())?;
        writeln!(s)?;
        for (name, why) in &withheld {
            let at = tk.component(name).map(|c| span(&c.span)).unwrap_or_else(|| "?".into());
            writeln!(s, "- `{name}` — {why} (`{at}`)")?;
        }
    }

    if !plan.excluded.is_empty() {
        let total: usize = plan.excluded.values().map(Vec::len).sum();
        writeln!(s)?;
        writeln!(s, "## Out of scope ({total})")?;
        writeln!(s)?;
        writeln!(
            s,
            "Recorded here because a component that is missing on purpose and one that is missing"
        )?;
        writeln!(s, "by accident look identical from the outside.")?;
        writeln!(s)?;
        for (rule, names) in &plan.excluded {
            writeln!(s, "- **{rule}** ({}): {}", names.len(), names.join(", "))?;
        }
    }
    Ok(s)
}

// ---------------------------------------------------------------- filesystem

/// `src/*.ts[x]` files under the package that this run did not write.
fn stale_sources(pkg: &Path, files: &BTreeMap<String, String>) -> Result<Vec<String>> {
    let src = pkg.join("src");
    if !src.is_dir() {
        return Ok(Vec::new());
    }
    let mut stale = Vec::new();
    for entry in fs::read_dir(&src).with_context(|| format!("reading {}", src.display()))? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !(name.ends_with(".ts") || name.ends_with(".tsx")) {
            continue;
        }
        let rel = format!("src/{name}");
        if !files.contains_key(&rel) {
            stale.push(rel);
        }
    }
    stale.sort();
    Ok(stale)
}

fn span(s: &Span) -> String {
    format!("{}:{}", s.file, s.line)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(name: &str, line: usize, default: PropDefault, optional: bool) -> Prop {
        Prop {
            name: name.into(),
            rust_ty: if optional { "Option<AttrValue>".into() } else { "AttrValue".into() },
            kind: PropKind::Text,
            optional,
            default,
            span: Span { file: "src/atoms/button.rs".into(), line },
        }
    }

    fn button_toolkit() -> Toolkit {
        Toolkit {
            components: vec![Component {
                name: "Button".into(),
                props_ty: Some("ButtonProps".into()),
                tier: Tier::Presentational,
                status: Status::Ok,
                props: vec![
                    text("label", 21, PropDefault::Required, false),
                    Prop {
                        name: "variant".into(),
                        rust_ty: "ButtonVariant".into(),
                        kind: PropKind::UnitEnum { name: "ButtonVariant".into() },
                        optional: false,
                        default: PropDefault::Expr {
                            expr: "ButtonVariant::Primary".into(),
                            ts: None,
                        },
                        span: Span { file: "src/atoms/button.rs".into(), line: 23 },
                    },
                    text("href", 26, PropDefault::DefaultTrait, true),
                ],
                span: Span { file: "src/atoms/button.rs".into(), line: 30 },
            }],
            enums: vec![PropEnum {
                name: "ButtonVariant".into(),
                variants: vec!["Primary".into(), "Ghost".into()],
                default_variant: None,
                span: Span { file: "src/atoms/button.rs".into(), line: 5 },
            }],
            structs: Vec::new(),
            src_hash: "deadbeef".into(),
        }
    }

    fn button_jsx() -> BTreeMap<String, Jsx> {
        let body = "props.href ? (\n  <a className={`btn btn-${props.variant}`} href={props.href}>{props.label}</a>\n) : (\n  <button type=\"button\" className={`btn btn-${props.variant}`}>{props.label}</button>\n)";
        BTreeMap::from([("Button".into(), Jsx::new("Button", body))])
    }

    #[test]
    fn a_prop_or_expr_becomes_a_real_default_not_undefined() {
        let tk = button_toolkit();
        let (files, _) = render_package(&tk, &button_jsx()).unwrap();
        let tsx = &files["src/Button.tsx"];
        // The whole point: `#[prop_or(ButtonVariant::Primary)]` is a value, and
        // a React caller that omits `variant` must get that value.
        assert!(tsx.contains("variant: $props.variant ?? 'primary',"), "{tsx}");
        assert!(tsx.contains("variant?: 'primary' | 'ghost';"), "{tsx}");
        // Through a binding called `props`, not a destructuring pattern:
        // `splice` reads `props.variant` (`splice.rs:831`), which a destructured
        // default would leave undefined.
        assert!(tsx.contains("export function Button($props: ButtonProps) {"), "{tsx}");
        assert!(tsx.contains("...$props,"), "{tsx}");
        // `Option<AttrValue>` + `#[prop_or_default]` is `None`, which already is
        // `undefined`; normalising `href` to `''` would render an `<a>` where
        // Yew renders a `<button>` (src/atoms/button.rs:31-34).
        assert!(!tsx.contains("href: $props.href"), "{tsx}");
    }

    #[test]
    fn prop_or_default_on_a_value_type_is_that_types_default() {
        let mut tk = button_toolkit();
        tk.components[0].props = vec![
            text("placeholder", 18, PropDefault::DefaultTrait, false),
            Prop {
                name: "copyable".into(),
                rust_ty: "bool".into(),
                kind: PropKind::Bool,
                optional: false,
                default: PropDefault::DefaultTrait,
                span: Span { file: "src/atoms/button.rs".into(), line: 19 },
            },
            Prop {
                name: "variants".into(),
                rust_ty: "Vec<AttrValue>".into(),
                kind: PropKind::List { item: Box::new(PropKind::Text) },
                optional: false,
                default: PropDefault::DefaultTrait,
                span: Span { file: "src/atoms/button.rs".into(), line: 20 },
            },
        ];
        // Its own body: the props above replaced Button's, and `component_tsx`
        // refuses a body that reads a prop the interface does not declare.
        let jsx = BTreeMap::from([(
            "Button".to_string(),
            Jsx::new("Button", "<span>{props.placeholder}</span>"),
        )]);
        let (files, _) = render_package(&tk, &jsx).unwrap();
        let tsx = &files["src/Button.tsx"];
        assert!(tsx.contains("placeholder: $props.placeholder ?? '',"), "{tsx}");
        assert!(tsx.contains("copyable: $props.copyable ?? false,"), "{tsx}");
        assert!(tsx.contains("variants: $props.variants ?? [],"), "{tsx}");
    }

    #[test]
    fn prop_or_default_on_an_enum_is_the_variant_marked_default() {
        // OntoBadge's shape (`src/atoms/onto_badge.rs:24`): `#[prop_or_default]`
        // on an enum that derives `Default`. `PropEnum::default_variant` records
        // which variant carries it, and it resolves through the same positional
        // mapping `#[prop_or(..)]` uses, so the literal is by construction a
        // member of the union declared three lines above it.
        let mut tk = button_toolkit();
        tk.components[0].props[1].default = PropDefault::DefaultTrait;
        tk.enums[0].default_variant = Some("Ghost".into());
        let (files, _) = render_package(&tk, &button_jsx()).unwrap();
        let tsx = &files["src/Button.tsx"];
        assert!(tsx.contains("variant: $props.variant ?? 'ghost',"), "{tsx}");
        assert!(!tsx.contains("FIXME(bridgegen)"), "{tsx}");
    }

    #[test]
    fn a_default_the_ir_cannot_express_is_a_fixme_never_a_guess() {
        // The same prop on an enum that does *not* derive `Default`. There is no
        // answer, so there must be no initializer — a guess here is an API that
        // silently disagrees with Yew.
        let mut tk = button_toolkit();
        tk.components[0].props[1].default = PropDefault::DefaultTrait;
        tk.enums[0].default_variant = None;
        let (files, _) = render_package(&tk, &button_jsx()).unwrap();
        let tsx = &files["src/Button.tsx"];
        assert!(tsx.contains("FIXME(bridgegen): no default emitted"), "{tsx}");
        assert!(tsx.contains("it does not derive"), "{tsx}");
        // and emphatically not a made-up one
        assert!(!tsx.contains("variant: $props.variant ??"), "{tsx}");
    }

    /// The seam this integration pass had to repair. `splice` spells prop reads
    /// with `js_prop_name` and `emit` writes the interface; when they disagreed
    /// the package was every file of plausible TSX reading `props.downloadName`
    /// off an interface declaring `download_name`. Nothing but `tsc` noticed.
    #[test]
    fn a_body_that_reads_a_prop_the_interface_does_not_declare_is_an_error() {
        let tk = button_toolkit();
        let jsx = BTreeMap::from([(
            "Button".to_string(),
            Jsx::new("Button", "<span>{props.download_name}</span>"),
        )]);
        let err = format!("{:#}", render_package(&tk, &jsx).unwrap_err());
        assert!(err.contains("props.download_name"), "{err}");
        assert!(err.contains("ButtonProps does not declare"), "{err}");
    }

    #[test]
    fn a_nested_field_read_is_not_mistaken_for_a_prop() {
        // `props.props[0].default` reads the prop `props`; `default` belongs to
        // `types.ts` (`src/molecules/props_table.rs:36`).
        let reads = props_read("{props.props.map((item, i) => item.default)}");
        assert_eq!(reads.into_iter().collect::<Vec<_>>(), vec!["props".to_string()]);
        // `$props` is emit's own raw parameter, never something splice wrote.
        assert!(props_read("{...$props, variant: $props.variant ?? 'x'}").is_empty());
    }

    #[test]
    fn the_enum_default_is_read_off_the_union_it_must_belong_to() {
        let tk = button_toolkit();
        assert_eq!(enum_literal("ButtonVariant::Primary", "ButtonVariant", &tk).as_deref(), Some("primary"));
        assert_eq!(enum_literal("Ghost", "ButtonVariant", &tk).as_deref(), Some("ghost"));
        assert_eq!(enum_literal("ButtonVariant::Nope", "ButtonVariant", &tk), None);
        // A wrapped variant resolves to the leaf the union carries, so
        // `variant="class"` is `BadgeVariant::Kind(TermKind::Class)` and not a
        // nonexistent `BadgeVariant::Class`.
        let mut tk = tk;
        tk.enums.push(PropEnum {
            name: "BadgeVariant".into(),
            variants: vec!["Kind(TermKind::Class)".into(), "Lang".into()],
            default_variant: None,
            span: Span { file: "src/atoms/onto_badge.rs".into(), line: 20 },
        });
        assert_eq!(
            enum_literal("BadgeVariant::Kind(TermKind::Class)", "BadgeVariant", &tk).as_deref(),
            Some("class")
        );
    }

    #[test]
    fn quarantined_and_needs_override_are_named_in_the_index_not_dropped() {
        let mut tk = button_toolkit();
        let mut modal = tk.components[0].clone();
        modal.name = "Modal".into();
        modal.props_ty = Some("ModalProps".into());
        modal.props = vec![text("id", 14, PropDefault::Required, false)];
        modal.span = Span { file: "src/molecules/modal.rs".into(), line: 20 };
        modal.status = Status::NeedsOverride {
            reason: "inert hardcoded; needs open: bool upstream (#809)".into(),
        };
        let mut swatch = modal.clone();
        swatch.name = "Swatch".into();
        swatch.props_ty = Some("SwatchProps".into());
        swatch.span = Span { file: "src/molecules/swatch.rs".into(), line: 30 };
        swatch.status =
            Status::Quarantined { reason: "slices &hex[0..2] (src/molecules/swatch.rs:11)".into() };
        let mut hidden = modal.clone();
        hidden.name = "ThemeToggle".into();
        hidden.tier = Tier::Interactive;
        hidden.span = Span { file: "src/interactive/theme_toggle.rs".into(), line: 40 };
        hidden.status = Status::Excluded { reason: "interactive tier".into() };
        tk.components.extend([modal, swatch, hidden]);

        let mut jsx = button_jsx();
        jsx.insert("Modal".into(), Jsx::new("Modal", "<div id={id} />"));
        let (files, _) = render_package(&tk, &jsx).unwrap();
        let index = &files["src/index.ts"];
        assert!(index.contains("export { Button, type ButtonProps } from './Button';"));
        assert!(!index.contains("from './Modal'"), "{index}");
        assert!(index.contains("Modal — needs an override — inert hardcoded"), "{index}");
        assert!(index.contains("Swatch — quarantined — slices &hex[0..2]"), "{index}");
        assert!(index.contains("1 interactive tier: ThemeToggle"), "{index}");
        // The port is still written, and says on its first lines why it is dark.
        assert!(files["src/Modal.tsx"].contains("// Not re-exported from the package index"));
        assert!(!files.contains_key("src/Swatch.tsx"));
    }

    #[test]
    fn an_ok_component_with_no_jsx_fails_loudly() {
        let tk = button_toolkit();
        let err = render_package(&tk, &BTreeMap::new()).unwrap_err().to_string();
        assert!(err.contains("Button (src/atoms/button.rs:30)"), "{err}");
    }

    #[test]
    fn jsx_for_a_component_the_ir_does_not_know_fails() {
        let tk = button_toolkit();
        let mut jsx = button_jsx();
        jsx.insert("Ghost".into(), Jsx::new("Ghost", "<div />"));
        let err = render_package(&tk, &jsx).unwrap_err().to_string();
        assert!(err.contains("Ghost"), "{err}");
    }

    #[test]
    fn a_client_marker_anywhere_is_a_build_failure() {
        let tk = button_toolkit();
        let jsx = BTreeMap::from([(
            "Button".into(),
            Jsx::new("Button", "<button onClick={useState()[1]}>{label}</button>"),
        )]);
        let err = render_package(&tk, &jsx).unwrap_err().to_string();
        assert!(err.contains("useState("), "{err}");
        // and nothing in a clean package says it either — not even the README,
        // which describes the rule without quoting the directive.
        let (files, _) = render_package(&tk, &button_jsx()).unwrap();
        for (rel, contents) in &files {
            assert!(!contents.contains("use client"), "{rel}");
        }
    }

    #[test]
    fn output_is_byte_identical_whatever_order_it_is_fed_in() {
        let tk = button_toolkit();
        let mut shuffled = button_toolkit();
        shuffled.components.reverse();
        shuffled.enums.reverse();
        let (a, _) = render_package(&tk, &button_jsx()).unwrap();
        let (b, _) = render_package(&shuffled, &button_jsx()).unwrap();
        assert_eq!(a, b);
        for contents in a.values() {
            assert!(!contents.contains('\r'), "CRLF in the output");
        }
    }

    #[test]
    fn a_crlf_body_is_rejected_rather_than_written() {
        let tk = button_toolkit();
        let jsx = BTreeMap::from([("Button".into(), Jsx::new("Button", "<b>\r\n{label}</b>"))]);
        let err = render_package(&tk, &jsx).unwrap_err().to_string();
        assert!(err.contains("Button's JSX contains a CR"), "{err}");
    }

    #[test]
    fn package_json_is_valid_json_and_holds_the_agreed_fields() {
        let raw = package_json().unwrap();
        let v: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(v["name"], PACKAGE_NAME);
        assert_eq!(v["sideEffects"], false);
        assert_eq!(v["peerDependencies"]["react"], ">=19");
        // No runtime dependency at all. `CSS_PACKAGE` used to be declared here
        // as `"workspace:*"`, which npm rejects (`EUNSUPPORTEDPROTOCOL`) and
        // which named a package this repo does not contain, so the published
        // manifest could not be installed.
        assert!(v["dependencies"].is_null(), "{}", v["dependencies"]);
        assert!(!raw.contains("workspace:"), "{raw}");
        // The three paths below must name files the `build` script actually
        // produces. Under `"type": "module"` tsup writes `index.js` (ESM),
        // `index.cjs`, `index.d.ts` and `index.d.cts` — and nothing called
        // `index.mjs`, which is what the map used to point `import` at.
        assert_eq!(v["module"], "./dist/index.js");
        assert_eq!(v["exports"]["."]["import"]["types"], "./dist/index.d.ts");
        assert_eq!(v["exports"]["."]["import"]["default"], "./dist/index.js");
        assert_eq!(v["exports"]["."]["require"]["types"], "./dist/index.d.cts");
        assert_eq!(v["exports"]["."]["require"]["default"], "./dist/index.cjs");
        assert!(!raw.contains("index.mjs"), "{raw}");
        assert!(raw.ends_with("}\n"));
    }

    #[test]
    fn struct_props_get_an_interface_and_an_import() {
        let mut tk = button_toolkit();
        tk.structs.push(PlainStruct {
            name: "LiteralValue".into(),
            fields: vec![
                Prop {
                    name: "value".into(),
                    rust_ty: "AttrValue".into(),
                    kind: PropKind::Text,
                    optional: false,
                    default: PropDefault::Required,
                    span: Span { file: "src/ontology.rs".into(), line: 121 },
                },
                Prop {
                    name: "language".into(),
                    rust_ty: "Option<AttrValue>".into(),
                    kind: PropKind::Text,
                    optional: true,
                    default: PropDefault::Required,
                    span: Span { file: "src/ontology.rs".into(), line: 122 },
                },
            ],
            span: Span { file: "src/ontology.rs".into(), line: 120 },
        });
        tk.components[0].props.push(Prop {
            name: "values".into(),
            rust_ty: "Vec<LiteralValue>".into(),
            kind: PropKind::List {
                item: Box::new(PropKind::Struct { name: "LiteralValue".into() }),
            },
            optional: false,
            default: PropDefault::Required,
            span: Span { file: "src/molecules/annotation.rs".into(), line: 17 },
        });
        let (files, _) = render_package(&tk, &button_jsx()).unwrap();
        assert!(files["src/Button.tsx"].contains("import type { LiteralValue } from './types';"));
        let types = &files["src/types.ts"];
        assert!(types.contains("export interface LiteralValue {"), "{types}");
        assert!(types.contains("language?: string | undefined;"), "{types}");
        assert!(files["src/index.ts"].contains("export type { LiteralValue } from './types';"));
    }

    #[test]
    fn slot_props_import_react_node() {
        let mut tk = button_toolkit();
        tk.components[0].props.push(Prop {
            name: "children".into(),
            rust_ty: "Children".into(),
            kind: PropKind::Slot,
            optional: false,
            default: PropDefault::Required,
            span: Span { file: "src/atoms/pill.rs".into(), line: 22 },
        });
        let (files, _) = render_package(&tk, &button_jsx()).unwrap();
        let tsx = &files["src/Button.tsx"];
        assert!(tsx.contains("import type { ReactNode } from 'react';"), "{tsx}");
        assert!(tsx.contains("children: ReactNode;"), "{tsx}");
    }

    #[test]
    fn a_stale_component_file_is_swept() {
        let dir = std::env::temp_dir().join(format!("emit-stale-{}", std::process::id()));
        let src = dir.join(PACKAGE_DIR).join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("Gone.tsx"), "export const Gone = 1;\n").unwrap();
        fs::write(src.join("notes.md"), "kept\n").unwrap();
        emit(&button_toolkit(), &button_jsx(), &dir).unwrap();
        assert!(!src.join("Gone.tsx").exists());
        assert!(src.join("notes.md").exists(), "only .ts/.tsx under src/ is ours to sweep");
        assert!(src.join("Button.tsx").exists());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_signature_stays_one_line_however_many_props_there_are() {
        // Nothing to wrap any more: the props arrive as one object, and each
        // default gets its own line in the normalisation block. `SiteHeader`
        // and `Hero` are the wide ones in the real crate.
        let mut tk = button_toolkit();
        for i in 0..12 {
            tk.components[0]
                .props
                .push(text(&format!("a_long_prop_name_{i}"), 40 + i, PropDefault::Required, false));
        }
        let (files, _) = render_package(&tk, &button_jsx()).unwrap();
        let tsx = &files["src/Button.tsx"];
        assert!(tsx.contains("export function Button($props: ButtonProps) {\n"), "{tsx}");
        for line in tsx.lines() {
            assert!(line.len() <= 200, "{line}");
        }
    }

    /// The seam test: the real 42-component IR, not a fixture. Every `Ok`
    /// component must be emittable from nothing but the IR plus a body, because
    /// that is exactly what `splice` will hand over.
    #[test]
    fn the_real_crate_emits() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let mut tk = crate::parse::parse(&root).unwrap();
        crate::classify::classify(&mut tk).unwrap();

        let jsx: BTreeMap<String, Jsx> = tk
            .components
            .iter()
            .filter(|c| matches!(c.status, Status::Ok | Status::NeedsOverride { .. }))
            .map(|c| (c.name.clone(), Jsx::new(c.name.clone(), "<div className=\"stub\" />")))
            .collect();
        let (files, warnings) = render_package(&tk, &jsx).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");

        let ok = tk.components.iter().filter(|c| matches!(c.status, Status::Ok)).count();
        let tsx = files.keys().filter(|k| k.ends_with(".tsx")).count();
        // One .tsx per Ok component. There is no longer a written-but-withheld
        // case: #822 fixed the last of them, so the counts match exactly.
        assert_eq!(tsx, ok, "{:?}", files.keys().collect::<Vec<_>>());
        let index = &files["src/index.ts"];
        assert_eq!(index.matches("export { ").count(), ok, "{index}");

        // Every default in the crate is now expressible: `PropEnum::default_variant`
        // closed the last hole (`#[prop_or_default]` on `BadgeVariant`,
        // `src/atoms/onto_badge.rs:24`). A FIXME reappearing is a new hole, and
        // this is where it surfaces.
        let fixmes: Vec<&String> = files
            .iter()
            .filter(|(_, v)| v.contains("FIXME(bridgegen)"))
            .map(|(k, _)| k)
            .collect();
        assert!(fixmes.is_empty(), "{fixmes:?}");

        for (rel, contents) in &files {
            assert!(!contents.contains("use client"), "{rel}");
            assert!(!contents.contains('\r'), "{rel}");
            assert!(!contents.contains(root.to_str().unwrap()), "absolute path in {rel}");
        }
    }
}
