//! Stage: `abi`. See xtask/README.md for the pipeline contract.
//!
//! The committed contract. `bridgegen` writes 38 `.tsx` files plus their types;
//! nobody reviews 38 generated files on an MR, so this stage reduces the whole
//! surface to one sorted JSON document and `check` diffs *that*. A reviewer who
//! reads `abi/toolkit.abi.json` has seen every public name, type and default the
//! React package exposes.
//!
//! Two properties make that reduction trustworthy:
//!
//! - It is **total**. Every component the crate defines appears, carrying the
//!   `Status` that explains its fate. A component cannot leave the React package
//!   by disappearing from a file listing — it can only leave by changing status,
//!   which is one line in this file.
//! - It is **deterministic**. Components sort by name, props sort by name, types
//!   sort by name. Two runs over the same source produce byte-identical output,
//!   which is what lets `check` treat any difference at all as drift.
//!
//! [`diff_abi`] is the semver half: it answers "may this ship as a minor?" and
//! nothing else. It deliberately ignores `source` line moves — moving a
//! component down a file changes the artifact (and `check`'s unified diff shows
//! it) but changes no name, type or default, so it is not a semver event.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::classify::ts_type;
use crate::ir::*;
use crate::matrix::Cell;

/// Bumped when the *shape* of this document changes, so a stale
/// `abi/toolkit.abi.json` fails to deserialise loudly instead of diffing as if
/// a hundred components had changed at once.
pub const ABI_VERSION: u32 = 1;

// ---------------------------------------------------------------- the document

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Abi {
    pub abi_version: u32,
    /// `Toolkit::src_hash` — proves this document describes a specific `src/`.
    pub src_hash: String,
    /// sha256 over `xtask/src/**/*.rs`. The generator is as much an input to
    /// this document as the toolkit is: the same `src/` run through a changed
    /// `classify` yields different TS types. `check` needs both hashes to know
    /// whether it may trust the committed render results ([`generator_hash`]).
    pub generator_hash: String,
    /// Sorted by name. Includes excluded and quarantined components.
    pub components: Vec<AbiComponent>,
    /// Named types reachable from an in-scope component's props, sorted by name.
    /// A struct prop's TS type is a bare identifier (`PropSpec[]`), so without
    /// this section renaming `PropSpec::name` would be invisible to [`diff_abi`]
    /// — and it is a breaking change to the React API.
    pub types: Vec<AbiType>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbiComponent {
    pub name: String,
    /// `ir::Status` verbatim: the reason string is the rule that excluded or
    /// quarantined it, and losing it is how a deliberate exclusion becomes an
    /// oversight.
    #[serde(flatten)]
    pub status: Status,
    pub tier: Tier,
    /// `file:line` of the `#[function_component]`, openable as-is.
    pub source: String,
    /// `matrix::cells(c).len()` — how many renders stand behind this entry. A
    /// component whose prop count grew but whose cell count did not is a matrix
    /// axis that was silently dropped.
    pub cells: usize,
    /// Sorted by name.
    pub props: Vec<AbiMember>,
}

/// A prop of a component, or a field of a reachable struct. One type for both,
/// because they are the same thing to a React caller — `PropsTable` takes
/// `props: PropSpec[]`, so `PropSpec::name` is as public as `PropsTable::props`
/// — and because [`diff_abi`] can then compare them with one routine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbiMember {
    pub name: String,
    /// The Rust type verbatim, so a reviewer can tell `AttrValue` from `String`
    /// when both print as `string`.
    pub rust: String,
    pub ts: String,
    /// Rust `Option<T>`. Distinct from required-ness: `PropSpec::default` is
    /// `Option<AttrValue>` with no `#[prop_or*]`
    /// (`src/molecules/props_table.rs:11`), so the caller must supply it and may
    /// supply nothing.
    pub optional: bool,
    /// Required-ness is `default == "required"` and is deliberately not stored
    /// a second time — a committed artifact with two spellings of one fact can
    /// disagree with itself.
    #[serde(flatten)]
    pub default: AbiDefault,
    pub source: String,
}

/// `ir::PropDefault` with its `ts` field renamed.
///
/// This exists for one reason: `PropDefault::Expr` carries a field called `ts`
/// (the default *value* as a TS literal), and flattening it next to
/// [`AbiMember::ts`] (the prop's TS *type*) made the two collide — serde emitted
/// the default's `ts` last, so `Button.variant` serialised as `"ts": null` and
/// the union `"primary" | "ghost"` disappeared from the contract silently. Two
/// different facts cannot share one key, so the literal is `defaultTs` here.
/// `ir::PropDefault` is untouched; this is the ABI's own spelling of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "default", rename_all = "kebab-case")]
pub enum AbiDefault {
    Required,
    DefaultTrait,
    Expr {
        expr: String,
        #[serde(
            rename = "defaultTs",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        ts: Option<String>,
    },
}

impl From<&PropDefault> for AbiDefault {
    fn from(d: &PropDefault) -> Self {
        match d {
            PropDefault::Required => AbiDefault::Required,
            PropDefault::DefaultTrait => AbiDefault::DefaultTrait,
            PropDefault::Expr { expr, ts } => {
                AbiDefault::Expr { expr: expr.clone(), ts: ts.clone() }
            }
        }
    }
}

impl AbiMember {
    /// The caller must pass this prop. The `Breaking` rule "a required prop
    /// added" is this predicate.
    pub fn required(&self) -> bool {
        matches!(self.default, AbiDefault::Required)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbiType {
    pub name: String,
    pub source: String,
    #[serde(flatten)]
    pub body: AbiTypeBody,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AbiTypeBody {
    Struct { fields: Vec<AbiMember> },
    /// The TS union members in declaration order, which is also the order
    /// `classify::ts_type` inlines them into a prop's `ts`. Listed here as well
    /// so a narrowing is reviewable at the definition rather than only at each
    /// of the props that happen to mention it.
    Enum { union: Vec<String> },
}

impl Abi {
    pub fn component(&self, name: &str) -> Option<&AbiComponent> {
        self.components.iter().find(|c| c.name == name)
    }
    pub fn ty(&self, name: &str) -> Option<&AbiType> {
        self.types.iter().find(|t| t.name == name)
    }
    /// The components the React package actually contains.
    pub fn emitted(&self) -> impl Iterator<Item = &AbiComponent> {
        self.components.iter().filter(|c| matches!(c.status, Status::Ok))
    }
    pub fn quarantined(&self) -> BTreeSet<&str> {
        self.components
            .iter()
            .filter(|c| matches!(c.status, Status::Quarantined { .. }))
            .map(|c| c.name.as_str())
            .collect()
    }
}

// ---------------------------------------------------------------- building

fn source(span: &Span) -> String {
    format!("{}:{}", span.file, span.line)
}

fn member(p: &Prop, tk: &Toolkit) -> AbiMember {
    AbiMember {
        name: p.name.clone(),
        rust: p.rust_ty.clone(),
        ts: ts_type(&p.kind, p.optional, tk),
        optional: p.optional,
        default: (&p.default).into(),
        source: source(&p.span),
    }
}

fn sorted_members(props: &[Prop], tk: &Toolkit) -> Vec<AbiMember> {
    let mut out: Vec<AbiMember> = props.iter().map(|p| member(p, tk)).collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Named types reachable from the props of a component that is not `Excluded`.
///
/// Reachability rather than "every type in the crate": an unreachable struct is
/// not part of the React surface, and including it would make a change to it
/// look like a semver event in [`diff_abi`]. The walk descends through `List`
/// and `Tuple` because `Vec<PropSpec>` is how the crate's one nested-axis struct
/// is actually reached (`src/molecules/props_table.rs:36`).
fn reachable_types(tk: &Toolkit) -> Result<Vec<AbiType>> {
    let mut queue: Vec<PropKind> = Vec::new();
    for c in tk.components.iter().filter(|c| !matches!(c.status, Status::Excluded { .. })) {
        queue.extend(c.props.iter().map(|p| p.kind.clone()));
    }

    let mut structs: BTreeSet<String> = BTreeSet::new();
    let mut enums: BTreeSet<String> = BTreeSet::new();
    while let Some(kind) = queue.pop() {
        match kind {
            PropKind::Struct { name } => {
                if structs.insert(name.clone()) {
                    let s = tk.plain_struct(&name).with_context(|| {
                        format!(
                            "a prop resolves to struct `{name}`, which the IR does not carry; \
                             `parse` and `classify` disagree about the crate's types"
                        )
                    })?;
                    queue.extend(s.fields.iter().map(|f| f.kind.clone()));
                }
            }
            PropKind::UnitEnum { name } => {
                enums.insert(name);
            }
            PropKind::List { item } => queue.push(*item),
            PropKind::Tuple { items } => queue.extend(items),
            PropKind::Text
            | PropKind::Bool
            | PropKind::Num { .. }
            | PropKind::Slot
            | PropKind::Callback { .. } => {}
        }
    }

    let mut out = Vec::new();
    for name in &structs {
        let s = tk.plain_struct(name).expect("checked when queued");
        out.push(AbiType {
            name: s.name.clone(),
            source: source(&s.span),
            body: AbiTypeBody::Struct { fields: sorted_members(&s.fields, tk) },
        });
    }
    for name in &enums {
        let e = tk.unit_enum(name).with_context(|| {
            format!("a prop resolves to enum `{name}`, which the IR does not carry")
        })?;
        // The same rendering `classify::ts_type` inlines into every prop that
        // mentions this enum, parsed back out so the two can never disagree:
        // `BadgeVariant` expands `Kind(TermKind::Class)` to `"class"`
        // (`src/atoms/onto_badge.rs:20`), and re-deriving that here would be a
        // second implementation of a rule that already has one.
        let inline = ts_type(&PropKind::UnitEnum { name: e.name.clone() }, false, tk);
        out.push(AbiType {
            name: e.name.clone(),
            source: source(&e.span),
            body: AbiTypeBody::Enum { union: union_members(&inline) },
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// The document for this IR. `cells` is [`crate::harness::cells_for`]'s map; a
/// component absent from it was never rendered and records zero cells.
pub fn build_abi(tk: &Toolkit, cells: &BTreeMap<String, Vec<Cell>>) -> Result<Abi> {
    let mut components: Vec<AbiComponent> = tk
        .components
        .iter()
        .map(|c| AbiComponent {
            name: c.name.clone(),
            status: c.status.clone(),
            tier: c.tier,
            source: source(&c.span),
            cells: cells.get(&c.name).map_or(0, |v| v.len()),
            props: sorted_members(&c.props, tk),
        })
        .collect();
    components.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Abi {
        abi_version: ABI_VERSION,
        src_hash: tk.src_hash.clone(),
        generator_hash: generator_hash()?,
        components,
        types: reachable_types(tk)?,
    })
}

/// Pretty JSON with a trailing newline, which is the form `check` diffs and git
/// stores.
pub fn to_json(abi: &Abi) -> Result<String> {
    let json = serde_json::to_string_pretty(abi).context("serialising the ABI")?;
    Ok(format!("{json}\n"))
}

pub fn write_abi(tk: &Toolkit, cells: &BTreeMap<String, Vec<Cell>>, path: &Path) -> Result<()> {
    let abi = build_abi(tk, cells)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(path, to_json(&abi)?).with_context(|| format!("writing {}", path.display()))
}

pub fn read_abi(path: &Path) -> Result<Abi> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_abi(&text, &path.display().to_string())
}

pub fn parse_abi(text: &str, origin: &str) -> Result<Abi> {
    let abi: Abi = serde_json::from_str(text)
        .with_context(|| format!("{origin} is not a v{ABI_VERSION} ABI document"))?;
    if abi.abi_version != ABI_VERSION {
        anyhow::bail!(
            "{origin} is abiVersion {} but this generator writes {ABI_VERSION}; \
             regenerate it rather than diffing across versions",
            abi.abi_version
        );
    }
    Ok(abi)
}

/// sha256 over `xtask/src/**/*.rs`, built exactly like `parse::src_hash` builds
/// the toolkit's: sorted, length-prefixed, so no rename can collide with a
/// content change.
///
/// Every file is included, `check.rs` among them. `check` only ever verifies and
/// never produces an artifact, so hashing it is strictly conservative — the cost
/// is that editing the gate makes the next `check` re-render instead of taking
/// the hash-match fast path, which is a few seconds, once.
pub fn generator_hash() -> Result<String> {
    hash_rs_tree(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path())
}

pub fn hash_rs_tree(dir: &Path) -> Result<String> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in WalkDir::new(dir).follow_links(false) {
        let entry = entry.with_context(|| format!("walking {}", dir.display()))?;
        if !entry.file_type().is_file() || entry.path().extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(dir)
            .unwrap_or(entry.path())
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let bytes = std::fs::read(entry.path())
            .with_context(|| format!("reading {}", entry.path().display()))?;
        files.push((rel, bytes));
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (path, bytes) in &files {
        hasher.update((path.len() as u64).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// ---------------------------------------------------------------- the diff

/// Whether a change may ship as a minor. There is no third bucket: a change this
/// stage cannot prove is safe is `Breaking`, because the failure mode of
/// guessing wrong is a silent major.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    Additive,
    Breaking,
}

/// Which side of the surface a change is on, so a message can say
/// "component Button" rather than leaving the reader to infer it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SurfaceKind {
    Component,
    Struct,
    Enum,
}

impl SurfaceKind {
    fn label(self) -> &'static str {
        match self {
            SurfaceKind::Component => "component",
            SurfaceKind::Struct => "type",
            SurfaceKind::Enum => "union",
        }
    }
    /// A struct's members are fields; a component's are props.
    fn member(self) -> &'static str {
        match self {
            SurfaceKind::Component => "prop",
            _ => "field",
        }
    }
}

/// How a union moved. A widening is the only type change that is safely
/// additive, which is why it is the only one modelled precisely.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "union", rename_all = "kebab-case")]
pub enum UnionDelta {
    Widened { added: Vec<String> },
    Narrowed { removed: Vec<String> },
    /// Members both appeared and disappeared, or the type is not a union at all.
    /// Either way it is not a provable widening.
    Unrelated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "kebab-case")]
pub enum Change {
    SurfaceRemoved { was: Option<Status> },
    SurfaceAdded,
    /// Reported for its own sake as well as for semver: a component sliding from
    /// `Ok` to `Quarantined` is how the React package loses a component without
    /// anyone deleting a line.
    StatusChanged { from: Status, to: Status },
    MemberRemoved { member: String, ts: String },
    MemberAdded { member: String, ts: String, required: bool },
    MemberRenamed { from: String, to: String, ts: String },
    MemberTypeChanged { member: String, from: String, to: String, delta: UnionDelta },
    /// `required` is the *new* state.
    MemberRequiredChanged { member: String, required: bool },
    MemberDefaultChanged { member: String, from: String, to: String },
    /// A union type's own definition moved, independently of the props that
    /// mention it.
    UnionChanged { removed: Vec<String>, added: Vec<String> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiChange {
    pub severity: Severity,
    pub kind: SurfaceKind,
    /// The component or type the change is on.
    pub subject: String,
    #[serde(flatten)]
    pub change: Change,
}

impl std::fmt::Display for AbiChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sev = match self.severity {
            Severity::Breaking => "BREAKING",
            Severity::Additive => "additive",
        };
        let (k, s, m) = (self.kind.label(), &self.subject, self.kind.member());
        write!(f, "{sev:9} ")?;
        match &self.change {
            Change::SurfaceRemoved { was } => match was {
                Some(st) => write!(f, "{k} {s} removed (was {})", status_name(st)),
                None => write!(f, "{k} {s} removed"),
            },
            Change::SurfaceAdded => write!(f, "{k} {s} added"),
            Change::StatusChanged { from, to } => {
                write!(f, "{k} {s} status {} -> {}", status_name(from), status_name(to))
            }
            Change::MemberRemoved { member, ts } => {
                write!(f, "{k} {s}: {m} `{member}: {ts}` removed")
            }
            Change::MemberAdded { member, ts, required } => write!(
                f,
                "{k} {s}: {}{m} `{member}: {ts}` added",
                if *required { "required " } else { "" }
            ),
            Change::MemberRenamed { from, to, ts } => {
                write!(f, "{k} {s}: {m} `{from}` renamed to `{to}` ({ts})")
            }
            Change::MemberTypeChanged { member, from, to, delta } => {
                write!(f, "{k} {s}: {m} `{member}` {from} -> {to}")?;
                match delta {
                    UnionDelta::Widened { added } => write!(f, " (widened: +{})", added.join(", ")),
                    UnionDelta::Narrowed { removed } => {
                        write!(f, " (narrowed: -{})", removed.join(", "))
                    }
                    UnionDelta::Unrelated => Ok(()),
                }
            }
            Change::MemberRequiredChanged { member, required } => write!(
                f,
                "{k} {s}: {m} `{member}` became {}",
                if *required { "required" } else { "optional" }
            ),
            Change::MemberDefaultChanged { member, from, to } => {
                write!(f, "{k} {s}: {m} `{member}` default {from} -> {to}")
            }
            Change::UnionChanged { removed, added } => {
                write!(f, "{k} {s}:")?;
                if !removed.is_empty() {
                    write!(f, " -{}", removed.join(", "))?;
                }
                if !added.is_empty() {
                    write!(f, " +{}", added.join(", "))?;
                }
                Ok(())
            }
        }
    }
}

pub fn status_name(s: &Status) -> &'static str {
    match s {
        Status::Ok => "ok",
        Status::NeedsOverride { .. } => "needs-override",
        Status::Quarantined { .. } => "quarantined",
        Status::Excluded { .. } => "excluded",
    }
}

/// Whether a status puts the component in the React package. Only `Ok` is
/// emitted, so `NeedsOverride` is as absent from the package as `Excluded` is —
/// `Modal` is the live example (`src/molecules/modal.rs` hardcodes `inert`).
fn is_emitted(s: &Status) -> bool {
    matches!(s, Status::Ok)
}

/// Every difference between two ABIs, classified for semver.
///
/// Ordered by subject and then by the order the rules below run, so two runs
/// over the same pair produce the same list. `source` moves are not changes:
/// they alter the artifact (`check` shows them in its unified diff) but no name,
/// type or default, and treating a refactor as a semver event trains people to
/// ignore the tool.
pub fn diff_abi(old: &Abi, new: &Abi) -> Vec<AbiChange> {
    let mut out = Vec::new();

    let old_c: BTreeMap<&str, &AbiComponent> =
        old.components.iter().map(|c| (c.name.as_str(), c)).collect();
    let new_c: BTreeMap<&str, &AbiComponent> =
        new.components.iter().map(|c| (c.name.as_str(), c)).collect();

    for (name, o) in &old_c {
        let Some(n) = new_c.get(name) else {
            out.push(AbiChange {
                severity: Severity::Breaking,
                kind: SurfaceKind::Component,
                subject: (*name).to_string(),
                change: Change::SurfaceRemoved { was: Some(o.status.clone()) },
            });
            continue;
        };

        if o.status != n.status {
            // Entering the package is additive; leaving it removes every one of
            // its exports, which is exactly as breaking as deleting the file.
            let severity = match (is_emitted(&o.status), is_emitted(&n.status)) {
                (true, false) => Severity::Breaking,
                _ => Severity::Additive,
            };
            out.push(AbiChange {
                severity,
                kind: SurfaceKind::Component,
                subject: (*name).to_string(),
                change: Change::StatusChanged { from: o.status.clone(), to: n.status.clone() },
            });
        }

        // A component nobody can import has no API to break. Comparing its props
        // anyway would report breakage on something that is already absent and
        // drown the change that actually matters — the status line above.
        if is_emitted(&o.status) || is_emitted(&n.status) {
            diff_members(SurfaceKind::Component, name, &o.props, &n.props, &mut out);
        }
    }
    for name in new_c.keys() {
        if !old_c.contains_key(name) {
            out.push(AbiChange {
                severity: Severity::Additive,
                kind: SurfaceKind::Component,
                subject: (*name).to_string(),
                change: Change::SurfaceAdded,
            });
        }
    }

    let old_t: BTreeMap<&str, &AbiType> = old.types.iter().map(|t| (t.name.as_str(), t)).collect();
    let new_t: BTreeMap<&str, &AbiType> = new.types.iter().map(|t| (t.name.as_str(), t)).collect();

    for (name, o) in &old_t {
        let kind = type_kind(o);
        let Some(n) = new_t.get(name) else {
            out.push(AbiChange {
                severity: Severity::Breaking,
                kind,
                subject: (*name).to_string(),
                change: Change::SurfaceRemoved { was: None },
            });
            continue;
        };
        match (&o.body, &n.body) {
            (AbiTypeBody::Struct { fields: of }, AbiTypeBody::Struct { fields: nf }) => {
                diff_members(SurfaceKind::Struct, name, of, nf, &mut out);
            }
            (AbiTypeBody::Enum { union: ou }, AbiTypeBody::Enum { union: nu }) => {
                let os: BTreeSet<&str> = ou.iter().map(String::as_str).collect();
                let ns: BTreeSet<&str> = nu.iter().map(String::as_str).collect();
                let removed: Vec<String> = os.difference(&ns).map(|s| (*s).to_string()).collect();
                let added: Vec<String> = ns.difference(&os).map(|s| (*s).to_string()).collect();
                if !removed.is_empty() || !added.is_empty() {
                    out.push(AbiChange {
                        severity: if removed.is_empty() {
                            Severity::Additive
                        } else {
                            Severity::Breaking
                        },
                        kind,
                        subject: (*name).to_string(),
                        change: Change::UnionChanged { removed, added },
                    });
                }
            }
            // A struct that became an enum, or the reverse. No member-level
            // story survives that, so it is reported as the whole surface being
            // replaced.
            _ => {
                out.push(AbiChange {
                    severity: Severity::Breaking,
                    kind,
                    subject: (*name).to_string(),
                    change: Change::SurfaceRemoved { was: None },
                });
                out.push(AbiChange {
                    severity: Severity::Additive,
                    kind: type_kind(n),
                    subject: (*name).to_string(),
                    change: Change::SurfaceAdded,
                });
            }
        }
    }
    for (name, t) in &new_t {
        if !old_t.contains_key(name) {
            out.push(AbiChange {
                severity: Severity::Additive,
                kind: type_kind(t),
                subject: (*name).to_string(),
                change: Change::SurfaceAdded,
            });
        }
    }

    out
}

fn type_kind(t: &AbiType) -> SurfaceKind {
    match t.body {
        AbiTypeBody::Struct { .. } => SurfaceKind::Struct,
        AbiTypeBody::Enum { .. } => SurfaceKind::Enum,
    }
}

/// The props of one component, or the fields of one struct.
fn diff_members(
    kind: SurfaceKind,
    subject: &str,
    old: &[AbiMember],
    new: &[AbiMember],
    out: &mut Vec<AbiChange>,
) {
    let om: BTreeMap<&str, &AbiMember> = old.iter().map(|m| (m.name.as_str(), m)).collect();
    let nm: BTreeMap<&str, &AbiMember> = new.iter().map(|m| (m.name.as_str(), m)).collect();

    let gone: Vec<&AbiMember> = old.iter().filter(|m| !nm.contains_key(m.name.as_str())).collect();
    let fresh: Vec<&AbiMember> = new.iter().filter(|m| !om.contains_key(m.name.as_str())).collect();
    let renames = pair_renames(&gone, &fresh);

    let push = |out: &mut Vec<AbiChange>, severity, change| {
        out.push(AbiChange { severity, kind, subject: subject.to_string(), change })
    };

    for m in &gone {
        match renames.get(m.name.as_str()) {
            Some(to) => push(
                out,
                Severity::Breaking,
                Change::MemberRenamed {
                    from: m.name.clone(),
                    to: (*to).to_string(),
                    ts: m.ts.clone(),
                },
            ),
            None => push(
                out,
                Severity::Breaking,
                Change::MemberRemoved { member: m.name.clone(), ts: m.ts.clone() },
            ),
        }
    }
    let renamed_to: BTreeSet<&str> = renames.values().copied().collect();
    for m in &fresh {
        if renamed_to.contains(m.name.as_str()) {
            continue;
        }
        push(
            out,
            // Adding a required prop breaks every existing call site; adding an
            // optional one breaks none.
            if m.required() { Severity::Breaking } else { Severity::Additive },
            Change::MemberAdded {
                member: m.name.clone(),
                ts: m.ts.clone(),
                required: m.required(),
            },
        );
    }

    for (name, o) in &om {
        let Some(n) = nm.get(name) else { continue };
        if o.ts != n.ts {
            let delta = union_delta(&o.ts, &n.ts);
            push(
                out,
                match delta {
                    UnionDelta::Widened { .. } => Severity::Additive,
                    _ => Severity::Breaking,
                },
                Change::MemberTypeChanged {
                    member: (*name).to_string(),
                    from: o.ts.clone(),
                    to: n.ts.clone(),
                    delta,
                },
            );
        }
        if o.required() != n.required() {
            push(
                out,
                // Callers already pass it, so relaxing is safe; tightening
                // rejects every call site that omitted it.
                if n.required() { Severity::Breaking } else { Severity::Additive },
                Change::MemberRequiredChanged {
                    member: (*name).to_string(),
                    required: n.required(),
                },
            );
        } else if o.default != n.default {
            let (from, to) = (default_text(&o.default), default_text(&n.default));
            if from != to {
                push(
                    out,
                    // A different default renders differently but typechecks
                    // identically. Visible, not breaking.
                    Severity::Additive,
                    Change::MemberDefaultChanged { member: (*name).to_string(), from, to },
                );
            }
        }
    }
}

fn default_text(d: &AbiDefault) -> String {
    match d {
        AbiDefault::Required => "required".into(),
        AbiDefault::DefaultTrait => "Default::default()".into(),
        AbiDefault::Expr { expr, ts } => match ts {
            Some(lit) => format!("{expr} ({lit})"),
            None => expr.clone(),
        },
    }
}

/// Pair a disappeared member with an appeared one when their whole signature is
/// identical and the pairing is unambiguous.
///
/// A rename and a remove+add are both `Breaking`, so a wrong guess here cannot
/// change a semver verdict — only the wording of the message. That is why the
/// rule is allowed to be a heuristic at all, and why it declines when a
/// signature matches more than one candidate on either side.
fn pair_renames<'a>(
    gone: &[&'a AbiMember],
    fresh: &[&'a AbiMember],
) -> BTreeMap<&'a str, &'a str> {
    let sig =
        |m: &AbiMember| (m.rust.clone(), m.ts.clone(), m.optional, default_text(&m.default));
    let mut buckets: BTreeMap<_, (Vec<&'a AbiMember>, Vec<&'a AbiMember>)> = BTreeMap::new();
    for m in gone {
        buckets.entry(sig(m)).or_default().0.push(m);
    }
    for m in fresh {
        buckets.entry(sig(m)).or_default().1.push(m);
    }
    let mut out = BTreeMap::new();
    for (_, (g, f)) in buckets {
        if g.len() == 1 && f.len() == 1 {
            out.insert(g[0].name.as_str(), f[0].name.as_str());
        }
    }
    out
}

// ------------------------------------------------------------ union analysis

/// Split a TypeScript type into its top-level union members, honouring nesting
/// so that `("a" | "b")[]` is one member rather than two.
pub fn union_members(ts: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0usize;
    for (i, ch) in ts.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' | '>' => depth -= 1,
            '|' if depth == 0 => {
                out.push(ts[start..i].trim().to_string());
                start = i + ch.len_utf8();
            }
            _ => {}
        }
    }
    out.push(ts[start..].trim().to_string());
    out.retain(|m| !m.is_empty());
    out
}

/// `("a" | "b")[]` -> `"a" | "b"`, so an array of a union can be compared as the
/// union it is. `None` when the type is not a parenthesised array.
fn unwrap_array(ts: &str) -> Option<&str> {
    let inner = ts.strip_suffix("[]")?.trim();
    let inner = inner.strip_prefix('(')?.strip_suffix(')')?;
    Some(inner.trim())
}

/// How `new` relates to `old` as a set of union members.
///
/// `classify::ts_type` appends `| undefined` for a Rust `Option<T>`, so
/// optionality is just another member here: `string` -> `string | undefined` is
/// a widening and needs no special case, while dropping `| undefined` correctly
/// reads as a narrowing.
pub fn union_delta(old: &str, new: &str) -> UnionDelta {
    if let (Some(o), Some(n)) = (unwrap_array(old), unwrap_array(new)) {
        return union_delta(o, n);
    }
    let os: BTreeSet<String> = union_members(old).into_iter().collect();
    let ns: BTreeSet<String> = union_members(new).into_iter().collect();
    let removed: Vec<String> = os.difference(&ns).cloned().collect();
    let added: Vec<String> = ns.difference(&os).cloned().collect();
    match (removed.is_empty(), added.is_empty()) {
        (true, false) => UnionDelta::Widened { added },
        (false, true) => UnionDelta::Narrowed { removed },
        // Both directions at once is a replacement, not a widening; and
        // (true, true) is unreachable because `diff_members` only asks when the
        // strings differ. Neither is a widening.
        _ => UnionDelta::Unrelated,
    }
}

/// The version bump a change set requires.
pub fn required_bump(changes: &[AbiChange]) -> &'static str {
    if changes.iter().any(|c| c.severity == Severity::Breaking) {
        "major"
    } else if changes.is_empty() {
        "patch"
    } else {
        "minor"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(line: usize) -> Span {
        Span { file: "src/atoms/button.rs".into(), line }
    }

    fn text_prop(name: &str, optional: bool, default: PropDefault) -> Prop {
        Prop {
            name: name.into(),
            rust_ty: if optional { "Option<AttrValue>".into() } else { "AttrValue".into() },
            kind: PropKind::Text,
            optional,
            default,
            span: span(10),
        }
    }

    fn toolkit(components: Vec<Component>) -> Toolkit {
        Toolkit { components, src_hash: "deadbeef".into(), ..Default::default() }
    }

    fn component(name: &str, status: Status, props: Vec<Prop>) -> Component {
        Component {
            name: name.into(),
            props_ty: Some(format!("{name}Props")),
            tier: Tier::Presentational,
            status,
            props,
            span: span(30),
        }
    }

    fn abi_of(tk: &Toolkit) -> Abi {
        build_abi(tk, &BTreeMap::new()).unwrap()
    }

    #[test]
    fn components_and_props_sort_by_name() {
        let tk = toolkit(vec![
            component(
                "Zebra",
                Status::Ok,
                vec![
                    text_prop("zed", false, PropDefault::Required),
                    text_prop("alpha", false, PropDefault::Required),
                ],
            ),
            component("Alpha", Status::Ok, vec![]),
        ]);
        let abi = abi_of(&tk);
        assert_eq!(
            abi.components.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
            ["Alpha", "Zebra"]
        );
        assert_eq!(
            abi.component("Zebra")
                .unwrap()
                .props
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "zed"]
        );
    }

    #[test]
    fn serialising_is_byte_stable_and_round_trips() {
        let tk = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![text_prop("label", false, PropDefault::Required)],
        )]);
        let a = to_json(&abi_of(&tk)).unwrap();
        let b = to_json(&abi_of(&tk)).unwrap();
        assert_eq!(a, b, "two builds of one IR must be byte-identical or `check` is useless");
        assert_eq!(parse_abi(&a, "test").unwrap(), abi_of(&tk));
    }

    #[test]
    fn an_excluded_component_keeps_its_rule() {
        let tk = toolkit(vec![component(
            "DemoPage",
            Status::Excluded { reason: "crate showcase page".into() },
            vec![],
        )]);
        let abi = abi_of(&tk);
        assert_eq!(
            abi.component("DemoPage").unwrap().status,
            Status::Excluded { reason: "crate showcase page".into() }
        );
        // The whole point of the document being total: present, not missing.
        assert_eq!(abi.components.len(), 1);
        assert_eq!(abi.emitted().count(), 0);
    }

    #[test]
    fn a_nested_struct_reaches_the_types_section() {
        // `PropsTable` takes `Vec<PropSpec>` (src/molecules/props_table.rs:36);
        // without the walk through `List`, `PropSpec[]` would name a type the
        // document never defines.
        let mut tk = toolkit(vec![component(
            "PropsTable",
            Status::Ok,
            vec![Prop {
                name: "props".into(),
                rust_ty: "Vec<PropSpec>".into(),
                kind: PropKind::List {
                    item: Box::new(PropKind::Struct { name: "PropSpec".into() }),
                },
                optional: false,
                default: PropDefault::Required,
                span: span(36),
            }],
        )]);
        tk.structs.push(PlainStruct {
            name: "PropSpec".into(),
            fields: vec![
                text_prop("name", false, PropDefault::Required),
                text_prop("default", true, PropDefault::Required),
            ],
            span: Span { file: "src/molecules/props_table.rs".into(), line: 6 },
        });
        let abi = abi_of(&tk);
        assert_eq!(abi.component("PropsTable").unwrap().props[0].ts, "PropSpec[]");
        let AbiTypeBody::Struct { fields } = &abi.ty("PropSpec").unwrap().body else {
            panic!("PropSpec must be a struct");
        };
        assert_eq!(
            fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
            ["default", "name"]
        );
    }

    #[test]
    fn an_unreachable_struct_stays_out_of_the_surface() {
        let mut tk = toolkit(vec![component("Button", Status::Ok, vec![])]);
        tk.structs.push(PlainStruct { name: "NotAProp".into(), fields: vec![], span: span(1) });
        assert!(abi_of(&tk).ty("NotAProp").is_none());
    }

    #[test]
    fn a_struct_reachable_only_from_an_excluded_component_stays_out() {
        let mut tk = toolkit(vec![component(
            "BeeNest",
            Status::Excluded { reason: "build.rs OUT_DIR type".into() },
            vec![Prop {
                name: "spec".into(),
                rust_ty: "&'static BeeNestSpec".into(),
                kind: PropKind::Struct { name: "Unreachable".into() },
                optional: false,
                default: PropDefault::Required,
                span: span(29),
            }],
        )]);
        tk.structs.push(PlainStruct { name: "Unreachable".into(), fields: vec![], span: span(1) });
        assert!(abi_of(&tk).ty("Unreachable").is_none());
    }

    // ------------------------------------------------------------ diff rules

    fn one(old: &Toolkit, new: &Toolkit) -> AbiChange {
        let changes = diff_abi(&abi_of(old), &abi_of(new));
        assert_eq!(changes.len(), 1, "expected exactly one change, got {changes:#?}");
        changes.into_iter().next().unwrap()
    }

    #[test]
    fn an_identical_abi_produces_no_changes() {
        let tk = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![text_prop("label", false, PropDefault::Required)],
        )]);
        assert!(diff_abi(&abi_of(&tk), &abi_of(&tk)).is_empty());
        assert_eq!(required_bump(&[]), "patch");
    }

    #[test]
    fn removing_a_component_is_breaking() {
        let old = toolkit(vec![component("Button", Status::Ok, vec![])]);
        let new = toolkit(vec![]);
        let c = one(&old, &new);
        assert_eq!(c.severity, Severity::Breaking);
        assert!(matches!(c.change, Change::SurfaceRemoved { .. }));
    }

    #[test]
    fn adding_a_component_is_additive() {
        let old = toolkit(vec![]);
        let new = toolkit(vec![component("Button", Status::Ok, vec![])]);
        assert_eq!(one(&old, &new).severity, Severity::Additive);
    }

    #[test]
    fn removing_a_prop_is_breaking() {
        let old = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![
                text_prop("label", false, PropDefault::Required),
                text_prop("href", true, PropDefault::DefaultTrait),
            ],
        )]);
        let new = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![text_prop("label", false, PropDefault::Required)],
        )]);
        let c = one(&old, &new);
        assert_eq!(c.severity, Severity::Breaking);
        assert!(matches!(&c.change, Change::MemberRemoved { member, .. } if member == "href"));
    }

    #[test]
    fn adding_a_required_prop_is_breaking_and_an_optional_one_is_not() {
        let old = toolkit(vec![component("Button", Status::Ok, vec![])]);
        let required = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![text_prop("label", false, PropDefault::Required)],
        )]);
        let optional = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![text_prop("label", true, PropDefault::DefaultTrait)],
        )]);
        assert_eq!(one(&old, &required).severity, Severity::Breaking);
        assert_eq!(one(&old, &optional).severity, Severity::Additive);
    }

    #[test]
    fn a_rename_is_reported_as_a_rename_not_a_remove_plus_an_add() {
        let old = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![text_prop("label", false, PropDefault::Required)],
        )]);
        let new = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![text_prop("text", false, PropDefault::Required)],
        )]);
        let c = one(&old, &new);
        assert_eq!(c.severity, Severity::Breaking);
        let Change::MemberRenamed { from, to, .. } = &c.change else {
            panic!("expected a rename, got {:#?}", c.change);
        };
        assert_eq!((from.as_str(), to.as_str()), ("label", "text"));
    }

    #[test]
    fn an_ambiguous_rename_stays_a_remove_plus_an_add() {
        // Two identical signatures leave and two arrive: nothing distinguishes
        // the pairings, so the heuristic must decline rather than invent one.
        let old = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![
                text_prop("a", false, PropDefault::Required),
                text_prop("b", false, PropDefault::Required),
            ],
        )]);
        let new = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![
                text_prop("c", false, PropDefault::Required),
                text_prop("d", false, PropDefault::Required),
            ],
        )]);
        let changes = diff_abi(&abi_of(&old), &abi_of(&new));
        assert!(changes.iter().all(|c| !matches!(c.change, Change::MemberRenamed { .. })));
        assert_eq!(changes.len(), 4);
        assert!(changes.iter().all(|c| c.severity == Severity::Breaking));
    }

    fn enum_toolkit(variants: &[&str]) -> Toolkit {
        let mut tk = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![Prop {
                name: "variant".into(),
                rust_ty: "ButtonVariant".into(),
                kind: PropKind::UnitEnum { name: "ButtonVariant".into() },
                optional: false,
                default: PropDefault::Required,
                span: span(23),
            }],
        )]);
        tk.enums.push(PropEnum {
            name: "ButtonVariant".into(),
            variants: variants.iter().map(|v| (*v).to_string()).collect(),
            default_variant: None,
            span: span(5),
        });
        tk
    }

    #[test]
    fn narrowing_a_union_is_breaking_and_widening_is_additive() {
        let two = enum_toolkit(&["Primary", "Ghost"]);
        let three = enum_toolkit(&["Primary", "Ghost", "Danger"]);

        let widened = diff_abi(&abi_of(&two), &abi_of(&three));
        assert!(widened.iter().all(|c| c.severity == Severity::Additive), "{widened:#?}");
        assert_eq!(required_bump(&widened), "minor");

        let narrowed = diff_abi(&abi_of(&three), &abi_of(&two));
        assert!(narrowed.iter().any(|c| c.severity == Severity::Breaking), "{narrowed:#?}");
        assert_eq!(required_bump(&narrowed), "major");
        // Both the prop that mentions it and the union's own definition move,
        // and both are reported: the prop is where a caller breaks, the
        // definition is where the author made the decision.
        assert!(narrowed.iter().any(|c| matches!(c.change, Change::MemberTypeChanged { .. })));
        assert!(narrowed.iter().any(|c| matches!(c.change, Change::UnionChanged { .. })));
    }

    #[test]
    fn making_a_prop_optional_is_additive_and_the_reverse_is_breaking() {
        let required = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![text_prop("label", false, PropDefault::Required)],
        )]);
        let optional = toolkit(vec![component(
            "Button",
            Status::Ok,
            vec![text_prop("label", true, PropDefault::DefaultTrait)],
        )]);
        let relaxed = diff_abi(&abi_of(&required), &abi_of(&optional));
        assert!(relaxed.iter().all(|c| c.severity == Severity::Additive), "{relaxed:#?}");
        let tightened = diff_abi(&abi_of(&optional), &abi_of(&required));
        assert!(tightened.iter().any(|c| c.severity == Severity::Breaking), "{tightened:#?}");
    }

    #[test]
    fn losing_ok_status_is_breaking_even_though_the_props_are_unchanged() {
        let props = || vec![text_prop("label", false, PropDefault::Required)];
        let ok = toolkit(vec![component("Swatch", Status::Ok, props())]);
        let quarantined = toolkit(vec![component(
            "Swatch",
            Status::Quarantined { reason: "panics while rendering a sentinel".into() },
            props(),
        )]);
        let c = one(&ok, &quarantined);
        assert_eq!(c.severity, Severity::Breaking);
        assert!(matches!(c.change, Change::StatusChanged { .. }));
        // And the reverse restores a component nobody could import: additive.
        assert_eq!(one(&quarantined, &ok).severity, Severity::Additive);
    }

    #[test]
    fn a_prop_change_on_a_component_nobody_can_import_is_not_a_semver_event() {
        // Both sides `Excluded`, so no React caller exists to break. Reporting
        // it would bury the status lines that do matter.
        let ex = |props| {
            toolkit(vec![component(
                "Hero",
                Status::Excluded { reason: "zero-prop page section".into() },
                props,
            )])
        };
        let changes = diff_abi(
            &abi_of(&ex(vec![text_prop("a", false, PropDefault::Required)])),
            &abi_of(&ex(vec![])),
        );
        assert!(changes.is_empty(), "{changes:#?}");
    }

    #[test]
    fn a_struct_field_rename_is_breaking_through_the_types_section() {
        let build = |field: &str| {
            let mut tk = toolkit(vec![component(
                "PropsTable",
                Status::Ok,
                vec![Prop {
                    name: "props".into(),
                    rust_ty: "Vec<PropSpec>".into(),
                    kind: PropKind::List {
                        item: Box::new(PropKind::Struct { name: "PropSpec".into() }),
                    },
                    optional: false,
                    default: PropDefault::Required,
                    span: span(36),
                }],
            )]);
            tk.structs.push(PlainStruct {
                name: "PropSpec".into(),
                fields: vec![text_prop(field, false, PropDefault::Required)],
                span: span(6),
            });
            tk
        };
        let c = one(&build("name"), &build("label"));
        assert_eq!(c.kind, SurfaceKind::Struct);
        assert_eq!(c.severity, Severity::Breaking);
        assert!(matches!(c.change, Change::MemberRenamed { .. }));
    }

    #[test]
    fn a_changed_default_is_visible_but_not_breaking() {
        let with = |expr: &str| {
            toolkit(vec![component(
                "Button",
                Status::Ok,
                vec![text_prop(
                    "variant",
                    false,
                    PropDefault::Expr { expr: expr.into(), ts: None },
                )],
            )])
        };
        let c = one(&with("ButtonVariant::Primary"), &with("ButtonVariant::Ghost"));
        assert_eq!(c.severity, Severity::Additive);
        assert!(matches!(c.change, Change::MemberDefaultChanged { .. }));
    }

    #[test]
    fn a_line_move_is_not_a_semver_event() {
        let at = |line| {
            toolkit(vec![Component {
                span: Span { file: "src/atoms/button.rs".into(), line },
                ..component(
                    "Button",
                    Status::Ok,
                    vec![text_prop("label", false, PropDefault::Required)],
                )
            }])
        };
        assert!(diff_abi(&abi_of(&at(30)), &abi_of(&at(47))).is_empty());
    }

    // ------------------------------------------------------------ union math

    #[test]
    fn union_members_respect_nesting_and_quotes() {
        assert_eq!(union_members("\"a\" | \"b\""), ["\"a\"", "\"b\""]);
        assert_eq!(union_members("(\"a\" | \"b\")[]"), ["(\"a\" | \"b\")[]"]);
        assert_eq!(union_members("string | undefined"), ["string", "undefined"]);
        // A pipe inside a string literal is not a separator.
        assert_eq!(union_members("\"a|b\""), ["\"a|b\""]);
    }

    #[test]
    fn optionality_reads_as_a_union_member() {
        assert!(matches!(union_delta("string", "string | undefined"), UnionDelta::Widened { .. }));
        assert!(matches!(union_delta("string | undefined", "string"), UnionDelta::Narrowed { .. }));
    }

    #[test]
    fn an_array_of_a_union_is_compared_as_the_union() {
        assert!(matches!(
            union_delta("(\"a\" | \"b\")[]", "(\"a\" | \"b\" | \"c\")[]"),
            UnionDelta::Widened { .. }
        ));
        assert!(matches!(
            union_delta("(\"a\" | \"b\")[]", "(\"a\")[]"),
            UnionDelta::Narrowed { .. }
        ));
    }

    #[test]
    fn an_unrelated_type_change_is_not_a_widening() {
        assert_eq!(union_delta("string", "number"), UnionDelta::Unrelated);
        assert_eq!(union_delta("PropSpec[]", "NavLink[]"), UnionDelta::Unrelated);
    }

    // ------------------------------------------------------- the real crate

    fn real() -> Option<(Toolkit, BTreeMap<String, Vec<Cell>>)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent()?.to_path_buf();
        if !root.join("src").is_dir() {
            return None;
        }
        let mut tk = crate::parse::parse(&root).unwrap();
        crate::classify::classify(&mut tk).unwrap();
        let cells = crate::harness::cells_for(&tk).unwrap();
        Some((tk, cells))
    }

    #[test]
    fn the_real_crate_produces_a_total_and_deterministic_abi() {
        let Some((tk, cells)) = real() else { return };
        let abi = build_abi(&tk, &cells).unwrap();

        assert_eq!(abi.components.len(), tk.components.len(), "the ABI must be total");
        assert_eq!(abi.src_hash, tk.src_hash);
        assert_eq!(to_json(&abi).unwrap(), to_json(&build_abi(&tk, &cells).unwrap()).unwrap());

        let in_scope = abi
            .components
            .iter()
            .filter(|c| !matches!(c.status, Status::Excluded { .. }))
            .count();
        assert_eq!(in_scope, 38, "#809 scopes exactly 38 components");

        // Every in-scope component must have rendered at least one cell; a zero
        // there is an axis the matrix dropped, not a component with no props.
        for c in abi.components.iter().filter(|c| !matches!(c.status, Status::Excluded { .. })) {
            assert!(c.cells > 0, "{} is in scope but records no render cells", c.name);
        }
        // And every excluded one must carry the rule that excluded it.
        for c in &abi.components {
            if let Status::Excluded { reason } = &c.status {
                assert!(!reason.is_empty(), "{} is excluded with no rule", c.name);
            }
        }
    }

    #[test]
    fn the_real_crate_defines_every_type_its_props_mention() {
        let Some((tk, cells)) = real() else { return };
        let abi = build_abi(&tk, &cells).unwrap();
        let defined: BTreeSet<&str> = abi.types.iter().map(|t| t.name.as_str()).collect();

        // A TS type that is a bare capitalised identifier (possibly `[]`-ed or
        // parenthesised) has to resolve to a `types` entry, or the generated
        // package references a type it never declares.
        for c in abi.emitted() {
            for p in &c.props {
                for m in union_members(&p.ts) {
                    let m = m.trim_end_matches("[]").trim_matches(|c| c == '(' || c == ')');
                    if m.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && m != "ReactNode"
                    {
                        assert!(
                            defined.contains(m),
                            "{}.{} is typed `{}` but `{m}` is not in the types section",
                            c.name,
                            p.name,
                            p.ts
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_prop_with_both_a_union_type_and_an_expr_default_keeps_both() {
        // The regression this exists for: `PropDefault::Expr` has a field named
        // `ts` (the default as a literal) and so does `AbiMember` (the prop's
        // type). Flattened side by side they collided, and `Button.variant`
        // serialised as `"ts": null` — the union silently gone from the one
        // document whose job is to record it. Every fixture above uses a
        // default with no `ts` key, so none of them could see it; this asserts
        // on the real `src/atoms/button.rs:22-23`, which has both.
        let Some((tk, cells)) = real() else { return };
        let abi = build_abi(&tk, &cells).unwrap();
        let variant = abi
            .component("Button")
            .unwrap()
            .props
            .iter()
            .find(|p| p.name == "variant")
            .unwrap();
        assert_eq!(variant.ts, "\"primary\" | \"ghost\"");
        assert!(matches!(&variant.default, AbiDefault::Expr { expr, .. }
            if expr == "ButtonVariant::Primary"));

        // And it has to survive the JSON, which is where the collision happened.
        let json = to_json(&abi).unwrap();
        let back = parse_abi(&json, "test").unwrap();
        assert_eq!(back, abi, "the ABI must round-trip through its own file");
        let raw: serde_json::Value = serde_json::from_str(&json).unwrap();
        let props = raw["components"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == "Button")
            .unwrap()["props"]
            .as_array()
            .unwrap();
        let v = props.iter().find(|p| p["name"] == "variant").unwrap();
        assert_eq!(v["ts"], "\"primary\" | \"ghost\"");
        assert_eq!(v["default"], "expr");
    }

    #[test]
    fn every_real_prop_serialises_a_non_null_ts_type() {
        // The general form of the bug above: no flattened field may ever blank
        // out another. Asserted over all 146 props rather than the one that
        // happened to break.
        let Some((tk, cells)) = real() else { return };
        let json = to_json(&build_abi(&tk, &cells).unwrap()).unwrap();
        let raw: serde_json::Value = serde_json::from_str(&json).unwrap();
        let mut checked = 0;
        let mut members = |v: &serde_json::Value, owner: &str| {
            for p in v.as_array().into_iter().flatten() {
                assert!(
                    p["ts"].as_str().is_some_and(|s| !s.is_empty()),
                    "{owner}.{} serialised ts as {}",
                    p["name"],
                    p["ts"]
                );
                checked += 1;
            }
        };
        for c in raw["components"].as_array().unwrap() {
            members(&c["props"], c["name"].as_str().unwrap());
        }
        for t in raw["types"].as_array().unwrap() {
            members(&t["fields"], t["name"].as_str().unwrap());
        }
        assert!(checked > 100, "expected the whole crate's props, saw {checked}");
    }

    #[test]
    fn the_real_crate_is_unchanged_against_itself() {
        let Some((tk, cells)) = real() else { return };
        let abi = build_abi(&tk, &cells).unwrap();
        assert!(diff_abi(&abi, &abi).is_empty());
    }

    #[test]
    fn dropping_one_real_component_is_breaking() {
        let Some((tk, cells)) = real() else { return };
        let old = build_abi(&tk, &cells).unwrap();
        let mut new = old.clone();
        new.components.retain(|c| c.name != "Button");
        let changes = diff_abi(&old, &new);
        assert_eq!(required_bump(&changes), "major");
        assert!(changes
            .iter()
            .any(|c| c.subject == "Button" && matches!(c.change, Change::SurfaceRemoved { .. })));
    }
}
