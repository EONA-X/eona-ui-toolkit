//! Stage: `matrix`. See xtask/README.md for the pipeline contract.
//!
//! Derives the **structural** render cells for a component: one cell per
//! combination of choices that can change the *shape* of the rendered markup,
//! never per combination that only changes text. Text travels through the
//! private-use sentinels and is spliced back afterwards, so two renders that
//! differ only in a string are the same cell here.
//!
//! The axes:
//!
//! - `Option<T>` — present / absent.
//! - `bool` — false / true.
//! - a fieldless enum — one cell per variant.
//! - `Vec<T>` — arities 0, 1, 2, 3. 0 and 1 bracket the empty case, 2 recovers
//!   the item template, and 3 is what proves a separator sits *between* items
//!   rather than after each one (`src/molecules/swatch.rs:46` joins on
//!   `" \u{b7} "`, which arity 2 alone cannot distinguish from a suffix).
//! - the fields of a `Struct` prop, and the fields of a `Vec<Struct>` item,
//!   recursively.
//!
//! That last axis is not a refinement, it is load-bearing.
//! `src/molecules/props_table.rs:59` branches `if let Some(default)` on
//! `PropSpec.default`, and `src/molecules/annotation.rs:33-41` matches three
//! ways on `LiteralValue.language` / `.datatype`. A matrix that varies only
//! top-level props renders exactly one arm of each of those, every downstream
//! safety check still passes because nothing is missing from what *was*
//! rendered, and the emitter hard-codes the arm it happened to see. Those two
//! branches are the whole reason `PathSeg::Item` exists.
//!
//! # Dominance
//!
//! Axes are not independent, and the two kinds of dependency are handled
//! deliberately differently.
//!
//! 1. **Structural dominance — always sound, applied by construction.** The
//!    axes under an `Option<T>` exist only when it is `Some`; the axes of a
//!    list item exist only at arity >= 1. The enumerator never emits the
//!    impossible combinations, so `Option<NavLink>`
//!    (`src/organisms/main_nav.rs:21`) is 2 cells and not 2 x |NavLink's axes|.
//!
//! 2. **Sibling dominance — a heuristic, and therefore falsifiable.** An
//!    optional prop named `<other>_<suffix>`, where `<other>` is another
//!    optional prop of the same struct, is treated as living inside `<other>`'s
//!    conditional. This is read off the IR by name: `matrix` does not re-parse
//!    Rust (the IR is the contract — see `xtask/src/ir.rs`) and it cannot
//!    compare markup either, because it runs before `render`. In this crate the
//!    rule fires exactly once, on `DatasetCardProps::badge_class`
//!    (`src/molecules/dataset_card.rs:36`), which `dataset_card.rs:56` does in
//!    fact only read inside `if let Some(badge)` on line 55 — collapsing that
//!    component's 64-cell cross product to 48.
//!
//!    Because it is a guess it keeps a **probe**: the first cell the edge
//!    pruned is put back. If the guess is wrong the probe's markup differs from
//!    the baseline's and `verify` can see it; if the guess is right the probe
//!    costs one redundant render. Pruning silently would make a wrong guess
//!    unfalsifiable, which is the same failure mode as silent truncation.
//!
//! Truncation is reported on stderr and in [`CellReport`], never applied
//! quietly: a chopped cross product that reads as full coverage is how a
//! generator ends up confidently wrong.

#![allow(dead_code, unused_imports)]

use std::collections::BTreeSet;
use std::fmt;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::ir::*;

/// List arities probed. See the module docs for why 3 is the last one.
const ARITIES: [usize; 4] = [0, 1, 2, 3];

/// How deep into `Struct` / `Vec<Struct>` fields axes are derived. The deepest
/// real chain in the toolkit is `PaletteGroupProps::colors` ->
/// `ColorSpec::variants` (`src/molecules/palette_group.rs:30` and `:10`), which
/// is depth 3; 4 leaves headroom without letting a self-referential type run
/// away. Hitting it is reported, not swallowed.
const MAX_NEST_DEPTH: usize = 4;

/// Per-component ceiling on emitted cells. Past this the enumerator drops the
/// full cross product for a one-factor-at-a-time design, which still covers
/// every single axis value but loses interactions between axes — and says so.
pub const MAX_CELLS_PER_COMPONENT: usize = 128;

// ---------------------------------------------------------------- paths

/// One step of the route from a component's props to an axis. `Item` steps
/// into every element of a list at once: cells are uniform over a list's
/// elements, so `values[].language` means "every element's `language`".
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PathSeg {
    Field(String),
    /// A tuple element, by position — `PropKind::Tuple` (`xtask/src/ir.rs`) is
    /// how `Vec<(&'static str, &'static str)>` at
    /// `src/molecules/nav_dropdown.rs:18` and `src/molecules/pillar_card.rs:9`
    /// reaches the matrix.
    Index(usize),
    Item,
}

/// Where an axis lives, e.g. `badge_class`, `values[].language`,
/// `colors[].variants`.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Path(pub Vec<PathSeg>);

impl Path {
    fn field(&self, name: &str) -> Path {
        let mut segs = self.0.clone();
        segs.push(PathSeg::Field(name.to_string()));
        Path(segs)
    }

    fn index(&self, n: usize) -> Path {
        let mut segs = self.0.clone();
        segs.push(PathSeg::Index(n));
        Path(segs)
    }

    fn item(&self) -> Path {
        let mut segs = self.0.clone();
        segs.push(PathSeg::Item);
        Path(segs)
    }

    /// The prop this axis ultimately hangs off, for grouping diagnostics.
    pub fn root(&self) -> Option<&str> {
        match self.0.first() {
            Some(PathSeg::Field(name)) => Some(name),
            _ => None,
        }
    }

    fn slug(&self) -> String {
        let mut out = String::new();
        for seg in &self.0 {
            match seg {
                PathSeg::Field(name) => {
                    if !out.is_empty() {
                        out.push('_');
                    }
                    out.push_str(name);
                }
                PathSeg::Index(n) => {
                    out.push('_');
                    out.push_str(&n.to_string());
                }
                PathSeg::Item => out.push_str("_item"),
            }
        }
        out
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for seg in &self.0 {
            match seg {
                PathSeg::Field(name) => {
                    if !first {
                        f.write_str(".")?;
                    }
                    f.write_str(name)?;
                }
                PathSeg::Index(n) => {
                    if !first {
                        f.write_str(".")?;
                    }
                    write!(f, "{n}")?;
                }
                PathSeg::Item => f.write_str("[]")?,
            }
            first = false;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------- cells

/// The value picked for one axis in one cell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "select", rename_all = "kebab-case")]
pub enum Selection {
    /// `Option<T>` left `None`.
    Absent,
    /// `Option<T>` filled with `Some`.
    Present,
    Bool(bool),
    /// A fieldless enum's variant, by its Rust name.
    Variant(String),
    /// A `Vec<T>` built with this many elements.
    Arity(usize),
}

impl Selection {
    fn slug(&self) -> String {
        match self {
            Selection::Absent => "none".into(),
            Selection::Present => "some".into(),
            Selection::Bool(false) => "false".into(),
            Selection::Bool(true) => "true".into(),
            Selection::Variant(v) => to_snake(v),
            Selection::Arity(n) => format!("n{n}"),
        }
    }
}

impl fmt::Display for Selection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Selection::Absent => f.write_str("None"),
            Selection::Present => f.write_str("Some"),
            Selection::Bool(v) => write!(f, "{v}"),
            Selection::Variant(v) => f.write_str(v),
            Selection::Arity(n) => write!(f, "len {n}"),
        }
    }
}

/// One axis selection, carrying the span of the field it came from so any
/// diagnostic mentioning a cell names a line a human can open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub path: Path,
    pub selection: Selection,
    /// The `Prop`/field declaration this axis was derived from.
    pub span: Span,
}

impl fmt::Display for Choice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.path, self.selection)
    }
}

/// One structural render: the props harness must build a value for every
/// `Choice`, and everything not named here is a sentinel-filled text leaf.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cell {
    pub choices: Vec<Choice>,
    /// Stable, filesystem- and identifier-safe. Unique within a component.
    pub id: String,
}

impl Cell {
    /// The selection at a path written the way [`Path`] displays it, e.g.
    /// `"values[].language"`. Downstream stages match on this rather than
    /// rebuilding `Path` values.
    pub fn get(&self, path: &str) -> Option<&Selection> {
        self.choices.iter().find(|c| c.path.to_string() == path).map(|c| &c.selection)
    }

    fn new(choices: Vec<Choice>, baseline: &[Choice]) -> Cell {
        let id = cell_id(&choices, baseline);
        Cell { choices, id }
    }
}

/// Ids name generated harness components and output files, so they have to be
/// short, stable and readable. Only the choices that differ from the baseline
/// cell are named: an id uniquely identifies a cell because no axis unlocks
/// sub-axes on its baseline branch (`None`, `false`, arity 0 and the first enum
/// variant are all leaves), so two cells agreeing on every non-baseline choice
/// are the same cell. `cells_with_report` still checks that, and errors rather
/// than dropping a colliding cell.
fn cell_id(choices: &[Choice], baseline: &[Choice]) -> String {
    let interesting: Vec<&Choice> = choices
        .iter()
        .filter(|c| {
            !baseline.iter().any(|b| b.path == c.path && b.selection == c.selection)
        })
        .collect();
    if interesting.is_empty() {
        return "base".to_string();
    }
    let raw = interesting
        .iter()
        .map(|c| format!("{}_{}", c.path.slug(), c.selection.slug()))
        .collect::<Vec<_>>()
        .join("__");
    let safe: String = raw
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch.to_ascii_lowercase() } else { '_' })
        .collect();
    // Ids name files and generated harness components, so keep them readable but
    // bounded; the hash keeps a truncated id unique.
    if safe.len() <= 100 {
        safe
    } else {
        format!("{}_{:08x}", &safe[..88], fnv1a(&safe))
    }
}

fn fnv1a(s: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in s.as_bytes() {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn to_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

// ---------------------------------------------------------------- reporting

/// A sibling-dominance edge: `dominated` is assumed to be readable only where
/// `dominator` is `Some`. Both spans are carried so the claim can be checked by
/// opening two lines.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DominanceEdge {
    pub dominated: Path,
    pub dominated_span: Span,
    pub dominator: Path,
    pub dominator_span: Span,
    /// The rule that produced the edge, named so a reader knows it is a guess.
    pub rule: String,
    /// Cells removed by this edge.
    pub pruned: usize,
    /// Id of the cell put back to falsify the edge, if any was pruned.
    pub probe: Option<String>,
}

/// Recorded when [`MAX_NEST_DEPTH`] stopped axis derivation. Anything here is a
/// branch the generator cannot see, so it is reported rather than assumed inert.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DepthLimit {
    pub path: Path,
    pub span: Span,
    pub ty: String,
}

/// Recorded when the cross product was dropped for a one-factor-at-a-time
/// design. Interactions between axes are lost; single axis values are not.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Truncation {
    pub product: u128,
    pub cap: usize,
    pub emitted: usize,
}

/// What [`cells_with_report`] had to decide, for the ABI and for stderr.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellReport {
    pub component: String,
    pub axes: usize,
    /// Size of the full cross product before dominance and before any cap.
    pub product: u128,
    pub emitted: usize,
    pub dominance: Vec<DominanceEdge>,
    pub depth_limited: Vec<DepthLimit>,
    pub truncated: Option<Truncation>,
}

impl CellReport {
    /// Anything a human should see even on a clean run.
    fn noteworthy(&self) -> bool {
        self.truncated.is_some() || !self.depth_limited.is_empty() || !self.dominance.is_empty()
    }

    fn emit(&self) {
        if let Some(t) = &self.truncated {
            eprintln!(
                "matrix: {}: TRUNCATED — cross product is {} cells, cap is {}; \
                 emitted {} one-factor-at-a-time cells. Every axis value is covered, \
                 no interaction between two axes is. Treat coverage claims accordingly.",
                self.component, t.product, t.cap, t.emitted
            );
        }
        for d in &self.depth_limited {
            eprintln!(
                "matrix: {}: DEPTH LIMIT at {} ({} at {}:{}) — nested axes below \
                 depth {} were not derived, so any branch on them is invisible to the \
                 generator.",
                self.component, d.path, d.ty, d.span.file, d.span.line, MAX_NEST_DEPTH
            );
        }
        for e in &self.dominance {
            eprintln!(
                "matrix: {}: dominance ({}) — {} ({}:{}) assumed to live inside \
                 {} ({}:{}); pruned {} cells, kept probe {}.",
                self.component,
                e.rule,
                e.dominated,
                e.dominated_span.file,
                e.dominated_span.line,
                e.dominator,
                e.dominator_span.file,
                e.dominator_span.line,
                e.pruned,
                e.probe.as_deref().unwrap_or("<none>"),
            );
        }
    }
}

// ---------------------------------------------------------------- axes

/// An axis and, where it has them, the axes that exist only under one of its
/// branches. Structural dominance is this nesting.
#[derive(Clone, Debug)]
enum Axis {
    /// `Option<T>`; `inner` are `T`'s own axes, live only under `Some`.
    Presence { path: Path, span: Span, inner: Vec<Axis> },
    Flag { path: Path, span: Span },
    Variants { path: Path, span: Span, variants: Vec<String> },
    /// `Vec<T>`; `item` are `T`'s axes, live only at arity >= 1 and applied
    /// uniformly to every element.
    Arity { path: Path, span: Span, item: Vec<Axis> },
}

impl Axis {
    fn path(&self) -> &Path {
        match self {
            Axis::Presence { path, .. }
            | Axis::Flag { path, .. }
            | Axis::Variants { path, .. }
            | Axis::Arity { path, .. } => path,
        }
    }
}

/// One branch of an axis: the choice, plus the axes it unlocks.
fn branches(axis: &Axis) -> Vec<(Choice, &[Axis])> {
    match axis {
        // Absent first, so the baseline cell of every component is the smallest
        // markup it can produce and every other cell reads as an addition to it.
        Axis::Presence { path, span, inner } => vec![
            (Choice { path: path.clone(), selection: Selection::Absent, span: span.clone() }, &[][..]),
            (
                Choice { path: path.clone(), selection: Selection::Present, span: span.clone() },
                inner.as_slice(),
            ),
        ],
        Axis::Flag { path, span } => vec![
            (Choice { path: path.clone(), selection: Selection::Bool(false), span: span.clone() }, &[][..]),
            (Choice { path: path.clone(), selection: Selection::Bool(true), span: span.clone() }, &[][..]),
        ],
        Axis::Variants { path, span, variants } => variants
            .iter()
            .map(|v| {
                (
                    Choice {
                        path: path.clone(),
                        selection: Selection::Variant(v.clone()),
                        span: span.clone(),
                    },
                    &[][..],
                )
            })
            .collect(),
        Axis::Arity { path, span, item } => ARITIES
            .iter()
            .map(|n| {
                let unlocked: &[Axis] = if *n == 0 { &[] } else { item.as_slice() };
                (
                    Choice {
                        path: path.clone(),
                        selection: Selection::Arity(*n),
                        span: span.clone(),
                    },
                    unlocked,
                )
            })
            .collect(),
    }
}

fn axis_size(axis: &Axis) -> u128 {
    branches(axis).iter().map(|(_, subs)| product_size(*subs)).fold(0u128, u128::saturating_add)
}

fn product_size(axes: &[Axis]) -> u128 {
    axes.iter().map(axis_size).fold(1u128, u128::saturating_mul)
}

/// Context threaded through axis derivation: the report to fill, and the struct
/// names already on the path so a self-referential type cannot recurse forever.
struct Ctx<'a> {
    tk: &'a Toolkit,
    report: &'a mut CellReport,
    stack: Vec<String>,
}

fn axes_for_fields(fields: &[Prop], base: &Path, depth: usize, cx: &mut Ctx) -> Result<Vec<Axis>> {
    let mut axes = Vec::new();
    for field in fields {
        let path = base.field(&field.name);
        let inner = axes_for_kind(&field.kind, &path, &field.span, &field.rust_ty, depth, cx)?;
        if field.optional {
            axes.push(Axis::Presence { path, span: field.span.clone(), inner });
        } else {
            axes.extend(inner);
        }
    }
    cx.report.dominance.extend(sibling_dominance(fields, base));
    Ok(axes)
}

fn axes_for_kind(
    kind: &PropKind,
    path: &Path,
    span: &Span,
    rust_ty: &str,
    depth: usize,
    cx: &mut Ctx,
) -> Result<Vec<Axis>> {
    match kind {
        // Text and numbers reach the render as sentinels; they never change the
        // markup's shape, only its leaves. Slots take one sentinel child. A
        // Callback cannot be pre-rendered at all, which is `classify`'s problem
        // to turn into `Status::NeedsOverride`, not an axis here.
        PropKind::Text | PropKind::Num { .. } | PropKind::Slot | PropKind::Callback { .. } => {
            Ok(Vec::new())
        }
        PropKind::Bool => Ok(vec![Axis::Flag { path: path.clone(), span: span.clone() }]),
        PropKind::UnitEnum { name } => {
            let Some(def) = cx.tk.unit_enum(name) else {
                bail!(
                    "{}:{}: `{}` is typed `{}` but enum `{}` is not in the IR; \
                     `matrix` cannot invent its variants",
                    span.file,
                    span.line,
                    path,
                    rust_ty,
                    name
                );
            };
            if def.variants.is_empty() {
                bail!(
                    "{}:{}: enum `{}` (declared {}:{}) has no variants, so `{}` has no \
                     value the harness could pass",
                    span.file,
                    span.line,
                    name,
                    def.span.file,
                    def.span.line,
                    path
                );
            }
            Ok(vec![Axis::Variants {
                path: path.clone(),
                span: span.clone(),
                variants: def.variants.clone(),
            }])
        }
        PropKind::List { item } => {
            let item_path = path.item();
            let item_axes = if depth + 1 > MAX_NEST_DEPTH {
                cx.report.depth_limited.push(DepthLimit {
                    path: item_path.clone(),
                    span: span.clone(),
                    ty: rust_ty.to_string(),
                });
                Vec::new()
            } else {
                axes_for_kind(item, &item_path, span, rust_ty, depth + 1, cx)?
            };
            Ok(vec![Axis::Arity { path: path.clone(), span: span.clone(), item: item_axes }])
        }
        PropKind::Tuple { items } => {
            // A tuple's elements are axes the same way a struct's fields are;
            // they just have no names, so the path carries the position.
            if depth + 1 > MAX_NEST_DEPTH {
                cx.report.depth_limited.push(DepthLimit {
                    path: path.clone(),
                    span: span.clone(),
                    ty: rust_ty.to_string(),
                });
                return Ok(Vec::new());
            }
            let mut axes = Vec::new();
            for (i, item) in items.iter().enumerate() {
                let elem = path.index(i);
                axes.extend(axes_for_kind(item, &elem, span, rust_ty, depth + 1, cx)?);
            }
            Ok(axes)
        }
        PropKind::Struct { name } => {
            let Some(def) = cx.tk.plain_struct(name) else {
                bail!(
                    "{}:{}: `{}` is typed `{}` but struct `{}` is not in the IR; \
                     its fields are matrix axes and cannot be skipped \
                     (see src/molecules/props_table.rs:59)",
                    span.file,
                    span.line,
                    path,
                    rust_ty,
                    name
                );
            };
            if depth + 1 > MAX_NEST_DEPTH {
                cx.report.depth_limited.push(DepthLimit {
                    path: path.clone(),
                    span: def.span.clone(),
                    ty: name.clone(),
                });
                return Ok(Vec::new());
            }
            if cx.stack.iter().any(|s| s == name) {
                // A struct reachable from itself. Stopping is the only option,
                // but it hides branches, so it is reported like a depth limit.
                cx.report.depth_limited.push(DepthLimit {
                    path: path.clone(),
                    span: def.span.clone(),
                    ty: format!("{name} (recursive)"),
                });
                return Ok(Vec::new());
            }
            cx.stack.push(name.clone());
            let axes = axes_for_fields(&def.fields, path, depth + 1, cx);
            cx.stack.pop();
            axes
        }
    }
}

/// The name-prefix rule described in the module docs. Deliberately narrow: only
/// optional-on-optional, only within one struct, only an exact `<a>_` prefix.
fn sibling_dominance(fields: &[Prop], base: &Path) -> Vec<DominanceEdge> {
    let mut edges = Vec::new();
    for dominated in fields.iter().filter(|f| f.optional) {
        for dominator in fields.iter().filter(|f| f.optional) {
            if dominator.name == dominated.name {
                continue;
            }
            if dominated.name.starts_with(&format!("{}_", dominator.name)) {
                edges.push(DominanceEdge {
                    dominated: base.field(&dominated.name),
                    dominated_span: dominated.span.clone(),
                    dominator: base.field(&dominator.name),
                    dominator_span: dominator.span.clone(),
                    rule: "name-prefix: optional `<other>_<suffix>` inside optional `<other>`"
                        .to_string(),
                    pruned: 0,
                    probe: None,
                });
            }
        }
    }
    edges
}

// ---------------------------------------------------------------- enumeration

fn expand(axes: &[Axis]) -> Vec<Vec<Choice>> {
    let mut out: Vec<Vec<Choice>> = vec![Vec::new()];
    for axis in axes {
        let mut next = Vec::with_capacity(out.len() * 2);
        for (choice, subs) in branches(axis) {
            let sub_combos = expand(subs);
            for prefix in &out {
                for sub in &sub_combos {
                    let mut combo = prefix.clone();
                    combo.push(choice.clone());
                    combo.extend(sub.iter().cloned());
                    next.push(combo);
                }
            }
        }
        out = next;
    }
    out
}

/// Every axis at its first branch, recursively. The smallest markup the
/// component can produce.
fn baseline(axes: &[Axis]) -> Vec<Choice> {
    let mut out = Vec::new();
    for axis in axes {
        let brs = branches(axis);
        let (choice, subs) = &brs[0];
        out.push(choice.clone());
        out.extend(baseline(*subs));
    }
    out
}

/// One-factor-at-a-time: the baseline, plus one cell per non-baseline branch of
/// every axis with everything else held at baseline. Covers every axis value;
/// covers no pair of them. Used only when the cross product blows the cap.
fn ofat(axes: &[Axis]) -> Vec<Vec<Choice>> {
    let mut out = vec![baseline(axes)];
    for (i, axis) in axes.iter().enumerate() {
        for (bi, (choice, subs)) in branches(axis).iter().enumerate() {
            for (si, sub) in ofat(*subs).into_iter().enumerate() {
                if bi == 0 && si == 0 {
                    continue; // already the baseline
                }
                let mut cell = Vec::new();
                for (j, other) in axes.iter().enumerate() {
                    if i == j {
                        cell.push(choice.clone());
                        cell.extend(sub.iter().cloned());
                    } else {
                        cell.extend(baseline(std::slice::from_ref(other)));
                    }
                }
                out.push(cell);
            }
        }
    }
    out
}

/// True when this combination is impossible under `edge`: the dominated prop
/// carries a value inside a conditional that is not taken.
fn violates(choices: &[Choice], edge: &DominanceEdge) -> bool {
    let dominated_present = choices
        .iter()
        .any(|c| c.path == edge.dominated && c.selection == Selection::Present);
    let dominator_absent = choices
        .iter()
        .any(|c| c.path == edge.dominator && c.selection == Selection::Absent);
    dominated_present && dominator_absent
}

// ---------------------------------------------------------------- entry points

/// The structural render cells for one component.
///
/// Pure in the component: status is the caller's filter (`Toolkit::generated`),
/// because a `Quarantined` component still wants its cells printed in a
/// diagnostic. A component with no structural axes gets exactly one cell.
pub fn cells(c: &Component, tk: &Toolkit) -> Result<Vec<Cell>> {
    let (cells, report) = cells_with_report(c, tk)?;
    if report.noteworthy() {
        report.emit();
    }
    Ok(cells)
}

/// [`cells`], with what it had to decide. `cells` prints this; callers that
/// aggregate (the ABI, `check`) want it as data.
pub fn cells_with_report(c: &Component, tk: &Toolkit) -> Result<(Vec<Cell>, CellReport)> {
    let mut report = CellReport { component: c.name.clone(), ..CellReport::default() };
    let axes = {
        let mut cx = Ctx { tk, report: &mut report, stack: Vec::new() };
        axes_for_fields(&c.props, &Path::default(), 0, &mut cx)?
    };
    report.axes = axes.len();
    report.product = product_size(&axes);

    let base = baseline(&axes);
    let combos = if report.product > MAX_CELLS_PER_COMPONENT as u128 {
        let combos = ofat(&axes);
        report.truncated = Some(Truncation {
            product: report.product,
            cap: MAX_CELLS_PER_COMPONENT,
            emitted: combos.len(),
        });
        combos
    } else {
        expand(&axes)
    };

    // Sibling dominance prunes, then puts back one probe per edge so a wrong
    // guess is visible downstream rather than invisible here.
    let mut kept: Vec<Vec<Choice>> = Vec::with_capacity(combos.len());
    let mut probes: Vec<Vec<Choice>> = Vec::new();
    for combo in combos {
        match report.dominance.iter_mut().find(|e| violates(&combo, e)) {
            Some(edge) => {
                edge.pruned += 1;
                if edge.probe.is_none() {
                    let cell = Cell::new(combo.clone(), &base);
                    edge.probe = Some(cell.id.clone());
                    probes.push(combo);
                }
            }
            None => kept.push(combo),
        }
    }
    kept.extend(probes);

    let mut out: Vec<Cell> = Vec::with_capacity(kept.len());
    for combo in kept {
        let cell = Cell::new(combo, &base);
        match out.iter().find(|c| c.id == cell.id) {
            // The OFAT design revisits the baseline; an exact repeat is a
            // duplicate render, not lost coverage, so drop it.
            Some(existing) if existing.choices == cell.choices => continue,
            Some(existing) => bail!(
                "{}: cells {:?} and {:?} share the id {:?}; downstream stages name \
                 harnesses and files by id, so this would silently drop a render",
                c.name,
                existing.choices.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
                cell.choices.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
                cell.id
            ),
            None => out.push(cell),
        }
    }
    if let Some(t) = report.truncated.as_mut() {
        t.emitted = out.len();
    }
    report.emitted = out.len();
    Ok((out, report))
}

/// Cells for every component the generator has not excluded, in IR order.
/// `Excluded` components are skipped — they are never rendered, so cells for
/// them would be noise in the ABI.
pub fn all_cells(tk: &Toolkit) -> Result<Vec<(String, Vec<Cell>, CellReport)>> {
    let mut out = Vec::new();
    for c in &tk.components {
        if matches!(c.status, Status::Excluded { .. }) {
            continue;
        }
        let (cells, report) = cells_with_report(c, tk)?;
        if report.noteworthy() {
            report.emit();
        }
        out.push((c.name.clone(), cells, report));
    }
    Ok(out)
}

/// Totals over [`all_cells`], which is what the render budget is actually
/// spent on.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixSummary {
    pub components: usize,
    pub total: usize,
    pub max: Option<(String, usize)>,
    /// Scaled by 2 so it stays exact for an even component count.
    pub median_x2: usize,
    pub per_component: Vec<(String, usize)>,
}

impl MatrixSummary {
    pub fn median(&self) -> f64 {
        self.median_x2 as f64 / 2.0
    }
}

pub fn summarise(tk: &Toolkit) -> Result<MatrixSummary> {
    let per: Vec<(String, usize)> =
        all_cells(tk)?.into_iter().map(|(name, cells, _)| (name, cells.len())).collect();
    let total = per.iter().map(|(_, n)| n).sum();
    let max = per.iter().max_by_key(|(_, n)| *n).cloned();
    let mut counts: Vec<usize> = per.iter().map(|(_, n)| *n).collect();
    counts.sort_unstable();
    let median_x2 = if counts.is_empty() {
        0
    } else if counts.len() % 2 == 1 {
        counts[counts.len() / 2] * 2
    } else {
        counts[counts.len() / 2 - 1] + counts[counts.len() / 2]
    };
    Ok(MatrixSummary {
        components: per.len(),
        total,
        max,
        median_x2,
        per_component: per,
    })
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    // The fixture below is a hand-transcribed IR for the 38 in-scope
    // components, field by field, from the crate's own source with the real
    // `file:line` of every declaration. `parse` is a sibling stage and not
    // wired up yet, and `matrix` must be testable without it — every span here
    // is checkable by opening the line it names. When `parse` lands, the same
    // assertions should hold against its output; if they stop holding, one of
    // the two is wrong and the spans say where to look.

    fn sp(file: &str, line: usize) -> Span {
        Span { file: file.to_string(), line }
    }

    fn p(name: &str, rust_ty: &str, kind: PropKind, optional: bool, file: &str, line: usize) -> Prop {
        Prop {
            name: name.to_string(),
            rust_ty: rust_ty.to_string(),
            kind,
            optional,
            // `matrix` never reads the default, only the shape.
            default: if optional { PropDefault::DefaultTrait } else { PropDefault::Required },
            span: sp(file, line),
        }
    }

    fn t(name: &str, file: &str, line: usize) -> Prop {
        p(name, "AttrValue", PropKind::Text, false, file, line)
    }
    fn ot(name: &str, file: &str, line: usize) -> Prop {
        p(name, "Option<AttrValue>", PropKind::Text, true, file, line)
    }
    fn bl(name: &str, file: &str, line: usize) -> Prop {
        p(name, "bool", PropKind::Bool, false, file, line)
    }
    fn en(name: &str, ty: &str, file: &str, line: usize) -> Prop {
        p(name, ty, PropKind::UnitEnum { name: ty.to_string() }, false, file, line)
    }
    fn lst(name: &str, item: PropKind, rust_ty: &str, file: &str, line: usize) -> Prop {
        p(name, rust_ty, PropKind::List { item: Box::new(item) }, false, file, line)
    }
    fn st(name: &str, ty: &str, file: &str, line: usize) -> Prop {
        p(name, ty, PropKind::Struct { name: ty.to_string() }, false, file, line)
    }
    fn ost(name: &str, ty: &str, file: &str, line: usize) -> Prop {
        p(name, &format!("Option<{ty}>"), PropKind::Struct { name: ty.to_string() }, true, file, line)
    }
    fn slot(name: &str, rust_ty: &str, file: &str, line: usize) -> Prop {
        p(name, rust_ty, PropKind::Slot, false, file, line)
    }

    fn comp(name: &str, file: &str, line: usize, props: Vec<Prop>) -> Component {
        Component {
            name: name.to_string(),
            props_ty: Some(format!("{name}Props")),
            tier: Tier::Presentational,
            status: Status::Ok,
            props,
            span: sp(file, line),
        }
    }

    fn excluded(name: &str, file: &str, line: usize, reason: &str) -> Component {
        Component {
            name: name.to_string(),
            props_ty: None,
            tier: Tier::Presentational,
            status: Status::Excluded { reason: reason.to_string() },
            props: Vec::new(),
            span: sp(file, line),
        }
    }

    fn enum_def(name: &str, variants: &[&str], file: &str, line: usize) -> PropEnum {
        PropEnum {
            name: name.to_string(),
            variants: variants.iter().map(|v| v.to_string()).collect(),
            default_variant: None,
            span: sp(file, line),
        }
    }

    fn fixture() -> Toolkit {
        let atoms_button = "src/atoms/button.rs";
        let atoms_pillb = "src/atoms/pill_button.rs";
        let onto_badge = "src/atoms/onto_badge.rs";
        let dataset = "src/molecules/dataset_card.rs";
        let annotation = "src/molecules/annotation.rs";
        let props_table = "src/molecules/props_table.rs";
        let palette = "src/molecules/palette_group.rs";
        let swatch = "src/molecules/swatch.rs";
        let main_nav = "src/organisms/main_nav.rs";
        let site_header = "src/organisms/site_header.rs";

        let components = vec![
            // ---- atoms
            comp("BadgeNum", "src/atoms/badge_num.rs", 11, vec![t("number", "src/atoms/badge_num.rs", 7)]),
            comp(
                "Button",
                atoms_button,
                30,
                vec![
                    t("label", atoms_button, 21),
                    en("variant", "ButtonVariant", atoms_button, 23),
                    ot("href", atoms_button, 26),
                ],
            ),
            comp(
                "CodeBlock",
                "src/atoms/code_block.rs",
                27,
                vec![
                    t("code", "src/atoms/code_block.rs", 13),
                    t("language", "src/atoms/code_block.rs", 19),
                    bl("copyable", "src/atoms/code_block.rs", 23),
                ],
            ),
            comp(
                "OntoBadge",
                onto_badge,
                66,
                vec![
                    t("label", onto_badge, 58),
                    en("variant", "BadgeVariant", onto_badge, 60),
                    ot("title", onto_badge, 62),
                ],
            ),
            comp(
                "PillButton",
                atoms_pillb,
                37,
                vec![
                    t("label", atoms_pillb, 24),
                    en("kind", "PillButtonKind", atoms_pillb, 26),
                    ot("href", atoms_pillb, 29),
                    bl("disabled", atoms_pillb, 33),
                ],
            ),
            comp(
                "Pill",
                "src/atoms/pill.rs",
                28,
                vec![
                    slot("children", "Children", "src/atoms/pill.rs", 22),
                    en("tone", "PillTone", "src/atoms/pill.rs", 24),
                ],
            ),
            comp(
                "TagLabel",
                "src/atoms/tag_label.rs",
                27,
                vec![
                    t("text", "src/atoms/tag_label.rs", 21),
                    en("tone", "TagLabelTone", "src/atoms/tag_label.rs", 23),
                ],
            ),
            // ---- molecules
            comp(
                "AccordionItem",
                "src/molecules/accordion_item.rs",
                17,
                vec![
                    t("title", "src/molecules/accordion_item.rs", 10),
                    slot("children", "Children", "src/molecules/accordion_item.rs", 11),
                    bl("open", "src/molecules/accordion_item.rs", 13),
                ],
            ),
            comp(
                "OntoAnnotation",
                annotation,
                44,
                vec![
                    t("label", annotation, 16),
                    lst(
                        "values",
                        PropKind::Struct { name: "LiteralValue".into() },
                        "Vec<LiteralValue>",
                        annotation,
                        17,
                    ),
                ],
            ),
            comp(
                "BgCheckItem",
                "src/molecules/bg_check_item.rs",
                16,
                vec![
                    t("label", "src/molecules/bg_check_item.rs", 6),
                    t("background", "src/molecules/bg_check_item.rs", 7),
                    t("color", "src/molecules/bg_check_item.rs", 8),
                    t("icon", "src/molecules/bg_check_item.rs", 10),
                    ot("border", "src/molecules/bg_check_item.rs", 12),
                ],
            ),
            comp(
                "Callout",
                "src/molecules/callout.rs",
                13,
                vec![
                    t("label", "src/molecules/callout.rs", 7),
                    t("french", "src/molecules/callout.rs", 8),
                    t("english", "src/molecules/callout.rs", 9),
                ],
            ),
            comp(
                "CaseCard",
                "src/molecules/case_card.rs",
                15,
                vec![
                    t("href", "src/molecules/case_card.rs", 9),
                    t("tag", "src/molecules/case_card.rs", 10),
                    t("title", "src/molecules/case_card.rs", 11),
                ],
            ),
            comp(
                "DatasetCard",
                dataset,
                42,
                vec![
                    t("title", dataset, 17),
                    t("href", dataset, 19),
                    ot("description", dataset, 21),
                    ot("version", dataset, 25),
                    ot("publisher", dataset, 27),
                    ot("thumbnail", dataset, 30),
                    ot("badge", dataset, 33),
                    ot("badge_class", dataset, 36),
                    t("action_label", dataset, 38),
                ],
            ),
            comp(
                "DeclCard",
                "src/molecules/decl_card.rs",
                17,
                vec![
                    t("category", "src/molecules/decl_card.rs", 8),
                    t("src", "src/molecules/decl_card.rs", 9),
                    t("alt", "src/molecules/decl_card.rs", 10),
                    t("decl_type", "src/molecules/decl_card.rs", 11),
                    t("title", "src/molecules/decl_card.rs", 12),
                    t("description", "src/molecules/decl_card.rs", 13),
                ],
            ),
            comp(
                "LogoDownloadCard",
                "src/molecules/logo_download_card.rs",
                22,
                vec![
                    t("label", "src/molecules/logo_download_card.rs", 10),
                    t("file", "src/molecules/logo_download_card.rs", 12),
                    t("download_name", "src/molecules/logo_download_card.rs", 14),
                    t("preview_style", "src/molecules/logo_download_card.rs", 16),
                    bl("is_transparent", "src/molecules/logo_download_card.rs", 18),
                ],
            ),
            comp("MemberCard", "src/molecules/member_card.rs", 11, vec![t("name", "src/molecules/member_card.rs", 7)]),
            comp(
                "MemberFilterCard",
                "src/molecules/member_filter_card.rs",
                18,
                vec![
                    t("name", "src/molecules/member_filter_card.rs", 11),
                    t("member_type", "src/molecules/member_filter_card.rs", 12),
                    t("country", "src/molecules/member_filter_card.rs", 13),
                    lst(
                        "sectors",
                        PropKind::Text,
                        "Vec<&'static str>",
                        "src/molecules/member_filter_card.rs",
                        14,
                    ),
                ],
            ),
            Component {
                // `inert={true}` is hardcoded at src/molecules/modal.rs:22, so the
                // open state is unrenderable until #809 adds `open: bool`.
                status: Status::NeedsOverride {
                    reason: "src/molecules/modal.rs:22 hardcodes inert={true}".into(),
                },
                ..comp(
                    "Modal",
                    "src/molecules/modal.rs",
                    20,
                    vec![
                        t("id", "src/molecules/modal.rs", 14),
                        t("title", "src/molecules/modal.rs", 15),
                        slot("children", "Children", "src/molecules/modal.rs", 16),
                    ],
                )
            },
            comp(
                "MvoCard",
                "src/molecules/mvo_card.rs",
                11,
                vec![
                    t("tag", "src/molecules/mvo_card.rs", 6),
                    t("description", "src/molecules/mvo_card.rs", 7),
                ],
            ),
            comp(
                "NavDropdown",
                "src/molecules/nav_dropdown.rs",
                22,
                vec![
                    t("id", "src/molecules/nav_dropdown.rs", 16),
                    t("label", "src/molecules/nav_dropdown.rs", 17),
                    lst(
                        "links",
                        PropKind::Tuple { items: vec![PropKind::Text, PropKind::Text] },
                        "Vec<(&'static str, &'static str)>",
                        "src/molecules/nav_dropdown.rs",
                        18,
                    ),
                ],
            ),
            comp(
                "NewsCard",
                "src/molecules/news_card.rs",
                16,
                vec![
                    t("href", "src/molecules/news_card.rs", 10),
                    t("tag", "src/molecules/news_card.rs", 11),
                    t("title", "src/molecules/news_card.rs", 12),
                ],
            ),
            comp(
                "PaletteGroup",
                palette,
                37,
                vec![
                    t("title", palette, 28),
                    t("description", palette, 29),
                    lst("colors", PropKind::Struct { name: "ColorSpec".into() }, "Vec<ColorSpec>", palette, 30),
                    ot("note", palette, 33),
                ],
            ),
            comp(
                "PersonalityCard",
                "src/molecules/personality_card.rs",
                11,
                vec![
                    t("tag", "src/molecules/personality_card.rs", 6),
                    t("description", "src/molecules/personality_card.rs", 7),
                ],
            ),
            comp(
                "PillarCard",
                "src/molecules/pillar_card.rs",
                13,
                vec![
                    t("num", "src/molecules/pillar_card.rs", 7),
                    t("title", "src/molecules/pillar_card.rs", 8),
                    lst(
                        "points",
                        PropKind::Tuple { items: vec![PropKind::Text, PropKind::Text] },
                        "Vec<(&'static str, &'static str)>",
                        "src/molecules/pillar_card.rs",
                        9,
                    ),
                ],
            ),
            comp(
                "PropsTable",
                props_table,
                38,
                vec![
                    t("component", props_table, 35),
                    lst("props", PropKind::Struct { name: "PropSpec".into() }, "Vec<PropSpec>", props_table, 36),
                ],
            ),
            comp(
                "RefCard",
                "src/molecules/ref_card.rs",
                14,
                vec![
                    t("src", "src/molecules/ref_card.rs", 7),
                    t("alt", "src/molecules/ref_card.rs", 8),
                    t("title", "src/molecules/ref_card.rs", 9),
                    t("description", "src/molecules/ref_card.rs", 10),
                ],
            ),
            comp(
                "RuleListItem",
                "src/molecules/rule_list_item.rs",
                28,
                vec![
                    en("kind", "RuleKind", "src/molecules/rule_list_item.rs", 23),
                    slot("children", "Children", "src/molecules/rule_list_item.rs", 24),
                ],
            ),
            comp(
                "SectionHead",
                "src/molecules/section_head.rs",
                15,
                vec![
                    t("number", "src/molecules/section_head.rs", 9),
                    t("kicker", "src/molecules/section_head.rs", 10),
                    t("title", "src/molecules/section_head.rs", 11),
                ],
            ),
            comp(
                "ShapeCard",
                "src/molecules/shape_card.rs",
                12,
                vec![
                    slot("demo", "Html", "src/molecules/shape_card.rs", 6),
                    t("title", "src/molecules/shape_card.rs", 7),
                    t("description", "src/molecules/shape_card.rs", 8),
                ],
            ),
            comp(
                "Swatch",
                swatch,
                33,
                vec![
                    t("hex", swatch, 23),
                    t("name", swatch, 24),
                    t("usage", swatch, 25),
                    lst("variants", PropKind::Text, "Vec<AttrValue>", swatch, 28),
                ],
            ),
            comp(
                "TextField",
                "src/molecules/text_field.rs",
                22,
                vec![
                    t("id", "src/molecules/text_field.rs", 13),
                    t("label", "src/molecules/text_field.rs", 14),
                    en("kind", "TextFieldKind", "src/molecules/text_field.rs", 16),
                    t("placeholder", "src/molecules/text_field.rs", 18),
                ],
            ),
            comp(
                "TypeScaleRow",
                "src/molecules/type_scale_row.rs",
                14,
                vec![
                    t("role", "src/molecules/type_scale_row.rs", 8),
                    slot("sample", "Html", "src/molecules/type_scale_row.rs", 9),
                    t("spec", "src/molecules/type_scale_row.rs", 10),
                ],
            ),
            comp(
                "UsageExample",
                "src/molecules/usage_example.rs",
                24,
                vec![
                    t("source", "src/molecules/usage_example.rs", 14),
                    bl("on_dark", "src/molecules/usage_example.rs", 18),
                    slot("children", "Html", "src/molecules/usage_example.rs", 20),
                ],
            ),
            // ---- organisms
            comp(
                "Hero",
                "src/organisms/hero.rs",
                35,
                vec![
                    en("variant", "HeroVariant", "src/organisms/hero.rs", 26),
                    t("eyebrow", "src/organisms/hero.rs", 27),
                    t("title_suffix", "src/organisms/hero.rs", 28),
                    t("lede", "src/organisms/hero.rs", 29),
                    slot("children", "Children", "src/organisms/hero.rs", 31),
                ],
            ),
            comp(
                "HeroFilter",
                "src/organisms/hero_filter.rs",
                34,
                vec![
                    t("title", "src/organisms/hero_filter.rs", 25),
                    t("description", "src/organisms/hero_filter.rs", 26),
                    slot("children", "Children", "src/organisms/hero_filter.rs", 28),
                    bl("with_backdrop", "src/organisms/hero_filter.rs", 30),
                ],
            ),
            comp(
                "MainNav",
                main_nav,
                26,
                vec![
                    ost("back", "NavLink", main_nav, 21),
                    lst("links", PropKind::Struct { name: "NavLink".into() }, "Vec<NavLink>", main_nav, 22),
                ],
            ),
            comp(
                "SimpleFooter",
                "src/organisms/simple_footer.rs",
                13,
                vec![
                    t("text", "src/organisms/simple_footer.rs", 8),
                    st("link", "NavLink", "src/organisms/simple_footer.rs", 9),
                ],
            ),
            comp(
                "SiteHeader",
                site_header,
                97,
                vec![
                    lst(
                        "links",
                        PropKind::Struct { name: "SiteNavLink".into() },
                        "Vec<SiteNavLink>",
                        site_header,
                        90,
                    ),
                    bl("show_login", site_header, 92),
                ],
            ),
            // ---- two of the exclusions, to prove `all_cells` skips them
            excluded("BeeNest", "src/organisms/beenest.rs", 313, "build.rs OUT_DIR type"),
            excluded("LogoSection", "src/organisms/logo_section.rs", 1, "zero-prop page section"),
        ];

        Toolkit {
            components,
            enums: vec![
                enum_def("ButtonVariant", &["Primary", "Ghost"], "src/atoms/button.rs", 5),
                enum_def(
                    "PillButtonKind",
                    &["Solid", "GlassLight", "GlassDark"],
                    "src/atoms/pill_button.rs",
                    6,
                ),
                enum_def("PillTone", &["Ok", "Bad"], "src/atoms/pill.rs", 6),
                enum_def("TagLabelTone", &["Default", "Blue"], "src/atoms/tag_label.rs", 5),
                enum_def("RuleKind", &["Do", "Dont", "Caution"], "src/molecules/rule_list_item.rs", 5),
                enum_def("TextFieldKind", &["Input", "Textarea"], "src/molecules/text_field.rs", 6),
                enum_def("HeroVariant", &["Full", "Compact"], "src/organisms/hero.rs", 8),
                // src/atoms/onto_badge.rs:20 — `Kind(TermKind)` carries a payload;
                // `classify` decides whether that is one axis value or eight. Here
                // it is the five top-level arms, which is what the css_class match
                // at :38 collapses to for markup purposes.
                enum_def(
                    "BadgeVariant",
                    &["Kind", "Lang", "Datatype", "Deprecated", "Default"],
                    "src/atoms/onto_badge.rs",
                    20,
                ),
            ],
            structs: vec![
                PlainStruct {
                    name: "LiteralValue".into(),
                    fields: vec![
                        p("value", "String", PropKind::Text, false, "src/ontology.rs", 40),
                        p("language", "Option<String>", PropKind::Text, true, "src/ontology.rs", 41),
                        p("datatype", "Option<String>", PropKind::Text, true, "src/ontology.rs", 42),
                    ],
                    span: sp("src/ontology.rs", 39),
                },
                PlainStruct {
                    name: "PropSpec".into(),
                    fields: vec![
                        t("name", "src/molecules/props_table.rs", 7),
                        t("ty", "src/molecules/props_table.rs", 8),
                        ot("default", "src/molecules/props_table.rs", 11),
                        t("doc", "src/molecules/props_table.rs", 12),
                    ],
                    span: sp("src/molecules/props_table.rs", 6),
                },
                PlainStruct {
                    name: "ColorSpec".into(),
                    fields: vec![
                        t("name", "src/molecules/palette_group.rs", 7),
                        t("hex", "src/molecules/palette_group.rs", 8),
                        t("usage", "src/molecules/palette_group.rs", 9),
                        lst(
                            "variants",
                            PropKind::Text,
                            "Vec<AttrValue>",
                            "src/molecules/palette_group.rs",
                            10,
                        ),
                    ],
                    span: sp("src/molecules/palette_group.rs", 6),
                },
                PlainStruct {
                    name: "NavLink".into(),
                    fields: vec![
                        t("href", "src/organisms/main_nav.rs", 5),
                        t("label", "src/organisms/main_nav.rs", 6),
                    ],
                    span: sp("src/organisms/main_nav.rs", 4),
                },
                PlainStruct {
                    name: "SiteNavLink".into(),
                    fields: vec![
                        t("href", "src/organisms/site_header.rs", 7),
                        t("label", "src/organisms/site_header.rs", 8),
                        bl("current", "src/organisms/site_header.rs", 9),
                    ],
                    span: sp("src/organisms/site_header.rs", 6),
                },
            ],
            src_hash: "fixture".into(),
        }
    }

    fn cells_of(tk: &Toolkit, name: &str) -> Vec<Cell> {
        let c = tk.component(name).unwrap_or_else(|| panic!("{name} missing from fixture"));
        cells(c, tk).unwrap_or_else(|e| panic!("{name}: {e:#}"))
    }

    /// The headline numbers. This prints rather than asserts a total, because a
    /// change in the toolkit's props legitimately changes it — what is asserted
    /// is that nothing silently lost coverage.
    #[test]
    fn report_totals() {
        let tk = fixture();
        let summary = summarise(&tk).expect("summarise");
        let mut rows = summary.per_component.clone();
        rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        eprintln!("--- matrix cells, {} in-scope components ---", summary.components);
        for (name, n) in &rows {
            eprintln!("{n:>4}  {name}");
        }
        eprintln!(
            "total {}  max {:?}  median {}",
            summary.total,
            summary.max,
            summary.median()
        );

        assert_eq!(summary.components, 38, "the in-scope set is 38 components");
        // Every component renders at least once.
        assert!(summary.per_component.iter().all(|(_, n)| *n >= 1));
        assert!(summary.total > 0);
    }

    /// `src/molecules/props_table.rs:59` branches `if let Some(default)` on a
    /// field of a `Vec<struct>` item. Without a cell on each side of that
    /// branch the emitter hard-codes whichever arm it saw.
    #[test]
    fn nested_option_in_list_item_is_an_axis() {
        let tk = fixture();
        let cells = cells_of(&tk, "PropsTable");
        let has = |arity: usize, sel: Selection| {
            cells.iter().any(|c| {
                c.get("props") == Some(&Selection::Arity(arity))
                    && c.get("props[].default") == Some(&sel)
            })
        };
        assert!(has(1, Selection::Present), "props_table.rs:59 Some(default) arm");
        assert!(has(1, Selection::Absent), "props_table.rs:59 else arm");
        assert!(has(2, Selection::Present), "item template with the Some arm repeated");
        // Arity 0 must not carry item choices: there is no item.
        assert!(cells
            .iter()
            .filter(|c| c.get("props") == Some(&Selection::Arity(0)))
            .all(|c| c.get("props[].default").is_none()));
    }

    /// `src/molecules/annotation.rs:33-41` matches three ways on
    /// `LiteralValue.language` / `.datatype`; all three arms must be reachable.
    #[test]
    fn annotation_covers_all_three_row_tag_arms() {
        let tk = fixture();
        let cells = cells_of(&tk, "OntoAnnotation");
        let arm = |lang: Selection, dt: Selection| {
            cells.iter().any(|c| {
                matches!(c.get("values"), Some(Selection::Arity(n)) if *n >= 1)
                    && c.get("values[].language") == Some(&lang)
                    && c.get("values[].datatype") == Some(&dt)
            })
        };
        // (Some(lang), _)
        assert!(arm(Selection::Present, Selection::Absent));
        assert!(arm(Selection::Present, Selection::Present));
        // (None, Some(datatype))
        assert!(arm(Selection::Absent, Selection::Present));
        // (None, None)
        assert!(arm(Selection::Absent, Selection::Absent));
    }

    /// `src/molecules/palette_group.rs:30` -> `ColorSpec::variants` ->
    /// `src/molecules/swatch.rs:46`'s `" \u{b7} "` join: the separator is only
    /// provable at arity 3, two levels down.
    #[test]
    fn nested_list_inside_list_item_reaches_arity_three() {
        let tk = fixture();
        let cells = cells_of(&tk, "PaletteGroup");
        assert!(cells.iter().any(|c| {
            c.get("colors") == Some(&Selection::Arity(1))
                && c.get("colors[].variants") == Some(&Selection::Arity(3))
        }));
        let reports = all_cells(&tk).unwrap();
        let pg = reports.iter().find(|(n, _, _)| n == "PaletteGroup").unwrap();
        assert!(pg.2.depth_limited.is_empty(), "PaletteGroup must not hit the depth limit");
    }

    /// Every list prop is probed at 0, 1, 2 and 3 — 3 is what separates a
    /// separator from a suffix.
    #[test]
    fn every_list_gets_four_arities() {
        let tk = fixture();
        for (name, path) in [
            ("Swatch", "variants"),
            ("MemberFilterCard", "sectors"),
            ("NavDropdown", "links"),
            ("PillarCard", "points"),
            ("MainNav", "links"),
            ("SiteHeader", "links"),
        ] {
            let cells = cells_of(&tk, name);
            for n in ARITIES {
                assert!(
                    cells.iter().any(|c| c.get(path) == Some(&Selection::Arity(n))),
                    "{name}.{path} missing arity {n}"
                );
            }
        }
    }

    /// Unit enums get one cell per variant; a bool gets both.
    #[test]
    fn enum_and_bool_axes() {
        let tk = fixture();
        let button = cells_of(&tk, "Button");
        for v in ["Primary", "Ghost"] {
            for href in [Selection::Absent, Selection::Present] {
                assert!(button.iter().any(|c| {
                    c.get("variant") == Some(&Selection::Variant(v.into()))
                        && c.get("href") == Some(&href)
                }));
            }
        }
        assert_eq!(button.len(), 4);

        let code = cells_of(&tk, "CodeBlock");
        assert_eq!(code.len(), 2, "only `copyable` changes CodeBlock's shape");

        let rule = cells_of(&tk, "RuleListItem");
        assert_eq!(rule.len(), 3, "RuleKind has three variants");
    }

    /// Text, numbers and slots are not axes: they are sentinel leaves.
    #[test]
    fn text_only_components_get_one_cell() {
        let tk = fixture();
        for name in ["Callout", "DeclCard", "RefCard", "SectionHead", "ShapeCard", "TypeScaleRow"] {
            assert_eq!(cells_of(&tk, name).len(), 1, "{name}");
        }
    }

    /// `src/molecules/dataset_card.rs:56` reads `badge_class` only inside
    /// `if let Some(badge)` on line 55. The prune must remove exactly the
    /// impossible combinations and keep one probe.
    #[test]
    fn dataset_card_dominance() {
        let tk = fixture();
        let c = tk.component("DatasetCard").unwrap();
        let (cells, report) = cells_with_report(c, &tk).unwrap();
        assert_eq!(report.product, 64, "six optional props");
        assert_eq!(report.dominance.len(), 1, "exactly one name-prefix edge");
        let edge = &report.dominance[0];
        assert_eq!(edge.dominated.to_string(), "badge_class");
        assert_eq!(edge.dominator.to_string(), "badge");
        assert_eq!(edge.pruned, 16, "half of the 32 badge-absent cells");
        assert!(edge.probe.is_some(), "a wrong guess must stay falsifiable");
        // 64 - 16 pruned + 1 probe.
        assert_eq!(cells.len(), 49);
        assert_eq!(report.emitted, 49);

        let impossible = |c: &Cell| {
            c.get("badge") == Some(&Selection::Absent)
                && c.get("badge_class") == Some(&Selection::Present)
        };
        assert_eq!(
            cells.iter().filter(|c| impossible(c)).count(),
            1,
            "only the probe may carry the impossible combination"
        );
        // The combination the dominance claim is about is still fully covered.
        assert!(cells.iter().any(|c| {
            c.get("badge") == Some(&Selection::Present)
                && c.get("badge_class") == Some(&Selection::Present)
        }));
    }

    /// The rule must not fire anywhere else in the toolkit; a second, unnoticed
    /// edge would be silently cutting real coverage.
    #[test]
    fn dominance_fires_exactly_once_across_the_crate() {
        let tk = fixture();
        let edges: Vec<(String, String)> = all_cells(&tk)
            .unwrap()
            .into_iter()
            .flat_map(|(name, _, r)| {
                r.dominance.into_iter().map(move |e| (name.clone(), e.dominated.to_string()))
            })
            .collect();
        assert_eq!(edges, vec![("DatasetCard".to_string(), "badge_class".to_string())]);
    }

    /// Structural dominance: an absent `Option<Struct>` unlocks no field axes,
    /// and a zero-length list unlocks no item axes.
    #[test]
    fn structural_dominance_prunes_unreachable_axes() {
        let tk = fixture();
        let cells = cells_of(&tk, "SiteHeader");
        assert!(cells
            .iter()
            .filter(|c| c.get("links") == Some(&Selection::Arity(0)))
            .all(|c| c.get("links[].current").is_none()));
        // 1 empty + 3 arities x 2 `current` values, crossed with show_login.
        assert_eq!(cells.len(), 14);
    }

    /// Ids name harness components and output files downstream, so they must be
    /// unique and safe.
    #[test]
    fn ids_are_unique_and_safe() {
        let tk = fixture();
        for (name, cells, _) in all_cells(&tk).unwrap() {
            let mut seen = BTreeSet::new();
            for cell in &cells {
                assert!(
                    cell.id.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_'),
                    "{name}: id {:?} is not identifier-safe",
                    cell.id
                );
                assert!(seen.insert(cell.id.clone()), "{name}: duplicate id {:?}", cell.id);
            }
            assert_eq!(cells[0].id, "base", "{name}: the first cell is the baseline");
        }
    }

    /// A missing enum or struct is a hard error, not a silently-dropped axis.
    #[test]
    fn unknown_types_fail_loudly() {
        let mut tk = fixture();
        tk.enums.retain(|e| e.name != "ButtonVariant");
        let err = cells(tk.component("Button").unwrap(), &tk).unwrap_err().to_string();
        assert!(err.contains("src/atoms/button.rs:23"), "{err}");
        assert!(err.contains("ButtonVariant"), "{err}");

        let mut tk = fixture();
        tk.structs.retain(|s| s.name != "PropSpec");
        let err = cells(tk.component("PropsTable").unwrap(), &tk).unwrap_err().to_string();
        assert!(err.contains("src/molecules/props_table.rs:36"), "{err}");
    }

    /// Past the cap the enumerator must degrade to a covering design and say so
    /// — not chop the tail off the cross product, which drops whole axis values.
    #[test]
    fn truncation_is_loud_and_still_covers_every_axis_value() {
        let file = "src/synthetic.rs";
        let props: Vec<Prop> = (0..10).map(|i| ot(&format!("opt{i}"), file, 10 + i)).collect();
        let tk = Toolkit {
            components: vec![comp("Synthetic", file, 1, props)],
            ..Toolkit::default()
        };
        let c = tk.component("Synthetic").unwrap();
        // `cells` is the loud path: it prints the truncation to stderr.
        assert!(!cells(c, &tk).unwrap().is_empty());
        let (cells, report) = cells_with_report(c, &tk).unwrap();
        assert_eq!(report.product, 1024);
        let truncation = report.truncated.as_ref().expect("must report truncation");
        assert_eq!(truncation.product, 1024);
        assert_eq!(truncation.emitted, cells.len());
        assert!(cells.len() <= MAX_CELLS_PER_COMPONENT);
        for i in 0..10 {
            let path = format!("opt{i}");
            for sel in [Selection::Absent, Selection::Present] {
                assert!(
                    cells.iter().any(|c| c.get(&path) == Some(&sel)),
                    "{path}={sel} lost to truncation"
                );
            }
        }
    }

    /// A self-referential struct must stop, and must be reported where it
    /// stopped rather than treated as having no axes.
    #[test]
    fn recursive_struct_stops_and_reports() {
        let file = "src/synthetic.rs";
        let tk = Toolkit {
            components: vec![comp("Tree", file, 1, vec![st("node", "Node", file, 2)])],
            structs: vec![PlainStruct {
                name: "Node".into(),
                fields: vec![
                    bl("leaf", file, 11),
                    lst("children", PropKind::Struct { name: "Node".into() }, "Vec<Node>", file, 12),
                ],
                span: sp(file, 10),
            }],
            ..Toolkit::default()
        };
        let (_, report) = cells_with_report(tk.component("Tree").unwrap(), &tk).unwrap();
        assert!(
            report.depth_limited.iter().any(|d| d.ty.contains("recursive") || d.ty.contains("Node")),
            "{report:?}"
        );
    }
}
