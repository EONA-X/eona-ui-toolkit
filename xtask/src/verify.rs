//! Stage: `verify`. See xtask/README.md for the pipeline contract.
//!
//! The safety gate. Everything downstream of here assumes one thing: that the
//! markup a component produced is the *same function of its props* for every
//! value those props could take, so that replacing a sentinel's byte range with
//! a JSX expression yields a component that behaves like the Yew one for all
//! inputs, not just for the one the generator happened to render. `splice` has
//! no way to check that — it sees one string of HTML and a set of byte offsets.
//! This stage is where the assumption is tested.
//!
//! Three checks per (component, cell):
//!
//! 1. **Sentinel integrity.** Every planted sentinel comes out exactly once and
//!    intact, and nothing sentinel-shaped comes out that was not planted. A
//!    sentinel that is missing, doubled, or broken into pieces means the
//!    component read the prop's *value* and wrote something else, so the byte
//!    range `splice` would overwrite is not the prop.
//!    `src/molecules/dataset_card.rs:43` is the crate's real instance: it takes
//!    `title.chars().next()` and renders that derived character, which surfaces
//!    here as a lone `U+E000` with no closing delimiter.
//!
//! 2. **Differential alphabet.** The same cell is rendered twice with sentinel
//!    alphabets chosen to disagree on every property a component could
//!    accidentally depend on — leading character, character count, byte length,
//!    UTF-8 boundary positions (see [`Alphabet`]). Normalise the sentinels away
//!    and the two markups must be byte-identical. Check 1 alone cannot see a
//!    component that emits `data-len={s.len()}` or picks a CSS class from the
//!    prop's first byte: the sentinel still arrives whole, and the derived value
//!    sits somewhere else in the markup. This check is the reason the gate is
//!    sound rather than merely plausible.
//!
//! 3. **Panic capture.** `src/molecules/swatch.rs:11` slices `&hex[0..2]`, which
//!    is not a character boundary in any sentinel (every private-use code point
//!    is 3 or 4 bytes in UTF-8), so `Swatch` aborts mid-render. That is a fact
//!    about the component, not a failure of the run: a
//!    [`RenderResult::Panicked`] quarantines the component and the pipeline
//!    continues.
//!
//! A component passes only if every one of its cells passes all three. Failures
//! are [`Status::Quarantined`], never dropped and never fatal — a quarantined
//! component keeps its place in the ABI with the evidence attached, so the
//! React library is missing it loudly rather than quietly.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::Path as FsPath;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::ir::*;
use crate::matrix;
// `render` owns this type: a panic is data there and a quarantine here, and two
// spellings of the same fact is how the two stages would drift apart.
use crate::render::RenderResult;

/// The two components this gate is *expected* to catch on the crate at the
/// commit under test, with the line that causes it. They are listed so that the
/// gate can report its own failure: a run where `Swatch` passes is a run where
/// the panic capture or the harness stopped working, and a silent pass would
/// look exactly like success.
pub const EXPECTED_QUARANTINES: &[(&str, &str)] = &[
    // `contrast_color` slices the hex string by byte index.
    ("Swatch", "src/molecules/swatch.rs:11"),
    // `initial` is `title.chars().next()`, a transform of the prop.
    ("DatasetCard", "src/molecules/dataset_card.rs:42"),
    // Renders a `<Swatch>` per `ColorSpec` (src/molecules/palette_group.rs:52),
    // so it inherits Swatch's panic. Quarantining a component is not transitive
    // on its own — this is the crate's only composer of a quarantined leaf, and
    // it is listed because the gate found it, not because anyone predicted it.
    ("PaletteGroup", "src/molecules/palette_group.rs:52"),
    // `short_datatype` (src/molecules/annotation.rs:24) slices after the last
    // `#` or `/`. Invisible until the differential alphabet's body contained
    // both — before that the transform was the identity on every probe, both
    // alphabets agreed, and the component shipped rendering a whole IRI where
    // Yew renders its local name. Listed so that a future run where it *passes*
    // is reported as a gate regression rather than as good news.
    ("OntoAnnotation", "src/molecules/annotation.rs:24"),
];

// ---------------------------------------------------------------- alphabets

/// A sentinel alphabet: the delimiters `harness` wraps a prop's placeholder id
/// in, so `render` produces markup in which each prop's contribution can be
/// located byte-exactly.
///
/// The two alphabets are deliberately different in every dimension a component
/// could branch on, because check 2 is only as strong as their disagreement:
///
/// | | `Primary` | `Differential` |
/// |---|---|---|
/// | open / close | `U+E000` / `U+E001` (BMP private use) | `U+F0000` / `U+F0001` (plane 15 private use) |
/// | bytes per delimiter | 3 | 4 |
/// | id | decimal, unpadded | decimal, zero-padded to 4 |
/// | body after the id | empty | [`PAD`] — 70 mixed-case ASCII chars including `#` and `/` |
/// | `sentinel(7)` | 3 chars, 7 bytes | 76 chars, 82 bytes |
///
/// # Why the differential body is a long hostile run
///
/// The delimiters alone only disagree on leading character, character count,
/// byte length and UTF-8 boundaries. That leaves the gate blind to any transform
/// that is the *identity* on both payloads, and the payloads used to be
/// digits-only — which has no case, no `#`, no `/`, and is 1–2 characters long.
/// Measured blind spots, all of which now fail:
///
/// - `to_uppercase()` / `to_lowercase()`: digits have no case, so both renders
///   were unchanged. [`PAD`] is mixed-case, so folding it breaks the sentinel
///   and check 1 reports a fragment.
/// - Truncation at any length above 6: both payloads were shorter than every
///   plausible threshold. [`PAD`] makes the differential sentinel 82 bytes.
/// - A class or attribute that flips on `len()`: detection used to require the
///   threshold to fall strictly between 7 and 12 bytes. The window is now 7..82.
/// - The `src/molecules/annotation.rs:24` shape — `rfind('#')` / `rfind('/')`
///   then slice. Neither character occurred in a payload, so `short_datatype`
///   was the identity on every probe and `OntoAnnotation` passed clean while
///   rendering a whole IRI where Yew renders its local name. [`PAD`] contains
///   both, so the transform now decapitates the differential sentinel.
///
/// What is still invisible is a branch gated on a *literal* value
/// (`if label == "danger"`), which no probe alphabet can reach — the gate cannot
/// guess a constant it was never told about. That is stated in xtask/README.md
/// rather than papered over.
///
/// Both alphabets survive Yew's escaping untouched: `html_escape`'s
/// `encode_text` and `encode_double_quoted_attribute` rewrite only `&`, `<`,
/// `>` and `"`, and [`PAD`] contains none of those.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Alphabet {
    Primary,
    Differential,
}

/// The differential alphabet's body, appended after the id.
///
/// Constraints, each of which a candidate character has to clear:
///
/// - none of `&`, `<`, `>`, `"` — those are the only characters Yew's
///   `html_escape` rewrites, and a rewritten sentinel would be a false positive;
/// - no `'`, no whitespace, no `;` and no `:` — a sentinel lands in `class` and
///   `style` attributes, and Yew's `Classes` collapses whitespace while
///   `splice`'s style parser splits on `;` and `:`, so any of those would make
///   the two renders differ for a reason that is not a transform;
/// - does not start with a digit, so [`Alphabet::scan`]'s `take_while` over the
///   id stops exactly at the id's end;
/// - contains upper case, lower case, digits, `#` and `/`, which is what buys
///   the four detections listed on [`Alphabet`];
/// - long, because the length-threshold detection window is `Primary`'s 7 bytes
///   up to this run's length.
const PAD: &str = "aZ0kQ9#mX/bY1nR8#pW/cV2jT7#hU/dS3gN6#fM/eL4iK5#oJ/fT6hP8#qW/gU7iO9#rX/";

impl Alphabet {
    pub const ALL: [Alphabet; 2] = [Alphabet::Primary, Alphabet::Differential];

    /// The text between the opening delimiter and the id-terminating close.
    /// `Primary` stays digits-only: `splice` reads the primary renders and only
    /// the primary renders (xtask/src/splice.rs:792), so widening that alphabet
    /// would move every byte offset the emitter works from for no gain.
    fn pad(self) -> &'static str {
        match self {
            Alphabet::Primary => "",
            Alphabet::Differential => PAD,
        }
    }

    pub fn open(self) -> char {
        match self {
            Alphabet::Primary => '\u{E000}',
            Alphabet::Differential => '\u{F0000}',
        }
    }

    pub fn close(self) -> char {
        match self {
            Alphabet::Primary => '\u{E001}',
            Alphabet::Differential => '\u{F0001}',
        }
    }

    /// The placeholder text for slot `id`. `harness` MUST build its prop values
    /// with this rather than formatting the delimiters itself, so that the
    /// planted text and the text this stage searches for can never drift.
    pub fn sentinel(self, id: u32) -> String {
        match self {
            Alphabet::Primary => format!("{}{id}{}", self.open(), self.close()),
            Alphabet::Differential => {
                format!("{}{id:04}{}{}", self.open(), PAD, self.close())
            }
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Alphabet::Primary => "primary",
            Alphabet::Differential => "differential",
        }
    }

    /// Every well-formed sentinel in `html`, plus every delimiter character that
    /// is not part of one. The strays are what catch a component that took a
    /// prop apart: `DatasetCard`'s uppercased first character is an opening
    /// delimiter with no id and no close.
    fn scan(self, html: &str) -> Scan {
        let (open, close) = (self.open(), self.close());
        let mut found = Vec::new();
        let mut strays = Vec::new();
        let mut i = 0usize;
        while i < html.len() {
            let ch = html[i..].chars().next().expect("byte index is on a char boundary");
            let w = ch.len_utf8();
            if ch == open {
                let digits_at = i + w;
                let digits: String =
                    html[digits_at..].chars().take_while(char::is_ascii_digit).collect();
                let after = digits_at + digits.len();
                // The pad has to match byte for byte. That is the point: a
                // transform that lowercases it, truncates it, or slices at its
                // last `#` leaves an opening delimiter this arm rejects, so the
                // `_` arm below records a stray and check 1 quarantines.
                let pad = self.pad();
                let body_ok = html[after..].starts_with(pad);
                let after = after + if body_ok { pad.len() } else { 0 };
                match (digits.parse::<u32>(), body_ok && html[after..].starts_with(close)) {
                    (Ok(id), true) => {
                        let end = after + close.len_utf8();
                        found.push(Found { at: i, end, id });
                        i = end;
                        continue;
                    }
                    // An opening delimiter that is not followed by `<digits><close>`
                    // is a fragment of a sentinel the component pulled apart.
                    _ => strays.push(Stray { at: i, ch }),
                }
            } else if ch == close {
                strays.push(Stray { at: i, ch });
            }
            i += w;
        }
        Scan { found, strays }
    }
}

impl fmt::Display for Alphabet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug)]
struct Found {
    at: usize,
    end: usize,
    id: u32,
}

#[derive(Clone, Copy, Debug)]
struct Stray {
    at: usize,
    ch: char,
}

#[derive(Debug, Default)]
struct Scan {
    found: Vec<Found>,
    strays: Vec<Stray>,
}

// ---------------------------------------------------------------- input

/// One prop value `harness` replaced with a placeholder, and where it came
/// from. `span` is carried so a quarantine names a line rather than a prop name
/// the reader then has to go and find.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sentinel {
    pub id: u32,
    /// The axis path the value was written to, as `matrix::Path` displays it —
    /// `title`, `values[].language`.
    pub path: String,
    pub span: Span,
}

/// One rendered cell in one alphabet — the unit `verify` consumes. `render`
/// produces exactly two of these per cell, one per [`Alphabet`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellRender {
    pub component: String,
    /// `matrix::Cell::id`.
    pub cell: String,
    pub alphabet: Alphabet,
    /// Every sentinel planted for this cell, in any order.
    pub plan: Vec<Sentinel>,
    pub result: RenderResult,
}

/// One harness wrapper as `harness` recorded it. `render` returns markup keyed
/// by wrapper id and knows nothing about props, so the two halves are rejoined
/// here — and the rejoin is itself checked, because an id present in one map and
/// absent from the other is a cell that silently did not happen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderPlan {
    pub component: String,
    /// `matrix::Cell::id`.
    pub cell: String,
    pub alphabet: Alphabet,
    pub sentinels: Vec<Sentinel>,
}

/// Join `render`'s results to `harness`'s plans. Both maps are keyed by wrapper
/// id (`render::component_of` documents the shape).
pub fn collect(
    results: &BTreeMap<String, RenderResult>,
    plans: &BTreeMap<String, RenderPlan>,
) -> Result<Vec<CellRender>> {
    let mut out = Vec::with_capacity(results.len());
    for (id, result) in results {
        let Some(plan) = plans.get(id) else {
            bail!(
                "render produced markup for wrapper {id:?}, which the harness never \
                 planned; verify cannot say which props went into it"
            );
        };
        out.push(CellRender {
            component: plan.component.clone(),
            cell: plan.cell.clone(),
            alphabet: plan.alphabet,
            plan: plan.sentinels.clone(),
            result: result.clone(),
        });
    }
    for id in plans.keys() {
        if !results.contains_key(id) {
            bail!(
                "the harness planned wrapper {id:?} but render returned no result for it; \
                 a cell that is neither markup nor a panic has not been checked by anything"
            );
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- output

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Check {
    /// A sentinel did not come back whole and exactly once.
    SentinelIntegrity,
    /// The two alphabets produced different markup.
    DifferentialAlphabet,
    /// A list's sentinels came back in an order other than the one they were
    /// planted in.
    SentinelOrder,
    /// The render panicked.
    Panic,
}

impl Check {
    fn name(self) -> &'static str {
        match self {
            Check::SentinelIntegrity => "sentinel-integrity",
            Check::DifferentialAlphabet => "differential-alphabet",
            Check::SentinelOrder => "sentinel-order",
            Check::Panic => "panic",
        }
    }

    /// The component-level reason a failure of this check produces. Ordered by
    /// [`Check::rank`]: a component that both panics and fails integrity is
    /// reported as panicking, since that is the cause and the rest is fallout.
    fn reason(self) -> &'static str {
        match self {
            Check::Panic => "panics while rendering a sentinel",
            Check::SentinelIntegrity => "does not pass its props through to the markup unmodified",
            Check::DifferentialAlphabet => "markup depends on prop content, not only on prop presence",
            Check::SentinelOrder => "renders a list in an order other than the list's own",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Check::Panic => 0,
            Check::SentinelIntegrity => 1,
            Check::SentinelOrder => 2,
            Check::DifferentialAlphabet => 3,
        }
    }
}

/// One concrete reason a cell failed, naming a span a human can open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evidence {
    pub check: Check,
    /// `matrix::Cell::id`.
    pub cell: String,
    /// `None` when the finding is about the pair of renders rather than one of
    /// them.
    pub alphabet: Option<Alphabet>,
    pub detail: String,
    pub span: Span,
}

impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] cell {}", self.check.name(), self.cell)?;
        if let Some(a) = self.alphabet {
            write!(f, " ({a})")?;
        }
        write!(f, ": {} — {}:{}", self.detail, self.span.file, self.span.line)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "kebab-case")]
pub enum Verdict {
    Pass,
    Quarantine { reason: String, evidence: Vec<Evidence> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentVerdict {
    pub component: String,
    /// The `#[function_component]` line, so the verdict is openable even before
    /// reading the evidence.
    pub span: Span,
    /// Cells checked, which is `matrix::cells(c).len()`.
    pub cells: usize,
    /// Findings that do not quarantine but that `splice` and `emit` must act
    /// on: a prop rendered in more than one place, or not rendered at all in
    /// some cell. Carried on a passing verdict too — a note dropped here is a
    /// splice that writes one occurrence where the Yew component wrote four
    /// (`src/molecules/nav_dropdown.rs:27,29,35,43`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Evidence>,
    #[serde(flatten)]
    pub verdict: Verdict,
}

impl ComponentVerdict {
    pub fn is_quarantined(&self) -> bool {
        matches!(self.verdict, Verdict::Quarantine { .. })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyReport {
    /// Copied from the IR, so `quarantine.json` can be shown to be about this
    /// source and not a stale tree.
    pub src_hash: String,
    /// In IR order, every component that was rendered.
    pub components: Vec<ComponentVerdict>,
    /// Quarantined components that [`EXPECTED_QUARANTINES`] does not list. Each
    /// one is a component that transforms its props in a way nobody had noticed,
    /// so it is surfaced separately rather than being one row among many.
    pub unexpected: Vec<String>,
    /// [`EXPECTED_QUARANTINES`] entries that passed. The gate failing to catch
    /// what it was built to catch is indistinguishable from a clean run unless
    /// it is stated.
    pub missing_expected: Vec<String>,
}

impl VerifyReport {
    pub fn quarantined(&self) -> impl Iterator<Item = &ComponentVerdict> {
        self.components.iter().filter(|c| c.is_quarantined())
    }

    pub fn passed(&self) -> impl Iterator<Item = &ComponentVerdict> {
        self.components.iter().filter(|c| !c.is_quarantined())
    }

    /// Components the emitters may generate: everything rendered that was not
    /// quarantined.
    pub fn emitted_count(&self) -> usize {
        self.passed().count()
    }

    /// Whether the gate behaved the way this crate's known hard cases say it
    /// must. Kept separate from [`verify`] so that an unexpected quarantine
    /// stops a release without stopping a development run.
    pub fn self_check(&self) -> Result<()> {
        if !self.missing_expected.is_empty() {
            bail!(
                "the verify gate passed {} which it is supposed to quarantine ({}); \
                 the gate itself has regressed, not the toolkit",
                self.missing_expected.join(", "),
                EXPECTED_QUARANTINES
                    .iter()
                    .map(|(n, at)| format!("{n} at {at}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        if !self.unexpected.is_empty() {
            bail!(
                "unexpected quarantine: {} — a component transforms its props in a way \
                 nobody had recorded; read xtask/out/quarantine.json before widening \
                 EXPECTED_QUARANTINES (xtask/src/verify.rs)",
                self.unexpected.join(", ")
            );
        }
        Ok(())
    }

    /// `xtask/out/quarantine.json`: the quarantine list, the evidence, and both
    /// directions of surprise.
    pub fn write(&self, path: &FsPath) -> Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("creating {}", dir.display()))?;
        }
        let file = QuarantineFile {
            src_hash: &self.src_hash,
            checked: self.components.len(),
            emitted: self.emitted_count(),
            passed: self.passed().map(|c| c.component.as_str()).collect(),
            quarantined: self.quarantined().collect(),
            noted: self.passed().filter(|c| !c.notes.is_empty()).collect(),
            unexpected: &self.unexpected,
            missing_expected: &self.missing_expected,
        };
        let json = serde_json::to_string_pretty(&file).context("serialising the quarantine list")?;
        std::fs::write(path, format!("{json}\n"))
            .with_context(|| format!("writing {}", path.display()))
    }

    /// Everything a human should see on stderr even when the run is allowed to
    /// continue.
    pub fn emit(&self) {
        for c in self.quarantined() {
            let Verdict::Quarantine { reason, evidence } = &c.verdict else { continue };
            let expected = EXPECTED_QUARANTINES.iter().any(|(n, _)| *n == c.component);
            eprintln!(
                "verify: quarantined {} ({}:{}) — {reason}{}",
                c.component,
                c.span.file,
                c.span.line,
                if expected { "" } else { "  [UNEXPECTED]" }
            );
            for e in evidence {
                eprintln!("  {e}");
            }
        }
        for c in self.passed().filter(|c| !c.notes.is_empty()) {
            eprintln!(
                "verify: {} ({}:{}) passed with {} finding(s) `splice` must honour",
                c.component,
                c.span.file,
                c.span.line,
                c.notes.len(),
            );
            // One line per distinct finding, not per cell: `NavDropdown` repeats
            // the same `id` note across 4 cells x 2 alphabets and the reader
            // needs it once.
            let mut seen = BTreeSet::new();
            for e in &c.notes {
                if seen.insert((e.detail.clone(), e.span.line)) {
                    eprintln!("  {e}");
                }
            }
        }
        for name in &self.missing_expected {
            eprintln!(
                "verify: {name} was expected to quarantine and did not; the gate is not \
                 doing its job"
            );
        }
    }
}

#[derive(Serialize)]
struct QuarantineFile<'a> {
    src_hash: &'a str,
    checked: usize,
    emitted: usize,
    passed: Vec<&'a str>,
    quarantined: Vec<&'a ComponentVerdict>,
    /// Passing components that nevertheless carry findings `splice` has to act
    /// on. Written next to the quarantines because the two are read together:
    /// these are the components where the naive one-sentinel-one-slot splice is
    /// wrong even though the component itself is fine.
    noted: Vec<&'a ComponentVerdict>,
    unexpected: &'a [String],
    missing_expected: &'a [String],
}

// ---------------------------------------------------------------- the gate

/// Run all three checks over every rendered cell.
///
/// Errors — as opposed to quarantines — are reserved for the generator being
/// wrong about itself: a cell rendered that `matrix` never asked for, a cell
/// `matrix` asked for that was never rendered, a cell rendered in only one
/// alphabet, two renders of the same cell planting different sentinels. None of
/// those is a statement about a component, and letting any of them through would
/// weaken a check rather than fail one.
pub fn verify(tk: &Toolkit, renders: &[CellRender]) -> Result<VerifyReport> {
    let by_component = group(renders)?;

    // A component the renderer skipped would otherwise vanish between `matrix`
    // and `emit` with every check reporting green, which is the failure mode
    // this whole stage exists to make impossible.
    for c in tk.components.iter().filter(|c| matches!(c.status, Status::Ok)) {
        if !by_component.contains_key(c.name.as_str()) {
            bail!(
                "{} ({}:{}) is Status::Ok but was never rendered; verify cannot clear a \
                 component it has not seen",
                c.name,
                c.span.file,
                c.span.line
            );
        }
    }

    let mut report = VerifyReport { src_hash: tk.src_hash.clone(), ..VerifyReport::default() };
    // IR order, so the report reads in the same order as `dump-ir`.
    for component in &tk.components {
        let Some(cells) = by_component.get(component.name.as_str()) else { continue };
        report.components.push(verify_component(component, tk, cells)?);
    }

    // A rendered component that is not in the IR means the render stage and the
    // IR disagree about what the crate contains.
    for name in by_component.keys() {
        if tk.component(name).is_none() {
            bail!("rendered component {name:?} is not in the IR; the two stages disagree \
                   about what the crate contains");
        }
    }

    let quarantined: BTreeSet<String> =
        report.quarantined().map(|c| c.component.clone()).collect();
    let expected: BTreeSet<&str> = EXPECTED_QUARANTINES.iter().map(|(n, _)| *n).collect();
    report.unexpected = quarantined
        .iter()
        .filter(|n| !expected.contains(n.as_str()))
        .cloned()
        .collect();
    report.missing_expected = expected
        .iter()
        // Only count an expected quarantine as missing if it was actually
        // rendered: a run over a subset of components has not had the chance.
        .filter(|n| by_component.contains_key(**n) && !quarantined.contains(**n))
        .map(|n| n.to_string())
        .collect();
    Ok(report)
}

/// Fold the quarantines back into the IR. Kept out of [`verify`] so the report
/// can be inspected (and written) before the IR is changed, and so `verify`
/// stays a pure function of what it was handed.
pub fn apply(tk: &mut Toolkit, report: &VerifyReport) -> Result<()> {
    for v in report.quarantined() {
        let Verdict::Quarantine { reason, .. } = &v.verdict else { continue };
        let Some(c) = tk.components.iter_mut().find(|c| c.name == v.component) else {
            bail!("quarantined component {:?} is not in the IR", v.component);
        };
        match &c.status {
            // Quarantining something already excluded would overwrite the rule
            // that excluded it, which is the one thing the ABI must not lose.
            Status::Excluded { reason } => bail!(
                "{} is Excluded ({reason}) but was rendered and quarantined; it should \
                 never have reached the renderer",
                c.name
            ),
            _ => c.status = Status::Quarantined { reason: reason.clone() },
        }
    }
    Ok(())
}

type CellMap<'a> = BTreeMap<&'a str, BTreeMap<Alphabet, &'a CellRender>>;

fn group(renders: &[CellRender]) -> Result<BTreeMap<&str, CellMap<'_>>> {
    let mut out: BTreeMap<&str, CellMap<'_>> = BTreeMap::new();
    for r in renders {
        let slot = out
            .entry(r.component.as_str())
            .or_default()
            .entry(r.cell.as_str())
            .or_default()
            .insert(r.alphabet, r);
        if slot.is_some() {
            bail!(
                "{}: cell {:?} was rendered twice in the {} alphabet; the second render \
                 would silently replace the first",
                r.component,
                r.cell,
                r.alphabet
            );
        }
    }
    Ok(out)
}

fn verify_component(
    component: &Component,
    tk: &Toolkit,
    rendered: &CellMap<'_>,
) -> Result<ComponentVerdict> {
    // Recomputed rather than trusted: this is the seam between `matrix` and
    // `render`, and a cell quietly dropped in between is coverage lost with
    // every downstream check still green.
    let expected = matrix::cells(component, tk)
        .with_context(|| format!("re-deriving render cells for {}", component.name))?;
    let want: BTreeSet<&str> = expected.iter().map(|c| c.id.as_str()).collect();
    let have: BTreeSet<&str> = rendered.keys().copied().collect();
    if want != have {
        let missing: Vec<&str> = want.difference(&have).copied().collect();
        let extra: Vec<&str> = have.difference(&want).copied().collect();
        bail!(
            "{} ({}:{}): rendered cells do not match the matrix — {} missing [{}], {} \
             unexpected [{}]",
            component.name,
            component.span.file,
            component.span.line,
            missing.len(),
            missing.join(", "),
            extra.len(),
            extra.join(", ")
        );
    }

    let mut evidence = Vec::new();
    let mut notes = Vec::new();
    for cell in &expected {
        let renders = &rendered[cell.id.as_str()];
        let outcome = verify_cell(component, &cell.id, renders)?;
        evidence.extend(outcome.failures);
        notes.extend(outcome.notes);
    }

    let verdict = match evidence
        .iter()
        .min_by_key(|e| (e.check.rank(), e.cell.clone()))
        .map(|e| e.check)
    {
        None => Verdict::Pass,
        Some(worst) => {
            Verdict::Quarantine { reason: worst.reason().to_string(), evidence }
        }
    };
    Ok(ComponentVerdict {
        component: component.name.clone(),
        span: component.span.clone(),
        cells: expected.len(),
        notes,
        verdict,
    })
}

/// A cell's outcome: what quarantines it, and what is worth telling `splice`
/// about but does not.
#[derive(Debug, Default)]
struct CellOutcome {
    failures: Vec<Evidence>,
    notes: Vec<Evidence>,
}

fn verify_cell(
    component: &Component,
    cell: &str,
    renders: &BTreeMap<Alphabet, &CellRender>,
) -> Result<CellOutcome> {
    for alphabet in Alphabet::ALL {
        if !renders.contains_key(&alphabet) {
            bail!(
                "{} ({}:{}): cell {cell:?} has no {alphabet} render; check 2 needs both \
                 alphabets and a one-sided pass would be a weaker guarantee wearing the \
                 same name",
                component.name,
                component.span.file,
                component.span.line
            );
        }
    }
    let primary = renders[&Alphabet::Primary];
    let differential = renders[&Alphabet::Differential];
    plans_agree(component, cell, primary, differential)?;

    let mut out = CellOutcome::default();
    // Check 3 first: a panicked render has no markup for checks 1 and 2 to read,
    // and the panic is the finding anyway.
    let mut panicked = false;
    for r in [primary, differential] {
        if let RenderResult::Panicked(message) = &r.result {
            panicked = true;
            out.failures.push(Evidence {
                check: Check::Panic,
                cell: cell.to_string(),
                alphabet: Some(r.alphabet),
                detail: format!(
                    "rendering panicked: {}; the component reads the prop's bytes or \
                     characters, and no sentinel is a valid input to that",
                    first_line(message)
                ),
                span: component.span.clone(),
            });
        }
    }
    if panicked {
        return Ok(out);
    }

    let (RenderResult::Html(a), RenderResult::Html(b)) = (&primary.result, &differential.result)
    else {
        unreachable!("both renders are Html: the Panicked arm returned above");
    };

    // Check 1, per render.
    let mut found = integrity(component, cell, primary, a);
    let second = integrity(component, cell, differential, b);
    found.corruption.extend(second.corruption);
    found.arity.extend(second.arity);

    // Check 2. Run even when check 1 already failed: the two findings point at
    // different lines (the sentinel that vanished vs. the derived bytes that
    // took its place) and a reader wants both.
    let divergence = differ(component, cell, primary, a, differential, b);

    // Check 2 is what makes an arity finding readable. The two alphabets share
    // no leading character, no character count, no byte length and no UTF-8
    // boundary, so anything a component derives from a prop's *content* differs
    // between them — `DatasetCard`'s uppercased initial
    // (src/molecules/dataset_card.rs:43) is one character in the primary render
    // and a different one in the differential. If the normalised markups are
    // nevertheless identical, then every byte that is not a sentinel is
    // independent of what the props contained, and a sentinel that arrived
    // twice arrived twice verbatim while one that did not arrive was not
    // rendered at all. Neither of those blocks a faithful splice, so they are
    // recorded for `splice` rather than held against the component.
    //
    // The converse is not assumed: corrupted markup quarantines on its own, and
    // it drags the arity findings along with it, because once a prop has
    // demonstrably been taken apart the arity of the pieces is evidence too.
    out.failures.extend(found.corruption.iter().cloned());
    if let Some(e) = divergence {
        out.failures.push(e);
    }
    if out.failures.is_empty() {
        out.notes.extend(found.arity);
    } else {
        out.failures.extend(found.arity);
    }
    Ok(out)
}

/// The two alphabets must have filled the same slots at the same paths, or the
/// comparison in check 2 is between two different things.
fn plans_agree(
    component: &Component,
    cell: &str,
    a: &CellRender,
    b: &CellRender,
) -> Result<()> {
    let key = |r: &CellRender| -> Vec<(u32, String)> {
        let mut v: Vec<(u32, String)> =
            r.plan.iter().map(|s| (s.id, s.path.clone())).collect();
        v.sort();
        v
    };
    let (ka, kb) = (key(a), key(b));
    if ka != kb {
        bail!(
            "{} ({}:{}): cell {cell:?} planted different sentinels in the two alphabets \
             ({:?} vs {:?}); check 2 would be comparing two different renders",
            component.name,
            component.span.file,
            component.span.line,
            ka,
            kb
        );
    }
    let mut ids: Vec<u32> = a.plan.iter().map(|s| s.id).collect();
    ids.sort_unstable();
    let unique = ids.len();
    ids.dedup();
    if ids.len() != unique {
        bail!(
            "{} ({}:{}): cell {cell:?} planted the same sentinel id in two slots, so a \
             missing one would be indistinguishable from a duplicated one",
            component.name,
            component.span.file,
            component.span.line
        );
    }
    Ok(())
}

/// Check 1 for one render, split by what the finding actually proves.
///
/// `.corruption` is evidence that the markup holds something the harness never
/// planted: a delimiter with no id around it, a sentinel id nobody wrote, the
/// other alphabet leaking in. Each of those is a component taking a prop apart,
/// and each quarantines on its own.
///
/// `.arity` is evidence that a planted sentinel came back a number of times
/// other than once. That is *not* proof of a transform, and on this crate it
/// usually is not one: `src/molecules/text_field.rs:25,29` renders `id` into
/// both `<label for>` and `<input id>`, and `src/molecules/annotation.rs:35`
/// reads `language` in preference to `datatype`, so in a cell where both are
/// present the `datatype` sentinel is correctly never rendered. Whether such a
/// finding is benign is decided one level up, by check 2 — see [`Findings`].
#[derive(Debug, Default)]
struct Findings {
    corruption: Vec<Evidence>,
    arity: Vec<Evidence>,
}

fn integrity(
    component: &Component,
    cell: &str,
    render: &CellRender,
    html: &str,
) -> Findings {
    let alphabet = render.alphabet;
    let scan = alphabet.scan(html);
    let mut out = Findings::default();
    let say = |detail: String, span: Span| Evidence {
        check: Check::SentinelIntegrity,
        cell: cell.to_string(),
        alphabet: Some(alphabet),
        detail,
        span,
    };

    for s in &render.plan {
        let hits: Vec<&Found> = scan.found.iter().filter(|f| f.id == s.id).collect();
        match hits.len() {
            1 => {}
            0 => out.arity.push(say(
                format!(
                    "the sentinel for `{}` ({}) never reached the markup: in this cell \
                     the component does not render the prop at all, or renders something \
                     derived from it. Check 2 tells the two apart — if the alphabets \
                     agree, nothing derived is there and the emitter must simply not \
                     reference `{}` in this cell",
                    s.path,
                    show(&alphabet.sentinel(s.id)),
                    s.path,
                ),
                s.span.clone(),
            )),
            n => out.arity.push(say(
                format!(
                    "the sentinel for `{}` ({}) appears {n} times, each one whole, so the \
                     component renders the prop verbatim in {n} places and `splice` must \
                     write the JSX expression into all {n}",
                    s.path,
                    show(&alphabet.sentinel(s.id))
                ),
                s.span.clone(),
            )),
        }
    }

    out.corruption.extend(order(cell, render, &scan));

    let planted: BTreeSet<u32> = render.plan.iter().map(|s| s.id).collect();
    for f in scan.found.iter().filter(|f| !planted.contains(&f.id)) {
        out.corruption.push(say(
            format!(
                "sentinel id {} was never planted but appears at byte {}: {}",
                f.id,
                f.at,
                excerpt(html, f.at)
            ),
            component.span.clone(),
        ));
    }

    // The `DatasetCard` signature: a delimiter with nothing around it, which is
    // a sentinel the component took apart.
    for stray in &scan.strays {
        out.corruption.push(say(
            format!(
                "a bare {} delimiter at byte {} is a fragment of a sentinel, not a \
                 sentinel: {}",
                show(&stray.ch.to_string()),
                stray.at,
                excerpt(html, stray.at)
            ),
            component.span.clone(),
        ));
    }

    // Cross-contamination: a render that contains the *other* alphabet's
    // delimiters is a render stage bug (a cached value, a shared buffer), and
    // would make check 2 compare a render with itself.
    let other = match alphabet {
        Alphabet::Primary => Alphabet::Differential,
        Alphabet::Differential => Alphabet::Primary,
    };
    if let Some(at) = html.find([other.open(), other.close()]) {
        out.corruption.push(say(
            format!(
                "the {other} alphabet leaked into the {alphabet} render at byte {at}: {}",
                excerpt(html, at)
            ),
            component.span.clone(),
        ));
    }
    out
}

/// The list-order half of check 1.
///
/// Check 1 proves every sentinel arrived whole; check 2 proves the two renders
/// agree. Neither sees a component that renders a list in a different order
/// from the one it was given: the two alphabets permute identically, and a
/// content-dependent sort sorts by the id digits, which are monotone in both.
/// `splice` turns a list axis into `items.map(...)`, so a component that
/// reverses or reorders would get JSX that silently renders the list forwards.
///
/// The planting order *is* the list order — `harness` walks a list by index
/// (xtask/src/harness.rs's `list`) — so requiring the byte positions of one
/// list's sentinels to rise with the index is exactly the property `.map()`
/// needs. Only sentinels that appear exactly once are considered; a prop
/// rendered in several places has no single position, and check 1 already
/// carries that as a note.
fn order(cell: &str, render: &CellRender, scan: &Scan) -> Vec<Evidence> {
    // Group key: the path with ONE index position blanked, every other index
    // left concrete. `a[0].b[1]` therefore joins both `a[*].b[1]` and
    // `a[0].b[*]`, so a reversal at any depth lands in some group.
    let mut groups: BTreeMap<String, Vec<(usize, usize, &Sentinel)>> = BTreeMap::new();
    for s in &render.plan {
        let hits: Vec<&Found> = scan.found.iter().filter(|f| f.id == s.id).collect();
        let [hit] = hits[..] else { continue };
        for (key, index) in blanked(&s.path) {
            groups.entry(key).or_default().push((index, hit.at, s));
        }
    }

    let mut out = Vec::new();
    for (key, mut members) in groups {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(|(index, _, _)| *index);
        let Some(w) = members.windows(2).find(|w| w[0].1 > w[1].1) else { continue };
        let (lo, lo_at, lo_s) = w[0];
        let (hi, hi_at, _) = w[1];
        out.push(Evidence {
            check: Check::SentinelOrder,
            cell: cell.to_string(),
            alphabet: Some(render.alphabet),
            detail: format!(
                "`{key}` is rendered out of list order: element {lo} lands at byte {lo_at} but \
                 element {hi} lands at byte {hi_at}, so the component reorders the list rather \
                 than rendering it as given. `splice` would emit a `.map()` over the array in \
                 index order, which is a different list"
            ),
            span: lo_s.span.clone(),
        });
    }
    out
}

/// Every `(path with one index replaced by `[*]`, that index)` pair.
fn blanked(path: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(open) = path[i..].find('[').map(|o| i + o) {
        let Some(close) = path[open..].find(']').map(|c| open + c) else { break };
        let inner = &path[open + 1..close];
        i = close + 1;
        let Ok(index) = inner.parse::<usize>() else { continue };
        out.push((format!("{}[*]{}", &path[..open], &path[close + 1..]), index));
    }
    out
}

/// Check 2: normalise both markups and compare.
fn differ(
    component: &Component,
    cell: &str,
    ra: &CellRender,
    a: &str,
    rb: &CellRender,
    b: &str,
) -> Option<Evidence> {
    let na = normalise(a, ra);
    let nb = normalise(b, rb);
    if na == nb {
        return None;
    }
    let at = na
        .as_bytes()
        .iter()
        .zip(nb.as_bytes())
        .position(|(x, y)| x != y)
        .unwrap_or_else(|| na.len().min(nb.len()));
    // The prop whose sentinel sits nearest the divergence is the likeliest
    // culprit, so the evidence points at its declaration rather than at the
    // component as a whole.
    let span = nearest_prop(&na, at, ra).unwrap_or_else(|| component.span.clone());
    Some(Evidence {
        check: Check::DifferentialAlphabet,
        cell: cell.to_string(),
        alphabet: None,
        detail: format!(
            "the two sentinel alphabets produced different markup once normalised; they \
             differ from byte {at}: {} vs {}. The markup is a function of the prop's \
             content (its length, its first character, its byte boundaries) and not only \
             of the prop, so splicing JSX over the sentinels would not reproduce it",
            excerpt(&na, at),
            excerpt(&nb, at)
        ),
        span,
    })
}

/// Rewrite every planted sentinel to an alphabet-independent token, so two
/// renders of the same cell differ only where the component put something other
/// than a prop. The token is delimited by control characters, which no HTML this
/// crate produces contains, and carries the id so two sentinels cannot be
/// transposed without the comparison noticing.
fn normalise(html: &str, render: &CellRender) -> String {
    let mut out = html.to_string();
    for s in &render.plan {
        out = out.replace(&render.alphabet.sentinel(s.id), &format!("\u{2}{}\u{3}", s.id));
    }
    out
}

/// The declaration of the prop whose normalised sentinel is closest to `at`.
fn nearest_prop(normalised: &str, at: usize, render: &CellRender) -> Option<Span> {
    render
        .plan
        .iter()
        .filter_map(|s| {
            let tok = format!("\u{2}{}\u{3}", s.id);
            let pos = normalised.find(&tok)?;
            Some((at.abs_diff(pos), s.span.clone()))
        })
        .min_by_key(|(d, _)| *d)
        .map(|(_, span)| span)
}

/// A window around `at`, escaped so private-use and control characters are
/// visible in a terminal and in JSON.
fn excerpt(s: &str, at: usize) -> String {
    const WINDOW: usize = 32;
    let start = floor_boundary(s, at.saturating_sub(WINDOW));
    let end = ceil_boundary(s, (at + WINDOW).min(s.len()));
    format!(
        "{}{}{}",
        if start > 0 { "…" } else { "" },
        show(&s[start..end]),
        if end < s.len() { "…" } else { "" }
    )
}

fn floor_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_boundary(s: &str, mut i: usize) -> usize {
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// Escape anything that would not survive being printed: the sentinels are
/// private-use code points and the normalised tokens are control characters, and
/// evidence that renders as an invisible glyph is not evidence.
fn show(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c.is_control() || matches!(c as u32, 0xE000..=0xF8FF | 0xF0000..=0x10FFFD)) => {
                out.push_str(&format!("\\u{{{:X}}}", c as u32))
            }
            c => out.push(c),
        }
    }
    out
}

fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or(s).trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ------------------------------------------------------------ fixtures

    fn text_prop(name: &str, file: &str, line: usize) -> Prop {
        Prop {
            name: name.to_string(),
            rust_ty: "AttrValue".into(),
            kind: PropKind::Text,
            optional: false,
            default: PropDefault::Required,
            span: Span { file: file.into(), line },
        }
    }

    fn opt_prop(name: &str, file: &str, line: usize) -> Prop {
        Prop {
            name: name.to_string(),
            rust_ty: "Option<AttrValue>".into(),
            kind: PropKind::Text,
            optional: true,
            default: PropDefault::DefaultTrait,
            span: Span { file: file.into(), line },
        }
    }

    fn comp(name: &str, file: &str, line: usize, props: Vec<Prop>) -> Component {
        Component {
            name: name.to_string(),
            props_ty: Some(format!("{name}Props")),
            tier: Tier::Presentational,
            status: Status::Ok,
            props,
            span: Span { file: file.into(), line },
        }
    }

    fn toolkit(components: Vec<Component>) -> Toolkit {
        Toolkit { components, src_hash: "test".into(), ..Toolkit::default() }
    }

    fn plan(entries: &[(u32, &str, &str, usize)]) -> Vec<Sentinel> {
        entries
            .iter()
            .map(|(id, path, file, line)| Sentinel {
                id: *id,
                path: path.to_string(),
                span: Span { file: (*file).into(), line: *line },
            })
            .collect()
    }

    /// Both alphabet renders of one cell, produced by a closure that stands in
    /// for the component: it is handed the planted sentinel texts in plan order
    /// and returns whatever the Yew component would have rendered from them.
    /// Writing the fixtures this way rather than as literal HTML is what lets
    /// the `Swatch` and `DatasetCard` tests reproduce the real defect — the
    /// slice and the `chars().next()` are executed here, not paraphrased.
    fn pair(
        component: &str,
        cell: &str,
        plan: Vec<Sentinel>,
        render: impl Fn(&[String]) -> RenderResult,
    ) -> Vec<CellRender> {
        Alphabet::ALL
            .iter()
            .map(|a| {
                let texts: Vec<String> = plan.iter().map(|s| a.sentinel(s.id)).collect();
                CellRender {
                    component: component.to_string(),
                    cell: cell.to_string(),
                    alphabet: *a,
                    plan: plan.clone(),
                    result: render(&texts),
                }
            })
            .collect()
    }

    fn html(s: String) -> RenderResult {
        RenderResult::Html(s)
    }

    fn verdict_of<'a>(r: &'a VerifyReport, name: &str) -> &'a Verdict {
        &r.components.iter().find(|c| c.component == name).expect("component in report").verdict
    }

    /// Owned, so a test can write `evidence_of(&verify(..).unwrap(), name)`
    /// without keeping the report alive by hand.
    fn evidence_of(r: &VerifyReport, name: &str) -> Vec<Evidence> {
        match verdict_of(r, name) {
            Verdict::Quarantine { evidence, .. } => evidence.clone(),
            Verdict::Pass => panic!("{name} passed; expected a quarantine"),
        }
    }

    fn notes_of(r: &VerifyReport, name: &str) -> Vec<Evidence> {
        r.components
            .iter()
            .find(|c| c.component == name)
            .expect("component in report")
            .notes
            .clone()
    }

    /// A stand-in the component derived from the prop rather than the prop
    /// itself. `chars().count()` is 3 in the primary alphabet and 6 in the
    /// differential (see [`Alphabet`]), which is exactly what check 2 exists to
    /// notice — a fixture that substitutes a *constant* is not modelling a
    /// transform at all, because a constant is the same in both renders.
    fn derived(sentinel: &str) -> String {
        sentinel.chars().count().to_string()
    }

    fn checks(evidence: &[Evidence]) -> BTreeSet<Check> {
        evidence.iter().map(|e| e.check).collect()
    }

    // ------------------------------------------------------------ alphabets

    /// Check 2 is only as strong as the two alphabets' disagreement, so the
    /// disagreement is asserted rather than assumed. Every property a component
    /// might read off a string has to differ.
    #[test]
    fn the_two_alphabets_disagree_on_everything_a_component_could_read() {
        let a = Alphabet::Primary.sentinel(7);
        let b = Alphabet::Differential.sentinel(7);
        assert_ne!(a.chars().next(), b.chars().next(), "first character");
        assert_ne!(a.chars().count(), b.chars().count(), "character count");
        assert_ne!(a.len(), b.len(), "byte length");
        assert_ne!(a.as_bytes()[0], b.as_bytes()[0], "first byte");
        // `Swatch`'s `&hex[0..2]` has to be a panic in both, not just in one.
        assert!(!a.is_char_boundary(2) && !b.is_char_boundary(2));
        // No delimiter is shared, so the cross-contamination check cannot
        // false-positive.
        for x in [Alphabet::Primary.open(), Alphabet::Primary.close()] {
            assert!(!b.contains(x));
        }
    }

    /// The only characters either alphabet uses are private-use, which is what
    /// makes them survive `html_escape` untouched.
    #[test]
    fn sentinels_contain_nothing_html_escaping_would_rewrite() {
        for a in Alphabet::ALL {
            let s = a.sentinel(123);
            assert!(!s.contains(['&', '<', '>', '"', '\'']));
        }
    }

    #[test]
    fn scan_separates_whole_sentinels_from_fragments() {
        let a = Alphabet::Primary;
        let s = format!("<b>{}</b>{}<i>{}</i>", a.sentinel(0), a.open(), a.sentinel(12));
        let scan = a.scan(&s);
        assert_eq!(scan.found.iter().map(|f| f.id).collect::<Vec<_>>(), vec![0, 12]);
        assert_eq!(scan.strays.len(), 1);
        assert_eq!(scan.strays[0].ch, a.open());
        // Ids are not confused by a shared prefix: 1 must not match inside 12.
        assert_eq!(a.scan(&a.sentinel(1)).found[0].id, 1);
    }

    // ------------------------------------------------------------ the happy path

    #[test]
    fn a_component_that_passes_its_props_through_passes() {
        let tk = toolkit(vec![comp(
            "Pill",
            "src/atoms/pill.rs",
            27,
            vec![text_prop("label", "src/atoms/pill.rs", 22)],
        )]);
        let renders = pair(
            "Pill",
            "base",
            plan(&[(0, "label", "src/atoms/pill.rs", 22)]),
            |s| html(format!("<span class=\"pill\">{}</span>", s[0])),
        );
        let report = verify(&tk, &renders).unwrap();
        assert_eq!(verdict_of(&report, "Pill"), &Verdict::Pass);
        assert_eq!(report.emitted_count(), 1);
        assert!(report.unexpected.is_empty() && report.missing_expected.is_empty());
        report.self_check().unwrap();
    }

    /// Escaped text around the sentinel is fine — check 2 compares markup, and
    /// the escaping is identical in both runs.
    #[test]
    fn escaped_neighbouring_text_does_not_disturb_the_gate() {
        let tk = toolkit(vec![comp(
            "Callout",
            "src/molecules/callout.rs",
            20,
            vec![text_prop("body", "src/molecules/callout.rs", 12)],
        )]);
        let renders = pair(
            "Callout",
            "base",
            plan(&[(0, "body", "src/molecules/callout.rs", 12)]),
            |s| html(format!("<p title=\"a &amp; b\">&lt;{}&gt;</p>", s[0])),
        );
        assert_eq!(verdict_of(&verify(&tk, &renders).unwrap(), "Callout"), &Verdict::Pass);
    }

    // ------------------------------------------------------------ the two known hard cases

    /// `src/molecules/swatch.rs:11` slices `&hex[0..2]`. The slice is executed
    /// here so the test fails if sentinels ever become byte-index-safe (which
    /// would silently turn this quarantine into a pass).
    #[test]
    fn swatch_panics_and_that_is_a_quarantine_not_a_crash() {
        let tk = toolkit(vec![comp(
            "Swatch",
            "src/molecules/swatch.rs",
            31,
            vec![text_prop("hex", "src/molecules/swatch.rs", 23)],
        )]);
        let renders = pair(
            "Swatch",
            "base",
            plan(&[(0, "hex", "src/molecules/swatch.rs", 23)]),
            |s| {
                let hex = s[0].clone();
                // `contrast_color(&props.hex)`, verbatim enough to panic the
                // same way.
                match std::panic::catch_unwind(move || hex[0..2].to_string()) {
                    Ok(_) => panic!("the sentinel was sliceable at byte 2; \
                                     swatch.rs:11 would not have panicked"),
                    Err(e) => RenderResult::Panicked(
                        e.downcast_ref::<String>().cloned().unwrap_or_else(|| "panicked".into()),
                    ),
                }
            },
        );
        let report = verify(&tk, &renders).unwrap();
        let ev = evidence_of(&report, "Swatch");
        assert_eq!(checks(&ev), BTreeSet::from([Check::Panic]));
        // Both alphabets panic, and the evidence says so for each.
        assert_eq!(ev.len(), 2);
        assert!(ev[0].detail.contains("char boundary"), "{}", ev[0].detail);
        assert_eq!(ev[0].span.file, "src/molecules/swatch.rs");
        assert_eq!(report.emitted_count(), 0);
    }

    /// `src/molecules/dataset_card.rs:43` renders `title.chars().next()`, a
    /// character *derived* from the prop. Two independent checks see it: the
    /// derived character is a bare opening delimiter (check 1), and it is a
    /// different character in the two alphabets (check 2).
    #[test]
    fn dataset_card_derives_an_initial_and_is_quarantined_twice_over() {
        let tk = toolkit(vec![comp(
            "DatasetCard",
            "src/molecules/dataset_card.rs",
            42,
            vec![text_prop("title", "src/molecules/dataset_card.rs", 20)],
        )]);
        let renders = pair(
            "DatasetCard",
            "base",
            plan(&[(0, "title", "src/molecules/dataset_card.rs", 20)]),
            |s| {
                let initial =
                    s[0].chars().next().unwrap_or('?').to_uppercase().to_string();
                html(format!(
                    "<a class=\"dataset-card\"><span class=\"dataset-card__thumb--empty\">\
                     {initial}</span><h3>{}</h3></a>",
                    s[0]
                ))
            },
        );
        let report = verify(&tk, &renders).unwrap();
        let ev = evidence_of(&report, "DatasetCard");
        assert_eq!(
            checks(&ev),
            BTreeSet::from([Check::SentinelIntegrity, Check::DifferentialAlphabet])
        );
        let fragment = ev.iter().find(|e| e.check == Check::SentinelIntegrity).unwrap();
        assert!(fragment.detail.contains("fragment of a sentinel"), "{}", fragment.detail);
        assert!(fragment.detail.contains("\\u{E000}"), "{}", fragment.detail);
        // The integrity failure is about the markup as a whole, so it names the
        // component; the differential failure names the prop nearest the diff.
        let diff = ev.iter().find(|e| e.check == Check::DifferentialAlphabet).unwrap();
        assert_eq!(diff.span.line, 20, "{diff}");
        match verdict_of(&report, "DatasetCard") {
            Verdict::Quarantine { reason, .. } => {
                assert!(reason.contains("unmodified"), "{reason}")
            }
            Verdict::Pass => unreachable!(),
        }
    }

    // ------------------------------------- what the widened pad buys (and not)

    /// Run one transform through the real gate and say whether it quarantined.
    ///
    /// Each closure is the *shape* of a real Rust component body, executed
    /// against the planted sentinel text rather than paraphrased, so a test
    /// passing here means the gate would catch the corresponding component.
    fn caught(render: impl Fn(&str) -> String + Copy) -> bool {
        let tk = toolkit(vec![comp(
            "Probe",
            "src/atoms/probe.rs",
            10,
            vec![text_prop("label", "src/atoms/probe.rs", 5)],
        )]);
        let renders = pair(
            "Probe",
            "base",
            plan(&[(0, "label", "src/atoms/probe.rs", 5)]),
            |s| html(format!("<span>{}</span>", render(&s[0]))),
        );
        let report = verify(&tk, &renders).unwrap();
        matches!(verdict_of(&report, "Probe"), Verdict::Quarantine { .. })
    }

    /// The four shapes an audit found passing before [`PAD`] existed. Each was
    /// the identity on a digits-only payload, so both alphabets agreed and the
    /// gate certified a component whose React port renders something else.
    #[test]
    fn the_pad_catches_the_transforms_a_digits_only_payload_hid() {
        // `to_uppercase` / `to_lowercase`: digits have no case.
        assert!(caught(|s| s.to_uppercase()), "to_uppercase");
        assert!(caught(|s| s.to_lowercase()), "to_lowercase");

        // Truncation above any threshold the old payloads (3 and 6 chars) could
        // not reach.
        assert!(caught(|s| s.chars().take(20).collect::<String>()), "truncate(20)");

        // A class chosen by byte length. Detection used to need the threshold to
        // fall strictly between 7 and 12 bytes; the window is now 7..=81.
        assert!(
            caught(|s| format!(
                "<b class=\"{}\">{s}</b>",
                if s.len() > 24 { "long" } else { "short" }
            )),
            "length-threshold class at 24 bytes"
        );

        // `src/molecules/annotation.rs:24`'s `short_datatype`. This is the one
        // that actually shipped wrong.
        assert!(
            caught(|s| match s.rfind('#').or_else(|| s.rfind('/')) {
                Some(i) => s[i + 1..].to_string(),
                None => s.to_string(),
            }),
            "slice after the last `#` or `/`"
        );

        // The control: a pass-through still passes, so none of the above is the
        // gate simply refusing everything.
        assert!(!caught(|s| s.to_string()), "pass-through must still pass");
    }

    /// Stated as a test so the limit is a fact in the repo rather than a claim
    /// in a README: a branch on a *literal* value is invisible to any probe
    /// alphabet, because the gate cannot guess a constant it was never told
    /// about. `xtask/README.md` says so; this is what "says so" means.
    #[test]
    fn a_branch_on_a_literal_value_is_still_invisible_and_that_is_recorded() {
        assert!(
            !caught(|s| if s == "danger" { "<b>!</b>".to_string() } else { s.to_string() }),
            "a literal-value branch is expected to pass; if this starts failing the \
             README's stated limitation is out of date"
        );
    }

    /// An audit proposed making the sentinel alphabet ASCII and hex-safe so that
    /// `src/molecules/swatch.rs:11`'s `&hex[0..2]` would stop panicking, on the
    /// reasoning that Swatch and PaletteGroup are absent only as collateral from
    /// the probe alphabet. Not emitting them is the right outcome, and this test
    /// is why: the panic is the *loud* symptom of a transform that would
    /// otherwise be silent.
    ///
    /// `contrast_color` (src/molecules/swatch.rs:7-16) parses the prop as hex and
    /// picks a foreground from the YIQ luminance, with `unwrap_or(0)` on a parse
    /// failure. No sentinel is valid hex under any alphabet, so every channel is
    /// 0, the luminance is 0, and both alphabets render the same constant
    /// `#ffffff`. The two renders agree, the sentinel itself passes through
    /// intact, and the gate certifies it — so a hex-safe alphabet would not
    /// recover Swatch, it would emit a Swatch whose foreground is frozen to
    /// white and unreadable on every light swatch in the palette.
    ///
    /// The closure is `contrast_color`'s body with the panic removed exactly as
    /// that audit proposed (`hex.get(0..2)` for `&hex[0..2]`), so what passes
    /// here is what that change would have shipped.
    #[test]
    fn a_hex_safe_alphabet_would_not_recover_swatch_it_would_silence_it() {
        fn contrast_color_without_the_panic(hex: &str) -> &'static str {
            let hex = hex.trim_start_matches('#');
            let byte = |s: Option<&str>| {
                s.and_then(|s| u8::from_str_radix(s, 16).ok()).unwrap_or(0) as f32
            };
            let (r, g, b) = (byte(hex.get(0..2)), byte(hex.get(2..4)), byte(hex.get(4..6)));
            let yiq = (r * 299.0 + g * 587.0 + b * 114.0) / 1000.0;
            if yiq >= 150.0 { "#040553" } else { "#ffffff" }
        }

        // Every probe lands on the same arm, so the gate sees a constant and has
        // nothing to compare. `swatch.rs:32` writes exactly this style string.
        assert_eq!(contrast_color_without_the_panic("\u{E000}0001\u{E001}"), "#ffffff");
        assert_eq!(contrast_color_without_the_panic("#ffffff"), "#040553");

        assert!(
            !caught(|s| format!(
                "<b class=\"swatch-color\" style=\"background:{s};color:{};\">{s}</b>",
                contrast_color_without_the_panic(s)
            )),
            "if this starts failing, the gate learned to see a value-keyed branch and \
             abi/overrides.toml's Swatch and PaletteGroup entries should be revisited"
        );
    }

    /// Check 1's order half. Both alphabets permute a reversed list identically,
    /// and a content sort sorts by the id digits, which rise in both — so
    /// neither the integrity count nor the differential comparison sees it.
    #[test]
    fn a_reversed_list_is_caught_even_though_both_alphabets_agree() {
        let tk = toolkit(vec![comp(
            "List",
            "src/molecules/list.rs",
            10,
            vec![text_prop("items", "src/molecules/list.rs", 5)],
        )]);
        let p = plan(&[
            (0, "items[0].text", "src/molecules/list.rs", 5),
            (1, "items[1].text", "src/molecules/list.rs", 5),
            (2, "items[2].text", "src/molecules/list.rs", 5),
        ]);
        let forward = pair("List", "base", p.clone(), |s| {
            html(format!("<ul><li>{}</li><li>{}</li><li>{}</li></ul>", s[0], s[1], s[2]))
        });
        assert!(
            matches!(verdict_of(&verify(&tk, &forward).unwrap(), "List"), Verdict::Pass),
            "the control must pass"
        );

        let reversed = pair("List", "base", p, |s| {
            html(format!("<ul><li>{}</li><li>{}</li><li>{}</li></ul>", s[2], s[1], s[0]))
        });
        let report = verify(&tk, &reversed).unwrap();
        let ev = evidence_of(&report, "List");
        assert!(ev.iter().any(|e| e.check == Check::SentinelOrder), "{ev:?}");
        assert!(
            ev.iter().all(|e| e.check != Check::DifferentialAlphabet),
            "the two alphabets agree on a reversal — that is the whole point: {ev:?}"
        );
        let order = ev.iter().find(|e| e.check == Check::SentinelOrder).unwrap();
        assert!(order.detail.contains("items[*].text"), "{}", order.detail);
    }

    /// The index blanked is the one that moved, at whatever depth.
    #[test]
    fn blanked_yields_one_key_per_index_position() {
        assert_eq!(blanked("title"), Vec::<(String, usize)>::new());
        assert_eq!(blanked("items[2].text"), vec![("items[*].text".to_string(), 2)]);
        assert_eq!(
            blanked("groups[1].items[3].text"),
            vec![
                ("groups[*].items[3].text".to_string(), 1),
                ("groups[1].items[*].text".to_string(), 3),
            ]
        );
    }

    /// The pad has to be safe everywhere a prop value lands. A character that
    /// Yew escapes, or that `splice`'s class/style readers treat as structure,
    /// would make the two renders differ for a reason that is not a transform —
    /// a false quarantine that looks exactly like a true one.
    #[test]
    fn the_pad_contains_nothing_that_escaping_or_splice_would_rewrite() {
        for c in PAD.chars() {
            assert!(c.is_ascii(), "{c:?} is not ASCII");
            assert!(
                !matches!(c, '&' | '<' | '>' | '"' | '\'' | ';' | ':'),
                "{c:?} is rewritten by html_escape or read as structure by splice"
            );
            assert!(!c.is_whitespace(), "{c:?} would be collapsed inside a class attribute");
        }
        // `scan` reads the id with `take_while(is_ascii_digit)` and then expects
        // the pad, so a pad starting with a digit would be eaten by the id.
        assert!(!PAD.starts_with(|c: char| c.is_ascii_digit()));
        // The detections the pad exists for.
        assert!(PAD.contains('#') && PAD.contains('/'));
        assert!(PAD.chars().any(|c| c.is_ascii_uppercase()));
        assert!(PAD.chars().any(|c| c.is_ascii_lowercase()));
        assert!(PAD.len() > 64, "the length-threshold window is 7..={}", PAD.len() + 11);
    }

    /// The expected quarantines together: the gate's own success criterion. A run
    /// in which `Swatch` *passes* is a run in which the panic capture stopped
    /// working, and a silent pass would look exactly like success.
    #[test]
    fn the_expected_quarantines_are_reported_and_a_surprise_is_an_error() {
        let tk = toolkit(vec![
            comp("Swatch", "src/molecules/swatch.rs", 31, vec![text_prop("hex", "src/molecules/swatch.rs", 23)]),
            comp("DatasetCard", "src/molecules/dataset_card.rs", 42, vec![text_prop("title", "src/molecules/dataset_card.rs", 20)]),
            comp("Pill", "src/atoms/pill.rs", 27, vec![text_prop("label", "src/atoms/pill.rs", 22)]),
        ]);
        let mut renders = Vec::new();
        renders.extend(pair("Swatch", "base", plan(&[(0, "hex", "src/molecules/swatch.rs", 23)]), |_| {
            RenderResult::Panicked("byte index 2 is not a char boundary".into())
        }));
        renders.extend(pair("DatasetCard", "base", plan(&[(0, "title", "src/molecules/dataset_card.rs", 20)]), |s| {
            html(format!("<span>{}</span><h3>{}</h3>", s[0].chars().next().unwrap(), s[0]))
        }));
        renders.extend(pair("Pill", "base", plan(&[(0, "label", "src/atoms/pill.rs", 22)]), |s| {
            html(format!("<span>{}</span>", s[0]))
        }));

        let report = verify(&tk, &renders).unwrap();
        let q: Vec<&str> = report.quarantined().map(|c| c.component.as_str()).collect();
        // IR order, not alphabetical: the report reads like `dump-ir`.
        assert_eq!(q, vec!["Swatch", "DatasetCard"]);
        assert!(report.unexpected.is_empty(), "{:?}", report.unexpected);
        assert!(report.missing_expected.is_empty(), "{:?}", report.missing_expected);
        report.self_check().unwrap();
        assert_eq!(report.emitted_count(), 1);
    }

    // ------------------------------------------------------------ check 2 on its own

    /// The whole reason for a second alphabet: a component that reads the
    /// prop's *length* passes check 1 completely — every sentinel arrives whole
    /// and exactly once — and is still not a function of the prop in the way
    /// splicing needs.
    #[test]
    fn a_content_length_dependence_is_invisible_to_check_1_and_caught_by_check_2() {
        let tk = toolkit(vec![comp(
            "TagLabel",
            "src/atoms/tag_label.rs",
            15,
            vec![text_prop("text", "src/atoms/tag_label.rs", 9)],
        )]);
        let renders = pair(
            "TagLabel",
            "base",
            plan(&[(0, "text", "src/atoms/tag_label.rs", 9)]),
            |s| html(format!("<span data-len=\"{}\">{}</span>", s[0].len(), s[0])),
        );
        let report = verify(&tk, &renders).unwrap();
        let ev = evidence_of(&report, "TagLabel");
        assert_eq!(
            checks(&ev),
            BTreeSet::from([Check::DifferentialAlphabet]),
            "check 1 must be clean here, or the test is not proving check 2 is needed"
        );
        assert!(ev[0].detail.contains("differ from byte"), "{}", ev[0].detail);
    }

    /// The same, for a component that branches on the first character — the
    /// shape of `Swatch`'s YIQ test, minus the panic.
    #[test]
    fn a_first_character_dependence_is_caught_by_check_2() {
        let tk = toolkit(vec![comp(
            "Pill",
            "src/atoms/pill.rs",
            27,
            vec![text_prop("label", "src/atoms/pill.rs", 22)],
        )]);
        let renders = pair("Pill", "base", plan(&[(0, "label", "src/atoms/pill.rs", 22)]), |s| {
            let class = if s[0].starts_with('\u{E000}') { "pill--a" } else { "pill--b" };
            html(format!("<span class=\"{class}\">{}</span>", s[0]))
        });
        let ev = evidence_of(&verify(&tk, &renders).unwrap(), "Pill");
        assert_eq!(checks(&ev), BTreeSet::from([Check::DifferentialAlphabet]));
    }

    // ------------------------------------------------------------ check 1 on its own

    #[test]
    fn a_prop_that_never_reaches_the_markup_is_caught() {
        let tk = toolkit(vec![comp(
            "SectionHead",
            "src/molecules/section_head.rs",
            18,
            vec![text_prop("eyebrow", "src/molecules/section_head.rs", 8)],
        )]);
        let renders = pair(
            "SectionHead",
            "base",
            plan(&[(0, "eyebrow", "src/molecules/section_head.rs", 8)]),
            |s| html(format!("<h2>{}</h2>", derived(&s[0]))),
        );
        let ev = evidence_of(&verify(&tk, &renders).unwrap(), "SectionHead");
        assert_eq!(
            checks(&ev),
            BTreeSet::from([Check::SentinelIntegrity, Check::DifferentialAlphabet])
        );
        assert!(
            ev.iter().any(|e| e.detail.contains("never reached the markup") && e.span.line == 8),
            "the evidence names the prop, not the component: {ev:?}"
        );
    }

    /// The same shape with nothing derived in the sentinel's place. A prop a
    /// component does not read in a given cell — `src/molecules/annotation.rs:35`
    /// prefers `language` and never looks at `datatype` — is a fact `splice` has
    /// to know and not a reason to drop the component.
    #[test]
    fn a_prop_a_cell_simply_does_not_read_is_a_note_not_a_quarantine() {
        let tk = toolkit(vec![comp(
            "SectionHead",
            "src/molecules/section_head.rs",
            18,
            vec![text_prop("eyebrow", "src/molecules/section_head.rs", 8)],
        )]);
        let renders = pair(
            "SectionHead",
            "base",
            plan(&[(0, "eyebrow", "src/molecules/section_head.rs", 8)]),
            |_| html("<h2>fixed</h2>".to_string()),
        );
        let report = verify(&tk, &renders).unwrap();
        assert!(!report.components[0].is_quarantined(), "a constant is not a transform");
        let notes = notes_of(&report, "SectionHead");
        assert!(
            notes.iter().any(|n| n.detail.contains("never reached the markup")),
            "the finding must still be carried to splice: {notes:?}"
        );
        assert_eq!(notes[0].span.line, 8);
    }

    #[test]
    fn a_sentinel_rendered_twice_is_reported_rather_than_assumed_harmless() {
        let tk = toolkit(vec![comp(
            "Button",
            "src/atoms/button.rs",
            29,
            vec![text_prop("label", "src/atoms/button.rs", 21)],
        )]);
        let renders = pair("Button", "base", plan(&[(0, "label", "src/atoms/button.rs", 21)]), |s| {
            html(format!("<a aria-label=\"{}\">{}</a>", s[0], s[0]))
        });
        let report = verify(&tk, &renders).unwrap();
        assert!(
            !report.components[0].is_quarantined(),
            "both occurrences are whole and the alphabets agree, so nothing was derived"
        );
        let notes = notes_of(&report, "Button");
        assert!(notes[0].detail.contains("appears 2 times"), "{}", notes[0].detail);
        assert!(
            notes[0].detail.contains("write the JSX expression into all 2"),
            "the note has to tell splice what to do: {}",
            notes[0].detail
        );
        assert_eq!(notes[0].span.line, 21);
    }

    /// The same duplication with one of the two occurrences derived. Check 1
    /// sees the same arity; only check 2 separates this from the case above.
    #[test]
    fn a_duplicate_alongside_a_derived_copy_still_quarantines() {
        let tk = toolkit(vec![comp(
            "Button",
            "src/atoms/button.rs",
            29,
            vec![text_prop("label", "src/atoms/button.rs", 21)],
        )]);
        let renders = pair("Button", "base", plan(&[(0, "label", "src/atoms/button.rs", 21)]), |s| {
            html(format!("<a aria-label=\"{}\" data-len=\"{}\">{}</a>", s[0], derived(&s[0]), s[0]))
        });
        let report = verify(&tk, &renders).unwrap();
        assert!(report.components[0].is_quarantined());
        let ev = evidence_of(&report, "Button");
        assert!(ev.iter().any(|e| e.check == Check::DifferentialAlphabet), "{ev:?}");
    }

    #[test]
    fn a_sentinel_nobody_planted_is_caught() {
        let tk = toolkit(vec![comp(
            "Hero",
            "src/organisms/hero.rs",
            25,
            vec![text_prop("title", "src/organisms/hero.rs", 10)],
        )]);
        let renders = pair("Hero", "base", plan(&[(0, "title", "src/organisms/hero.rs", 10)]), |s| {
            html(format!("<h1>{}</h1><p>{}</p>", s[0], Alphabet::Primary.sentinel(99)))
        });
        let ev = evidence_of(&verify(&tk, &renders).unwrap(), "Hero");
        assert!(
            ev.iter().any(|e| e.detail.contains("sentinel id 99 was never planted")),
            "{ev:?}"
        );
    }

    /// A render carrying the other alphabet's delimiters means `render` reused a
    /// value across runs, which would make check 2 compare a render with itself.
    #[test]
    fn cross_contamination_between_the_two_runs_is_caught() {
        let tk = toolkit(vec![comp(
            "Pill",
            "src/atoms/pill.rs",
            27,
            vec![text_prop("label", "src/atoms/pill.rs", 22)],
        )]);
        let p = plan(&[(0, "label", "src/atoms/pill.rs", 22)]);
        let renders = vec![
            CellRender {
                component: "Pill".into(),
                cell: "base".into(),
                alphabet: Alphabet::Primary,
                plan: p.clone(),
                result: html(format!("<span>{}</span>", Alphabet::Primary.sentinel(0))),
            },
            CellRender {
                component: "Pill".into(),
                cell: "base".into(),
                alphabet: Alphabet::Differential,
                plan: p,
                // The cached primary markup, handed back for the second run.
                result: html(format!("<span>{}</span>", Alphabet::Primary.sentinel(0))),
            },
        ];
        let ev = evidence_of(&verify(&tk, &renders).unwrap(), "Pill");
        assert!(ev.iter().any(|e| e.detail.contains("leaked into")), "{ev:?}");
    }

    // ------------------------------------------------------------ multi-cell

    /// One bad cell quarantines the component; the verdict still records how
    /// many cells were checked, so a one-in-eight failure is not read as total.
    #[test]
    fn a_single_failing_cell_quarantines_the_whole_component() {
        let c = comp(
            "MemberCard",
            "src/molecules/member_card.rs",
            30,
            vec![text_prop("name", "src/molecules/member_card.rs", 10), opt_prop("logo", "src/molecules/member_card.rs", 14)],
        );
        let tk = toolkit(vec![c.clone()]);
        let ids: Vec<String> =
            matrix::cells(&c, &tk).unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), 2, "one Option prop is two cells: {ids:?}");

        let mut renders = Vec::new();
        for id in &ids {
            let drop_it = id != "base";
            renders.extend(pair(
                "MemberCard",
                id,
                plan(&[(0, "name", "src/molecules/member_card.rs", 10)]),
                move |s| {
                    if drop_it {
                        // Derived, not constant: dropping the prop for a fixed
                        // string is a cell that does not read it, which is a note.
                        html(format!("<div class=\"member-card\">{}</div>", derived(&s[0])))
                    } else {
                        html(format!("<div>{}</div>", s[0]))
                    }
                },
            ));
        }
        let report = verify(&tk, &renders).unwrap();
        let v = &report.components[0];
        assert_eq!(v.cells, 2);
        assert!(v.is_quarantined());
        let ev = evidence_of(&report, "MemberCard");
        assert!(ev.iter().all(|e| e.cell != "base"), "only the second cell fails: {ev:?}");
    }

    // ------------------------------------------------------------ seams, not components

    /// The seam `matrix` -> `render`: a cell that was derived but never rendered
    /// is lost coverage with every check still green, so it is an error and not
    /// a quarantine.
    #[test]
    fn a_cell_the_renderer_skipped_is_an_error_not_a_pass() {
        let c = comp(
            "MemberCard",
            "src/molecules/member_card.rs",
            30,
            vec![opt_prop("logo", "src/molecules/member_card.rs", 14)],
        );
        let tk = toolkit(vec![c]);
        let renders = pair("MemberCard", "base", plan(&[]), |_| html("<div/>".into()));
        let err = verify(&tk, &renders).unwrap_err().to_string();
        assert!(err.contains("do not match the matrix"), "{err}");
        assert!(err.contains("src/molecules/member_card.rs:30"), "{err}");
    }

    #[test]
    fn a_component_that_was_never_rendered_cannot_be_cleared() {
        let tk = toolkit(vec![comp(
            "Pill",
            "src/atoms/pill.rs",
            27,
            vec![text_prop("label", "src/atoms/pill.rs", 22)],
        )]);
        let err = verify(&tk, &[]).unwrap_err().to_string();
        assert!(err.contains("never rendered"), "{err}");
    }

    #[test]
    fn a_cell_rendered_in_only_one_alphabet_is_an_error() {
        let tk = toolkit(vec![comp(
            "Pill",
            "src/atoms/pill.rs",
            27,
            vec![text_prop("label", "src/atoms/pill.rs", 22)],
        )]);
        let mut renders = pair("Pill", "base", plan(&[(0, "label", "src/atoms/pill.rs", 22)]), |s| {
            html(format!("<span>{}</span>", s[0]))
        });
        renders.retain(|r| r.alphabet == Alphabet::Primary);
        let err = verify(&tk, &renders).unwrap_err().to_string();
        assert!(err.contains("no differential render"), "{err}");
    }

    #[test]
    fn two_renders_of_a_cell_that_planted_different_sentinels_are_an_error() {
        let tk = toolkit(vec![comp(
            "Pill",
            "src/atoms/pill.rs",
            27,
            vec![text_prop("label", "src/atoms/pill.rs", 22)],
        )]);
        let mut renders = pair("Pill", "base", plan(&[(0, "label", "src/atoms/pill.rs", 22)]), |s| {
            html(format!("<span>{}</span>", s[0]))
        });
        renders[1].plan = plan(&[(0, "tone", "src/atoms/pill.rs", 24)]);
        let err = verify(&tk, &renders).unwrap_err().to_string();
        assert!(err.contains("different sentinels"), "{err}");
    }

    #[test]
    fn reusing_a_sentinel_id_for_two_props_is_an_error() {
        let tk = toolkit(vec![comp(
            "Pill",
            "src/atoms/pill.rs",
            27,
            vec![text_prop("label", "src/atoms/pill.rs", 22)],
        )]);
        let renders = pair(
            "Pill",
            "base",
            plan(&[(0, "label", "src/atoms/pill.rs", 22), (0, "tone", "src/atoms/pill.rs", 24)]),
            |s| html(format!("<span>{}{}</span>", s[0], s[1])),
        );
        let err = verify(&tk, &renders).unwrap_err().to_string();
        assert!(err.contains("same sentinel id"), "{err}");
    }

    #[test]
    fn rendering_the_same_cell_twice_in_one_alphabet_is_an_error() {
        let tk = toolkit(vec![comp("Pill", "src/atoms/pill.rs", 27, vec![])]);
        let mut renders = pair("Pill", "base", plan(&[]), |_| html("<span/>".into()));
        renders.push(renders[0].clone());
        let err = verify(&tk, &renders).unwrap_err().to_string();
        assert!(err.contains("rendered twice"), "{err}");
    }

    #[test]
    fn a_rendered_component_the_ir_does_not_know_is_an_error() {
        let tk = toolkit(vec![]);
        let renders = pair("Ghost", "base", plan(&[]), |_| html("<span/>".into()));
        let err = verify(&tk, &renders).unwrap_err().to_string();
        assert!(err.contains("is not in the IR"), "{err}");
    }

    // ------------------------------------------------------------ the harness -> render seam

    #[test]
    fn collect_rejoins_the_harness_plan_to_the_render_output() {
        let plans = BTreeMap::from([
            (
                "Pill/base".to_string(),
                RenderPlan {
                    component: "Pill".into(),
                    cell: "base".into(),
                    alphabet: Alphabet::Primary,
                    sentinels: plan(&[(0, "label", "src/atoms/pill.rs", 22)]),
                },
            ),
            (
                "Pill/base#differential".to_string(),
                RenderPlan {
                    component: "Pill".into(),
                    cell: "base".into(),
                    alphabet: Alphabet::Differential,
                    sentinels: plan(&[(0, "label", "src/atoms/pill.rs", 22)]),
                },
            ),
        ]);
        let results = BTreeMap::from([
            (
                "Pill/base".to_string(),
                RenderResult::Html(format!("<span>{}</span>", Alphabet::Primary.sentinel(0))),
            ),
            (
                "Pill/base#differential".to_string(),
                RenderResult::Html(format!(
                    "<span>{}</span>",
                    Alphabet::Differential.sentinel(0)
                )),
            ),
        ]);
        let renders = collect(&results, &plans).unwrap();
        let tk = toolkit(vec![comp(
            "Pill",
            "src/atoms/pill.rs",
            27,
            vec![text_prop("label", "src/atoms/pill.rs", 22)],
        )]);
        assert_eq!(verdict_of(&verify(&tk, &renders).unwrap(), "Pill"), &Verdict::Pass);
    }

    #[test]
    fn collect_refuses_a_plan_with_no_result_and_a_result_with_no_plan() {
        let p = RenderPlan {
            component: "Pill".into(),
            cell: "base".into(),
            alphabet: Alphabet::Primary,
            sentinels: vec![],
        };
        let plans = BTreeMap::from([("Pill/base".to_string(), p)]);
        let err = collect(&BTreeMap::new(), &plans).unwrap_err().to_string();
        assert!(err.contains("no result for it"), "{err}");

        let results =
            BTreeMap::from([("Ghost/base".to_string(), RenderResult::Html("<i/>".into()))]);
        let err = collect(&results, &BTreeMap::new()).unwrap_err().to_string();
        assert!(err.contains("never planned"), "{err}");
    }

    // ------------------------------------------------------------ the gate's own health

    #[test]
    fn a_quarantine_nobody_predicted_is_surfaced_separately() {
        let tk = toolkit(vec![comp(
            "PropsTable",
            "src/molecules/props_table.rs",
            50,
            vec![text_prop("name", "src/molecules/props_table.rs", 12)],
        )]);
        let renders = pair(
            "PropsTable",
            "base",
            plan(&[(0, "name", "src/molecules/props_table.rs", 12)]),
            |s| html(format!("<table>{}</table>", derived(&s[0]))),
        );
        let report = verify(&tk, &renders).unwrap();
        assert_eq!(report.unexpected, vec!["PropsTable".to_string()]);
        let err = report.self_check().unwrap_err().to_string();
        assert!(err.contains("unexpected quarantine"), "{err}");
    }

    /// The gate failing to catch what it exists to catch looks exactly like a
    /// clean run unless it is stated, so it is stated.
    #[test]
    fn an_expected_quarantine_that_passes_fails_the_self_check() {
        let tk = toolkit(vec![comp(
            "Swatch",
            "src/molecules/swatch.rs",
            31,
            vec![text_prop("hex", "src/molecules/swatch.rs", 23)],
        )]);
        // A `Swatch` that passed everything — which is what a broken panic
        // capture, or a sentinel that stopped being multi-byte, would look like.
        let renders = pair("Swatch", "base", plan(&[(0, "hex", "src/molecules/swatch.rs", 23)]), |s| {
            html(format!("<button class=\"swatch\">{}</button>", s[0]))
        });
        let report = verify(&tk, &renders).unwrap();
        assert_eq!(verdict_of(&report, "Swatch"), &Verdict::Pass);
        assert_eq!(report.missing_expected, vec!["Swatch".to_string()]);
        let err = report.self_check().unwrap_err().to_string();
        assert!(err.contains("gate itself has regressed"), "{err}");
    }

    /// A run over a subset of the crate has not had the chance to quarantine
    /// `Swatch`, so its absence is not a regression.
    #[test]
    fn a_component_that_was_not_rendered_is_not_a_missing_quarantine() {
        let tk = toolkit(vec![comp(
            "Pill",
            "src/atoms/pill.rs",
            27,
            vec![text_prop("label", "src/atoms/pill.rs", 22)],
        )]);
        let renders = pair("Pill", "base", plan(&[(0, "label", "src/atoms/pill.rs", 22)]), |s| {
            html(format!("<span>{}</span>", s[0]))
        });
        let report = verify(&tk, &renders).unwrap();
        assert!(report.missing_expected.is_empty());
        report.self_check().unwrap();
    }

    // ------------------------------------------------------------ IR and artifact

    #[test]
    fn apply_stamps_the_quarantine_into_the_ir_with_its_reason() {
        let mut tk = toolkit(vec![
            comp("Swatch", "src/molecules/swatch.rs", 31, vec![text_prop("hex", "src/molecules/swatch.rs", 23)]),
            comp("Pill", "src/atoms/pill.rs", 27, vec![text_prop("label", "src/atoms/pill.rs", 22)]),
        ]);
        let mut renders = pair("Swatch", "base", plan(&[(0, "hex", "src/molecules/swatch.rs", 23)]), |_| {
            RenderResult::Panicked("byte index 2 is not a char boundary".into())
        });
        renders.extend(pair("Pill", "base", plan(&[(0, "label", "src/atoms/pill.rs", 22)]), |s| {
            html(format!("<span>{}</span>", s[0]))
        }));
        let report = verify(&tk, &renders).unwrap();
        apply(&mut tk, &report).unwrap();
        match &tk.component("Swatch").unwrap().status {
            Status::Quarantined { reason } => assert!(reason.contains("panics"), "{reason}"),
            other => panic!("expected Quarantined, got {other:?}"),
        }
        assert_eq!(tk.component("Pill").unwrap().status, Status::Ok);
        assert_eq!(tk.generated().count(), 1);
    }

    /// An `Excluded` component reaching the renderer would have its exclusion
    /// rule overwritten by the quarantine reason — the one thing the ABI must
    /// not lose.
    #[test]
    fn apply_refuses_to_overwrite_an_exclusion_rule() {
        let mut c = comp("BeeNest", "src/organisms/beenest.rs", 35, vec![text_prop("x", "src/organisms/beenest.rs", 29)]);
        c.status = Status::Excluded { reason: "build.rs OUT_DIR type".into() };
        let mut tk = toolkit(vec![c]);
        let report = VerifyReport {
            components: vec![ComponentVerdict {
                component: "BeeNest".into(),
                span: Span { file: "src/organisms/beenest.rs".into(), line: 35 },
                cells: 1,
                notes: vec![],
                verdict: Verdict::Quarantine { reason: "panics".into(), evidence: vec![] },
            }],
            ..VerifyReport::default()
        };
        let err = apply(&mut tk, &report).unwrap_err().to_string();
        assert!(err.contains("build.rs OUT_DIR type"), "{err}");
    }

    #[test]
    fn the_quarantine_artifact_carries_the_evidence_and_the_source_hash() {
        let tk = toolkit(vec![
            comp("DatasetCard", "src/molecules/dataset_card.rs", 42, vec![text_prop("title", "src/molecules/dataset_card.rs", 20)]),
            comp("Pill", "src/atoms/pill.rs", 27, vec![text_prop("label", "src/atoms/pill.rs", 22)]),
        ]);
        let mut renders = pair("DatasetCard", "base", plan(&[(0, "title", "src/molecules/dataset_card.rs", 20)]), |s| {
            html(format!("<span>{}</span><h3>{}</h3>", s[0].chars().next().unwrap(), s[0]))
        });
        renders.extend(pair("Pill", "base", plan(&[(0, "label", "src/atoms/pill.rs", 22)]), |s| {
            html(format!("<span>{}</span>", s[0]))
        }));
        let report = verify(&tk, &renders).unwrap();

        let dir = std::env::temp_dir().join(format!("xtask-verify-{}", std::process::id()));
        let path = dir.join("quarantine.json");
        report.write(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::remove_dir_all(&dir).ok();

        let json: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(json["src_hash"], "test");
        assert_eq!(json["checked"], 2);
        assert_eq!(json["emitted"], 1);
        assert_eq!(json["passed"][0], "Pill");
        assert_eq!(json["quarantined"][0]["component"], "DatasetCard");
        assert_eq!(json["quarantined"][0]["verdict"], "quarantine");
        assert_eq!(json["quarantined"][0]["span"]["line"], 42);
        assert!(json["quarantined"][0]["evidence"][0]["detail"].is_string());
        // Sentinels are escaped in the artifact: private-use code points that
        // round-trip as invisible glyphs are not evidence.
        assert!(!text.contains('\u{E000}'));
    }

    #[test]
    fn excerpts_stay_on_character_boundaries_and_show_invisible_bytes() {
        let s = format!("«{}»", Alphabet::Differential.sentinel(3));
        let e = excerpt(&s, 2);
        assert!(e.contains("\\u{F0000}"), "{e}");
        assert!(e.contains('«'), "{e}");
    }
}
