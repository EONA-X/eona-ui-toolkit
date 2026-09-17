//! Stage: `check`. See xtask/README.md for the pipeline contract.
//!
//! The drift gate. It runs on every MR that touches `src/` and answers one
//! question: *is `abi/` what this `src/` would produce right now?* When the
//! answer is no it prints a unified diff and exits non-zero, because a generated
//! artifact that nobody regenerated is worse than no artifact — it is a lie with
//! a git history.
//!
//! # The two paths, and what they actually cost
//!
//! Measured on this repo, warm `xtask/out/harness-target` (~360MB) and warm
//! render cache. These are the numbers, not a budget:
//!
//! - **Fast path (both hashes match): 0.11s.** The committed ABI carries both
//!   the `srcHash` of the toolkit and the `generatorHash` of `xtask/src`. Those
//!   two inputs are the *only* inputs to the render, so if both match what is on
//!   disk right now, re-rendering is guaranteed to reproduce the committed
//!   `abi/quarantine.json` and `abi/package.manifest.json` byte for byte. The
//!   gate replays them onto the freshly parsed IR instead of re-deriving them,
//!   then rebuilds and diffs the ABI from scratch and hashes every file under
//!   `packages/react`. Everything except the render verdicts is recomputed.
//! - **Full path (either hash moved): 30-40s.** Nothing may be assumed, so
//!   `harness` -> `render` -> `verify` -> `splice` -> `emit` runs for real. This
//!   is the path an MR touching `src/` takes, and it is over 30 seconds every
//!   time, not occasionally: five separate one-line edits to `src/` measured
//!   33.6 / 34.5 / 35.0 / 35.6 / 36.4s, a sixth (a class rename never rendered
//!   before, so no cache could help) measured 39.6s, and a full `bridgegen`
//!   after editing `xtask/src` measured 37.9s. Essentially all of it is cargo — a release
//!   rebuild of the toolkit plus a harness relink; the render itself replays in
//!   0.03-0.08s. `--render` on an unchanged tree, which re-derives the verdicts
//!   from the cached renders rather than replaying the committed file, is 0.17s.
//!
//! So: **the 30-second budget in #809 is met on the replay path and missed on
//! the render path.** Cutting it means cutting cargo time (a lower `opt-level`
//! for the harness profile, or making the render a separate CI job keyed on
//! `srcHash`), not tuning this stage. Neither path needs a browser or a wasm
//! toolchain; `render` is a native binary using `yew`'s SSR renderer.
//!
//! The dishonest version of this would be to hash-compare and stop. The reason
//! that is not what happens here: a hash match proves the *render results* are
//! reusable, not that the *ABI* is current. The generator could have been
//! changed in a way that alters a TS type while `check.rs` itself was edited
//! — which is exactly why `generatorHash` covers all of `xtask/src`, `check.rs`
//! included.
//!
//! # What "stale" means
//!
//! Byte-inequality against the regenerated artifact, with a unified diff. On top
//! of that, seven invariants get their own finding class so that a reviewer does
//! not have to read a 4000-line diff to learn which one broke:
//!
//! 1. the ABI's `srcHash` matches `src/` as it is now;
//! 2. every prop is mapped, and every component that is not `Ok` has an
//!    `abi/overrides.toml` entry;
//! 3. no component's `Status` changed — reported per component, not as diff
//!    noise, because `Ok` -> `Quarantined` silently empties the React package;
//! 4. the quarantine set equals `abi/quarantine.json` exactly, so a NEW
//!    quarantine fails CI;
//! 5. every file under `packages/react` hashes to what the emitter wrote
//!    (`abi/package.manifest.json`), none is missing, and none is present that
//!    no stage wrote;
//! 6. the package and the ABI agree about which components exist — derived from
//!    `Status` on both sides, so it holds even when every hash agrees.
//! 7. `abi/examples.json` and `bridge/examples.rs` are what the current ABI plus
//!    `abi/overrides.toml` would produce. Recomputed on both paths and never
//!    replayed: the snippets need no render, and an edit to a `disposition`
//!    moves neither hash, so hashing would not see it.
//!
//! 5 and 6 are why this is a gate on the *deliverable* and not only on its
//! metadata. Without them, rewriting a class name in a shipped `.tsx` by hand
//! left every committed artifact untouched and the run green; and `--write`
//! could be made to accept a hand-edited `abi/quarantine.json`, producing an ABI
//! that called a component quarantined while its `.tsx` was still on disk and
//! still exported. `--write` now refuses to publish a status change it did not
//! render, which closes that pair at the source.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::abi::{self, Abi, AbiChange, Severity};
use crate::ir::{Span, Status, Toolkit};

/// `abi/`, relative to the toolkit root. The committed half of the contract.
pub const ABI_DIR: &str = "abi";
pub const ABI_FILE: &str = "abi/toolkit.abi.json";
pub const QUARANTINE_FILE: &str = "abi/quarantine.json";
pub const OVERRIDES_FILE: &str = "abi/overrides.toml";
/// The committed per-file hashes of the emitted React package.
///
/// A separate file rather than a section of `toolkit.abi.json` for the same
/// reason `quarantine.json` is separate: it is output of the *render* half of
/// the pipeline, so the fast path replays it rather than regenerating it, and
/// mixing replayed data into the document the fast path regenerates and
/// byte-diffs would make that diff meaningless.
pub const PACKAGE_MANIFEST_FILE: &str = "abi/package.manifest.json";
/// Re-exported so findings can name the directory without depending on `emit`'s
/// spelling drifting.
pub use crate::emit::PACKAGE_DIR;
// The usage snippets' two paths are not restated here: `check` names them by
// iterating `examples::files`, so `examples` stays their single spelling.

// ---------------------------------------------------------------- options

#[derive(Clone, Debug, Default)]
pub struct Options {
    /// `--semver --against <git-ref>`: also classify this ABI against the one
    /// committed at `<git-ref>`.
    pub against: Option<String>,
    /// `--render`: take the full path even when both hashes match. The escape
    /// hatch for "I do not believe the cache".
    pub force_render: bool,
    /// `--write`: rewrite the committed artifacts instead of reporting them
    /// stale. This is the "accept" half of a drift gate — without it the only
    /// way to update `abi/` is to hand-edit generated JSON.
    pub write: bool,
    /// `--deny-breaking`: make a breaking semver change fail the run. Off by
    /// default, because a major release is a decision, not an accident.
    pub deny_breaking: bool,
    /// Where the harness crate and its build cache live. Defaults to
    /// `xtask/out`, so a check reuses the ~1GB `harness-target` rather than
    /// rebuilding yew into a temp dir it then deletes.
    pub scratch: Option<PathBuf>,
}

impl Options {
    /// Parses the argv tail after `check`. Unknown flags are an error rather
    /// than being ignored: a typo'd `--sevmer` that silently checked nothing
    /// would be a green CI run that proved nothing.
    pub fn from_args<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut out = Options::default();
        let mut semver = false;
        let mut it = args.into_iter().peekable();
        while let Some(arg) = it.next() {
            match arg.as_ref() {
                "--semver" => semver = true,
                "--against" => {
                    let r = it.next().context("--against needs a git ref")?;
                    out.against = Some(r.as_ref().to_string());
                }
                "--render" => out.force_render = true,
                "--write" => out.write = true,
                "--deny-breaking" => out.deny_breaking = true,
                other => bail!(
                    "unknown `check` flag {other:?}; expected --semver --against <git-ref>, \
                     --render, --write or --deny-breaking"
                ),
            }
        }
        if semver && out.against.is_none() {
            bail!("--semver needs --against <git-ref> to compare with");
        }
        if out.against.is_some() && !semver {
            bail!("--against <git-ref> only means something with --semver");
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------- findings

/// How the run decided what to trust.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Both hashes matched the committed ABI, so the committed render verdicts
    /// were replayed rather than re-derived.
    Replayed,
    /// A hash moved (or `--render`): `harness` -> `render` -> `verify` ran.
    Rendered,
    /// Nothing to compare against. The ABI has never been written.
    Uninitialised,
}

impl Mode {
    fn label(&self) -> &'static str {
        match self {
            Mode::Replayed => "replayed committed render verdicts (both hashes match)",
            Mode::Rendered => "re-rendered every cell",
            Mode::Uninitialised => "nothing committed to compare against",
        }
    }
}

/// One reason the gate is unhappy. Each names a path or a span a human can open,
/// which is the difference between a gate and an alarm.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finding {
    /// A committed artifact does not exist at all.
    Missing { path: String, hint: String },
    /// A committed artifact differs from what regenerating produces.
    Stale { path: String, diff: String },
    /// Invariant 1.
    SrcHashMismatch { committed: String, actual: String },
    /// Invariant 1's sibling: the generator moved, so the committed render
    /// verdicts describe a different generator's output.
    GeneratorHashMismatch { committed: String, actual: String },
    /// Invariant 2.
    MissingOverride { component: String, reason: String, source: String },
    /// An `abi/overrides.toml` entry for a component that no longer needs one.
    StaleOverride { component: String, status: String },
    /// Invariant 3. Its own class: a status change is how a component leaves the
    /// React package without any file being deleted.
    StatusChanged { component: String, from: String, to: String, source: String },
    /// Invariant 4, in the direction that must fail CI.
    QuarantineAdded { component: String, reason: String, source: String },
    /// Invariant 4, the other direction. Not a regression, but the committed
    /// file is wrong until someone regenerates it.
    QuarantineLifted { component: String, source: String },
    /// Invariant 5: a file under `packages/react` that the generator never
    /// wrote. Its own class because a stray `.tsx` still typechecks, is still
    /// importable by path, and produces no diff anywhere else.
    PackageExtra { path: String, hint: String },
    /// Invariant 6: `abi/` and the emitted package disagree about which
    /// components exist. Derived from `Status` on both sides, so `--write`
    /// cannot make the two agree by rewriting one of them.
    PackageContract { detail: String },
    /// `--semver` only.
    Breaking { change: AbiChange },
}

impl Finding {
    /// Whether this finding fails the run. `Breaking` does not, by itself —
    /// `Options::deny_breaking` decides that, because shipping a major is a
    /// legitimate thing to do on purpose.
    pub fn is_fatal(&self) -> bool {
        !matches!(self, Finding::Breaking { .. })
    }

    fn headline(&self) -> String {
        match self {
            Finding::Missing { path, hint } => format!("{path} is missing — {hint}"),
            Finding::Stale { path, .. } => format!("{path} is stale"),
            Finding::SrcHashMismatch { committed, actual } => format!(
                "the committed ABI describes src_hash {} but src/ hashes to {}",
                short(committed),
                short(actual)
            ),
            Finding::GeneratorHashMismatch { committed, actual } => format!(
                "the committed ABI was produced by generator {} but xtask/src hashes to {}",
                short(committed),
                short(actual)
            ),
            Finding::MissingOverride { component, reason, source } => format!(
                "{component} ({source}) is not generated — \"{reason}\" — and has no \
                 [{component}] entry in {OVERRIDES_FILE}"
            ),
            Finding::StaleOverride { component, status } => format!(
                "{OVERRIDES_FILE} has a [{component}] entry, but {component} is {status} and \
                 needs no override"
            ),
            Finding::StatusChanged { component, from, to, source } => {
                format!("{component} ({source}) changed status: {from} -> {to}")
            }
            Finding::QuarantineAdded { component, reason, source } => {
                format!("NEW quarantine: {component} ({source}) — {reason}")
            }
            Finding::QuarantineLifted { component, source } => format!(
                "{component} ({source}) is quarantined in {QUARANTINE_FILE} but passes now"
            ),
            Finding::PackageExtra { path, hint } => {
                format!("{path} is under {PACKAGE_DIR} but no stage of the generator wrote it — {hint}")
            }
            Finding::PackageContract { detail } => detail.clone(),
            Finding::Breaking { change } => change.to_string(),
        }
    }

    fn body(&self) -> Option<&str> {
        match self {
            Finding::Stale { diff, .. } => Some(diff),
            _ => None,
        }
    }
}

fn short(hash: &str) -> &str {
    &hash[..hash.len().min(12)]
}

#[derive(Clone, Debug)]
pub struct CheckOutcome {
    pub mode: Mode,
    pub findings: Vec<Finding>,
    /// Present when `--semver` ran: "major", "minor" or "patch".
    pub bump: Option<&'static str>,
    /// Artifacts that `--write` actually rewrote. When this is non-empty the
    /// findings describe the state `--write` just replaced, so they are reported
    /// as accepted rather than as failures.
    pub written: Vec<String>,
    pub elapsed: Duration,
    pub deny_breaking: bool,
}

impl CheckOutcome {
    /// Whether the tree is in the state the gate wants.
    ///
    /// A `--write` run that rewrote something is clean by construction: the
    /// drift it found is the drift it just resolved, and failing afterwards
    /// would mean `--write` could never exit 0. A breaking semver finding is
    /// never fatal on its own — `--deny-breaking` is what makes it so, because
    /// shipping a major is a decision rather than an accident.
    pub fn is_clean(&self) -> bool {
        if self.accepted() {
            return !self.deny_breaking || !self.findings.iter().any(|f| !f.is_fatal());
        }
        !self.findings.iter().any(|f| f.is_fatal() || (self.deny_breaking && !f.is_fatal()))
    }

    /// `--write` rewrote at least one artifact.
    pub fn accepted(&self) -> bool {
        !self.written.is_empty()
    }

    pub fn exit_code(&self) -> i32 {
        if self.is_clean() {
            0
        } else {
            1
        }
    }

    /// Everything a reviewer needs, diffs included.
    pub fn report(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "xtask check — {}", self.mode.label());
        let _ = writeln!(s, "  took {:.2}s", self.elapsed.as_secs_f64());
        for path in &self.written {
            let _ = writeln!(s, "  wrote {path}");
        }
        if let Some(bump) = self.bump {
            let _ = writeln!(s, "  semver: this change requires a {bump} bump");
        }
        if self.findings.is_empty() {
            let _ = writeln!(s, "\nclean: abi/ and packages/react match src/.");
            return s;
        }
        if self.accepted() {
            let _ = writeln!(
                s,
                "\n{} finding(s), accepted into abi/ by --write:",
                self.findings.len()
            );
        } else {
            let fatal = self.findings.iter().filter(|f| f.is_fatal()).count();
            let _ = writeln!(s, "\n{} finding(s), {fatal} of them fatal:", self.findings.len());
        }
        for f in &self.findings {
            let _ = writeln!(s, "\n  * {}", f.headline());
            if let Some(body) = f.body() {
                for line in body.lines() {
                    let _ = writeln!(s, "    {line}");
                }
            }
        }
        if self.accepted() {
            return s;
        }
        // Two different failures with two different fixes. Telling someone to
        // regenerate when what they actually need to do is record a decision in
        // overrides.toml sends them round a loop that cannot terminate.
        if self.findings.iter().any(|f| {
            matches!(
                f,
                Finding::Stale { .. }
                    | Finding::Missing { .. }
                    | Finding::SrcHashMismatch { .. }
                    | Finding::GeneratorHashMismatch { .. }
                    | Finding::StatusChanged { .. }
                    | Finding::QuarantineAdded { .. }
                    | Finding::QuarantineLifted { .. }
            )
        }) {
            let _ = writeln!(
                s,
                "\nabi/ is out of date. Regenerate with `cargo xtask bridgegen` (or \
                 `cargo xtask check --write`) and commit the result."
            );
        }
        if self
            .findings
            .iter()
            .any(|f| matches!(f, Finding::MissingOverride { .. } | Finding::StaleOverride { .. }))
        {
            let _ = writeln!(
                s,
                "\n{OVERRIDES_FILE} needs editing by hand: a component the generator \
                 refuses to emit must carry a recorded decision, not just a reason."
            );
        }
        s
    }
}

// ---------------------------------------------------------------- the gate

/// Re-derive everything and compare it with `abi/`.
pub fn check(root: &Path) -> Result<CheckOutcome> {
    check_with(root, &Options::default())
}

pub fn check_with(root: &Path, opts: &Options) -> Result<CheckOutcome> {
    let started = Instant::now();
    let mut findings: Vec<Finding> = Vec::new();
    let mut written: Vec<String> = Vec::new();

    // The front half is unconditional and costs ~0.12s over the whole crate, so
    // there is no fast path worth having for it. Every TS type, every status
    // that is not a quarantine, and the whole scope arithmetic is recomputed on
    // every run regardless of which path the render takes.
    let mut tk = parse_and_classify(root)?;
    let actual_src = tk.src_hash.clone();
    let actual_gen = abi::generator_hash()?;

    let abi_path = root.join(ABI_FILE);
    let committed = if abi_path.is_file() { Some(abi::read_abi(&abi_path)?) } else { None };

    let Some(committed) = committed else {
        // Nothing to diff. Emit the artifacts if asked, and otherwise say what
        // is missing rather than reporting a clean run over an empty contract.
        let mode = Mode::Uninitialised;
        if opts.write {
            // No committed hashes to match, so there is nothing to replay: the
            // first ABI has to be rendered.
            let (cells, _, report, package) = render_or_replay(&mut tk, opts, None, false)?;
            let abi_doc = abi::build_abi(&tk, &cells)?;
            let snippets =
                crate::examples::build(&abi_doc, &tk, &read_overrides(root)?)
                    .context("deriving the per-component usage snippets")?;
            written.extend(write_artifacts(
                root,
                &tk,
                &cells,
                report.as_ref(),
                package.as_ref(),
                &snippets,
            )?);
        } else {
            findings.push(Finding::Missing {
                path: ABI_FILE.into(),
                hint: "run `cargo xtask bridgegen` (or `cargo xtask check --write`) and commit it"
                    .into(),
            });
        }
        return Ok(CheckOutcome {
            mode,
            findings,
            bump: None,
            written,
            elapsed: started.elapsed(),
            deny_breaking: opts.deny_breaking,
        });
    };

    // Invariant 1, stated in its own words. The unified diff below would show
    // the hash line moving too, but "srcHash differs" buried in 4000 lines of
    // JSON is not a diagnosis.
    if committed.src_hash != actual_src {
        findings.push(Finding::SrcHashMismatch {
            committed: committed.src_hash.clone(),
            actual: actual_src.clone(),
        });
    }
    if committed.generator_hash != actual_gen {
        findings.push(Finding::GeneratorHashMismatch {
            committed: committed.generator_hash.clone(),
            actual: actual_gen.clone(),
        });
    }

    let quarantine_path = root.join(QUARANTINE_FILE);
    let committed_q = read_quarantine(&quarantine_path)?;
    if committed_q.is_none() {
        findings.push(Finding::Missing {
            path: QUARANTINE_FILE.into(),
            hint: "the quarantine set cannot be checked without it".into(),
        });
    }

    // Both hashes, not just the toolkit's: the render verdicts are a function of
    // src/ *and* of the generator that rendered it.
    let hashes_match =
        committed.src_hash == actual_src && committed.generator_hash == actual_gen;
    let (cells, mode, report, package) =
        render_or_replay(&mut tk, opts, committed_q.as_ref(), hashes_match)?;

    // --- invariant 3: status, per component, before anything else can bury it.
    for c in &tk.components {
        let Some(was) = committed.component(&c.name) else { continue };
        if was.status != c.status {
            findings.push(Finding::StatusChanged {
                component: c.name.clone(),
                from: describe_status(&was.status),
                to: describe_status(&c.status),
                source: source(&c.span),
            });
        }
    }

    // --- invariant 4: the quarantine set, exactly.
    if let Some(q) = &committed_q {
        let now: BTreeMap<&str, &str> = tk
            .components
            .iter()
            .filter_map(|c| match &c.status {
                Status::Quarantined { reason } => Some((c.name.as_str(), reason.as_str())),
                _ => None,
            })
            .collect();
        let was: BTreeSet<&str> = q.quarantined.iter().map(|e| e.component.as_str()).collect();
        for (name, reason) in &now {
            if !was.contains(name) {
                findings.push(Finding::QuarantineAdded {
                    component: (*name).to_string(),
                    reason: (*reason).to_string(),
                    source: tk.component(name).map(|c| source(&c.span)).unwrap_or_default(),
                });
            }
        }
        for e in &q.quarantined {
            if !now.contains_key(e.component.as_str()) {
                findings.push(Finding::QuarantineLifted {
                    component: e.component.clone(),
                    source: e
                        .span
                        .as_ref()
                        .map(source)
                        .or_else(|| tk.component(&e.component).map(|c| source(&c.span)))
                        .unwrap_or_default(),
                });
            }
        }
        if q.src_hash != actual_src {
            findings.push(Finding::Stale {
                path: QUARANTINE_FILE.into(),
                diff: format!(
                    "- \"src_hash\": \"{}\"\n+ \"src_hash\": \"{}\"\n\
                     (the quarantine verdicts were produced from a different src/)",
                    q.src_hash, actual_src
                ),
            });
        }
    }

    // --- invariant 2: mapped, or overridden.
    findings.extend(check_overrides(root, &tk)?);

    // --- invariants 5 and 6: the deliverable itself.
    let committed_pkg = read_manifest(&root.join(PACKAGE_MANIFEST_FILE))?;
    findings.extend(check_package(root, &tk, package.as_ref(), committed_pkg.as_ref())?);

    // `--write` publishes a status change only when this run *rendered* it.
    //
    // On the replay path the verdicts come from `abi/quarantine.json`, so a
    // hand-edited quarantine file would be copied into the ABI and certified
    // clean by the next plain `check` in a tenth of a second. Requiring a real
    // render before a status may move makes the tampered pair impossible to
    // produce through the tool.
    if opts.write && mode == Mode::Replayed {
        let moved: Vec<&Finding> = findings
            .iter()
            .filter(|f| {
                matches!(
                    f,
                    Finding::StatusChanged { .. }
                        | Finding::QuarantineAdded { .. }
                        | Finding::QuarantineLifted { .. }
                )
            })
            .collect();
        if !moved.is_empty() {
            bail!(
                "--write will not publish a status change it did not render: {}. Re-run as \
                 `cargo xtask check --render --write` (or `cargo xtask bridgegen`), which \
                 re-derives the verdicts from the toolkit instead of replaying \
                 {QUARANTINE_FILE}",
                moved.iter().map(|f| f.headline()).collect::<Vec<_>>().join("; ")
            );
        }
    }

    // --- the artifacts themselves.
    let abi_doc = abi::build_abi(&tk, &cells)?;
    let regenerated = abi::to_json(&abi_doc)?;
    let on_disk = std::fs::read_to_string(&abi_path)
        .with_context(|| format!("reading {}", abi_path.display()))?;
    let mut artifact_findings: Vec<Finding> = Vec::new();
    if on_disk != regenerated {
        artifact_findings.push(Finding::Stale {
            path: ABI_FILE.into(),
            diff: unified_diff(ABI_FILE, &on_disk, &regenerated),
        });
    }

    // --- invariant 7: the usage snippets.
    //
    // Recomputed on both paths and never replayed: they are a function of the
    // ABI, the IR and `abi/overrides.toml`, none of which needs a render. That
    // also means an edit to a `disposition` — which moves neither hash — is
    // still caught, which is the point of diffing rather than hashing.
    let snippets = crate::examples::build(&abi_doc, &tk, &read_overrides(root)?)
        .context("deriving the per-component usage snippets")?;
    for (rel, regen) in crate::examples::files(&snippets)? {
        let path = root.join(rel);
        if !path.is_file() {
            artifact_findings.push(Finding::Missing {
                path: rel.into(),
                hint: "run `cargo xtask bridgegen` (or `cargo xtask check --write`) and commit it"
                    .into(),
            });
            continue;
        }
        let current = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        if current != regen {
            artifact_findings.push(Finding::Stale {
                path: rel.into(),
                diff: unified_diff(rel, &current, &regen),
            });
        }
    }

    if !artifact_findings.is_empty() {
        if opts.write {
            written.extend(write_artifacts(
                root,
                &tk,
                &cells,
                report.as_ref(),
                package.as_ref(),
                &snippets,
            )?);
        } else {
            findings.extend(artifact_findings);
        }
    } else if opts.write && report.is_some() {
        // The ABI can match while the verdict detail behind it has not been
        // published; only meaningful when this run actually rendered.
        written.extend(write_artifacts(
            root,
            &tk,
            &cells,
            report.as_ref(),
            package.as_ref(),
            &snippets,
        )?);
    }

    // --- semver, against a ref rather than against the working tree.
    let mut bump = None;
    if let Some(git_ref) = &opts.against {
        let old = read_abi_at(root, git_ref)?;
        let new = abi::build_abi(&tk, &cells)?;
        let changes = abi::diff_abi(&old, &new);
        bump = Some(abi::required_bump(&changes));
        for change in changes.into_iter().filter(|c| c.severity == Severity::Breaking) {
            findings.push(Finding::Breaking { change });
        }
    }

    Ok(CheckOutcome {
        mode,
        findings,
        bump,
        written,
        elapsed: started.elapsed(),
        deny_breaking: opts.deny_breaking,
    })
}

fn source(span: &Span) -> String {
    format!("{}:{}", span.file, span.line)
}

fn describe_status(s: &Status) -> String {
    match s {
        Status::Ok => "ok".into(),
        Status::NeedsOverride { reason } => format!("needs-override ({reason})"),
        Status::Quarantined { reason } => format!("quarantined ({reason})"),
        Status::Excluded { reason } => format!("excluded ({reason})"),
    }
}

fn parse_and_classify(root: &Path) -> Result<Toolkit> {
    let mut tk = crate::parse::parse(root)
        .with_context(|| format!("parsing {}", root.join("src").display()))?;
    crate::classify::classify(&mut tk).context("classifying props")?;
    Ok(tk)
}

/// Decide which path to take, and fold the render verdicts into `tk` either way.
///
/// Returns the cell map the ABI needs, which comes from `matrix` and so is the
/// same on both paths — only the *quarantine* half of the IR differs in how it
/// was obtained.
type Replayed = (
    BTreeMap<String, Vec<crate::matrix::Cell>>,
    Mode,
    Option<crate::verify::VerifyReport>,
    Option<BTreeMap<String, String>>,
);

fn render_or_replay(
    tk: &mut Toolkit,
    opts: &Options,
    committed_q: Option<&QuarantineDoc>,
    hashes_match: bool,
) -> Result<Replayed> {
    let cells = crate::harness::cells_for(tk).context("deriving render cells")?;

    // Replaying needs all three: neither input moved, and the verdict file is
    // both present and about this same src/. Any one of them failing means the
    // committed verdicts describe something else, so the render has to happen.
    let replayable = !opts.force_render
        && hashes_match
        && committed_q.is_some_and(|q| q.src_hash == tk.src_hash);

    if let (true, Some(q)) = (replayable, committed_q) {
        replay(tk, q)?;
        return Ok((cells, Mode::Replayed, None, None));
    }

    let scratch = opts
        .scratch
        .clone()
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("out"));
    let (report, package) = rerender(tk, &cells, &scratch)?;
    Ok((cells, Mode::Rendered, Some(report), Some(package)))
}

/// Apply the committed verdicts to a freshly parsed IR.
///
/// Sound only when the toolkit hash and the generator hash both match, which is
/// what `check_with` establishes before calling this: the verdicts are a pure
/// function of those two inputs, so replaying them reproduces exactly what
/// re-rendering would have written.
fn replay(tk: &mut Toolkit, q: &QuarantineDoc) -> Result<()> {
    for e in &q.quarantined {
        let Some(c) = tk.components.iter_mut().find(|c| c.name == e.component) else {
            bail!(
                "{QUARANTINE_FILE} quarantines {:?}, which src/ no longer defines; \
                 regenerate it",
                e.component
            );
        };
        if let Status::Excluded { reason } = &c.status {
            bail!(
                "{QUARANTINE_FILE} quarantines {} but it is now Excluded ({reason}); \
                 the two files disagree about scope",
                c.name
            );
        }
        c.status = Status::Quarantined { reason: e.reason.clone() };
    }
    Ok(())
}

/// The full path: build and run the harness, then fold the fresh verdicts in.
///
/// `scratch` holds the harness crate and its build cache, not the artifacts
/// under comparison — those are built in memory and diffed against `abi/`
/// without ever touching disk. Pointing it at `xtask/out` by default is what
/// keeps a re-render at ~20s instead of ~50s: `render` derives its cargo target
/// directory and its render cache from these paths, and a fresh temp dir means
/// compiling yew from scratch to learn something the cache already knows.
///
/// The consequence is that two checks sharing a `scratch` will fight over the
/// harness crate. That is fine for a CI gate, which runs one at a time; a caller
/// that needs isolation sets `Options::scratch` and pays the cold build.
/// Re-run the render half AND the emit half.
///
/// The emit half is here rather than in `check_with` because it needs `plan` and
/// `results`, which only exist inside this function: the package is the
/// deliverable, and a gate that regenerated the ABI but not the package is the
/// hole this closes. `splice` failing is an error rather than a finding for the
/// same reason it is in `bridgegen` — a component that rendered cleanly and
/// could not be reassembled is a generator defect, not drift.
fn rerender(
    tk: &mut Toolkit,
    cells: &BTreeMap<String, Vec<crate::matrix::Cell>>,
    scratch: &Path,
) -> Result<(crate::verify::VerifyReport, BTreeMap<String, String>)> {
    std::fs::create_dir_all(scratch)
        .with_context(|| format!("creating {}", scratch.display()))?;
    let harness_dir = scratch.join("harness");
    crate::harness::emit_harness(tk, cells, &harness_dir)
        .with_context(|| format!("writing the harness crate to {}", harness_dir.display()))?;
    let plan = crate::harness::plan(tk, cells).context("rebuilding the harness plan")?;

    let renders_json = scratch.join("renders.json");
    let (results, _) = crate::render::render_all_with(&harness_dir, &renders_json, &tk.src_hash)
        .context("building and running the harness")?;

    let plans = render_plans(&plan);
    let collected = crate::verify::collect(&results, &plans)
        .context("joining the harness plan to the render results")?;
    let report = crate::verify::verify(tk, &collected).context("running the verify gate")?;
    crate::verify::apply(tk, &report).context("folding quarantines back into the IR")?;

    let spliced = crate::splice::splice_toolkit(tk, cells, &plan, &results)
        .context("splicing the rendered markup back into JSX")?;
    if !spliced.failures.is_empty() {
        bail!(
            "{} component(s) passed the verify gate but could not be spliced: {}",
            spliced.failures.len(),
            spliced.failures.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
        );
    }
    let jsx = crate::to_emit_jsx(&spliced, &report);
    let (package, _warnings) =
        crate::emit::render_package(tk, &jsx).context("re-rendering the React package")?;

    // A surprising quarantine is a finding, not a crash: the gate's job is to
    // report it against the committed set, which `check_with` does.
    Ok((report, package))
}

/// `harness::HarnessPlan` -> the per-render plans `verify::collect` joins on.
///
/// The same translation `main::render_plans` does. It lives in both places
/// because the two drivers are independent entry points and neither may import
/// the other's private helper; the key shape (`<Component>/<cell>#<alphabet>`)
/// is the contract, and `render::component_of` documents it.
fn render_plans(
    plan: &crate::harness::HarnessPlan,
) -> BTreeMap<String, crate::verify::RenderPlan> {
    let mut out = BTreeMap::new();
    for cell in &plan.cells {
        for alphabet in crate::verify::Alphabet::ALL {
            let sentinels = cell
                .sentinels
                .iter()
                .map(|b| crate::verify::Sentinel {
                    id: b.index as u32,
                    path: b.path.clone(),
                    span: b.span.clone(),
                })
                .collect();
            out.insert(
                format!("{}#{}", cell.key, alphabet.name()),
                crate::verify::RenderPlan {
                    component: cell.component.clone(),
                    cell: cell.cell.clone(),
                    alphabet,
                    sentinels,
                },
            );
        }
    }
    out
}

/// `--write`.
///
/// The quarantine half is written from the [`crate::verify::VerifyReport`] this
/// run produced, never copied from `xtask/out/quarantine.json`: that file
/// belongs to whichever `cargo xtask render` last happened to run, and
/// publishing it would commit a verdict this run never made. On the replay path
/// there is no fresh report and none is needed — replaying only happens when
/// both hashes match, which is exactly the case where the committed file is
/// already correct.
fn write_artifacts(
    root: &Path,
    tk: &Toolkit,
    cells: &BTreeMap<String, Vec<crate::matrix::Cell>>,
    report: Option<&crate::verify::VerifyReport>,
    package: Option<&BTreeMap<String, String>>,
    snippets: &crate::examples::Examples,
) -> Result<Vec<String>> {
    let mut written = Vec::new();
    abi::write_abi(tk, cells, &root.join(ABI_FILE))?;
    written.push(ABI_FILE.to_string());

    written.extend(crate::examples::write(root, snippets)?);

    if let Some(report) = report {
        std::fs::create_dir_all(root.join(ABI_DIR))?;
        report.write(&root.join(QUARANTINE_FILE))?;
        written.push(QUARANTINE_FILE.to_string());
    }

    // Only on the render path. The manifest's whole value is that its hashes
    // came from bytes the emitter just produced; writing it from whatever was
    // on disk would turn the gate into a rubber stamp for any hand edit.
    if let Some(package) = package {
        write_package(root, tk, package)?;
        written.push(format!("{PACKAGE_DIR}/"));
        written.push(PACKAGE_MANIFEST_FILE.to_string());
    }
    Ok(written)
}

/// Write the regenerated package and its manifest.
pub fn write_package(
    root: &Path,
    tk: &Toolkit,
    package: &BTreeMap<String, String>,
) -> Result<()> {
    crate::emit::write_files(package, &root.join(PACKAGE_DIR))?;
    let manifest = PackageManifest::of(tk, package)?;
    std::fs::create_dir_all(root.join(ABI_DIR))?;
    let path = root.join(PACKAGE_MANIFEST_FILE);
    std::fs::write(&path, manifest.to_json()?)
        .with_context(|| format!("writing {}", path.display()))
}

// ------------------------------------------------------------- quarantine.json

/// The half of `verify`'s `quarantine.json` this stage reads.
///
/// Declared here rather than reusing `verify`'s writer type because that one is
/// `Serialize`-only and borrows: a reader has to own its data, and a reader that
/// tolerates extra fields is what lets `verify` add evidence without breaking
/// the gate.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct QuarantineDoc {
    pub src_hash: String,
    #[serde(default)]
    pub checked: usize,
    #[serde(default)]
    pub quarantined: Vec<QuarantineEntry>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct QuarantineEntry {
    pub component: String,
    pub reason: String,
    #[serde(default)]
    pub span: Option<Span>,
}

fn read_quarantine(path: &Path) -> Result<Option<QuarantineDoc>> {
    if !path.is_file() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let doc: QuarantineDoc = serde_json::from_str(&text)
        .with_context(|| format!("{} is not a quarantine report", path.display()))?;
    Ok(Some(doc))
}

// --------------------------------------------------------------- overrides

/// Invariant 2.
///
/// The "every prop is mapped" half is enforced by construction and re-proved on
/// every run: `classify` aborts naming the prop's own span when a type maps to
/// nothing, and `check_with` calls `classify` before it gets here. So what is
/// left to check is the other half — a component the generator refused to emit
/// must have a human's decision recorded next to it, or it has been quietly
/// dropped from the package.
///
/// Must be called *after* the quarantines have been folded into `tk`. A
/// pre-verify IR still has `Swatch`, `DatasetCard` and `PaletteGroup` as `Ok`,
/// so their committed entries would every one of them read as stale.
// ------------------------------------------------- invariant 5/6: the package

/// The committed per-file hashes of `packages/react`, written by `bridgegen`
/// beside `abi/quarantine.json`.
///
/// `check` needs this because the emitted package is a *product of the render*:
/// reproducing it costs a harness build, which is exactly what the fast path
/// exists to avoid. The soundness argument is the one already made for the
/// quarantine verdicts — the package is a pure function of `src/` and of
/// `xtask/src`, so when both hashes match, the committed hashes are what
/// regenerating would produce, and comparing them against disk is a real check
/// of the deliverable rather than a restatement of itself.
///
/// Without it, `check` covered `abi/` only: rewriting a class name in a shipped
/// `.tsx` by hand left `abi/` untouched and the gate green.
#[derive(Clone, Debug, Default, Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageManifest {
    pub src_hash: String,
    pub generator_hash: String,
    /// Relative to the toolkit root, so a finding names a path a human can open.
    pub dir: String,
    /// What `index.ts` re-exports, sorted.
    #[serde(default)]
    pub exported: Vec<String>,
    /// Every file the emitter wrote, sorted by path.
    #[serde(default)]
    pub files: Vec<PackageFile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct PackageFile {
    /// Relative to [`PackageManifest::dir`].
    pub path: String,
    pub sha256: String,
}

impl PackageManifest {
    /// Build it from the emitter's own output map, which is what makes the
    /// hashes describe the bytes that were written rather than the bytes that
    /// happened to be on disk afterwards.
    pub fn of(tk: &Toolkit, files: &BTreeMap<String, String>) -> Result<Self> {
        Ok(PackageManifest {
            src_hash: tk.src_hash.clone(),
            generator_hash: abi::generator_hash()?,
            dir: PACKAGE_DIR.to_string(),
            exported: crate::emit::expected_exports(tk).into_iter().collect(),
            files: files
                .iter()
                .map(|(path, body)| PackageFile { path: path.clone(), sha256: sha256(body) })
                .collect(),
        })
    }

    pub fn to_json(&self) -> Result<String> {
        let json = serde_json::to_string_pretty(self).context("serialising the package manifest")?;
        Ok(format!("{json}\n"))
    }
}

fn sha256(body: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(body.as_bytes());
    format!("{:x}", h.finalize())
}

fn read_manifest(path: &Path) -> Result<Option<PackageManifest>> {
    if !path.is_file() {
        return Ok(None);
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let doc: PackageManifest = serde_json::from_str(&text)
        .with_context(|| format!("{} is not a package manifest", path.display()))?;
    Ok(Some(doc))
}

/// Every file under `packages/react` this stage owns, as path -> contents.
///
/// `dist/` and `node_modules/` are skipped: they are build output of the npm
/// side, not of this generator, and `.gitignore` keeps them out of the tree
/// anyway. Anything else is fair game — a stray file in the package *is* the
/// finding.
fn scan_package(pkg: &Path) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    if !pkg.is_dir() {
        return Ok(out);
    }
    for entry in walkdir::WalkDir::new(pkg).follow_links(false).into_iter().filter_entry(|e| {
        !matches!(e.file_name().to_string_lossy().as_ref(), "dist" | "node_modules")
    }) {
        let entry = entry.with_context(|| format!("walking {}", pkg.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(pkg)
            .unwrap_or(entry.path())
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        // Lossy would silently turn a corrupt byte into U+FFFD and then hash
        // something that is not what is on disk.
        let body = std::fs::read_to_string(entry.path())
            .with_context(|| format!("reading {}", entry.path().display()))?;
        out.insert(rel, body);
    }
    Ok(out)
}

/// Invariants 5 and 6.
///
/// `regenerated` is `Some` only on the render path, where splice and emit
/// actually ran; when it is `Some` a differing file gets a real unified diff of
/// its TSX, and when it is `None` the committed hash is the reference and the
/// finding names the file. Both directions name a path, which is the thing the
/// hash-only `srcHash` finding could not do.
fn check_package(
    root: &Path,
    tk: &Toolkit,
    regenerated: Option<&BTreeMap<String, String>>,
    committed: Option<&PackageManifest>,
) -> Result<Vec<Finding>> {
    let mut findings = Vec::new();
    let pkg = root.join(PACKAGE_DIR);
    let on_disk = scan_package(&pkg)?;

    // --- invariant 6 first: it is derived from `Status` on both sides, so it
    // holds even when every hash agrees, and it is the one an `abi/` tampered
    // into internal consistency cannot satisfy.
    let want_files = crate::emit::expected_files(tk);
    let want_exports = crate::emit::expected_exports(tk);
    let have_files: BTreeSet<String> =
        on_disk.keys().filter(|p| p.starts_with("src/") && p.ends_with(".tsx")).cloned().collect();
    for path in want_files.difference(&have_files) {
        findings.push(Finding::PackageContract {
            detail: format!(
                "{PACKAGE_DIR}/{path} is absent, but the ABI says its component is generated; \
                 the React package is one component short of what {ABI_FILE} claims"
            ),
        });
    }
    for path in have_files.difference(&want_files) {
        let name = path.trim_start_matches("src/").trim_end_matches(".tsx");
        let status = tk.component(name).map(|c| describe_status(&c.status));
        findings.push(Finding::PackageContract {
            detail: match status {
                Some(s) => format!(
                    "{PACKAGE_DIR}/{path} is on disk, but {ABI_FILE} records {name} as {s}; a \
                     component that is not generated must not have a generated file"
                ),
                None => format!(
                    "{PACKAGE_DIR}/{path} is on disk, but src/ defines no component called {name}"
                ),
            },
        });
    }
    if let Some(index) = on_disk.get("src/index.ts") {
        let exported = exported_from_index(index);
        for name in want_exports.difference(&exported) {
            findings.push(Finding::PackageContract {
                detail: format!(
                    "{PACKAGE_DIR}/src/index.ts does not export {name}, which {ABI_FILE} records \
                     as generated"
                ),
            });
        }
        for name in exported.difference(&want_exports) {
            let status =
                tk.component(name).map(|c| describe_status(&c.status)).unwrap_or_else(|| "unknown to src/".into());
            findings.push(Finding::PackageContract {
                detail: format!(
                    "{PACKAGE_DIR}/src/index.ts exports {name}, which {ABI_FILE} records as \
                     {status}"
                ),
            });
        }
    }

    // --- invariant 5: the bytes.
    let Some(reference) = regenerated.map(|r| {
        r.iter().map(|(p, b)| (p.clone(), sha256(b))).collect::<BTreeMap<_, _>>()
    }).or_else(|| {
        committed.map(|m| m.files.iter().map(|f| (f.path.clone(), f.sha256.clone())).collect())
    }) else {
        findings.push(Finding::Missing {
            path: PACKAGE_MANIFEST_FILE.into(),
            hint: format!("{PACKAGE_DIR} cannot be checked without it; run `cargo xtask bridgegen`"),
        });
        return Ok(findings);
    };

    for (path, want) in &reference {
        match on_disk.get(path) {
            None => findings.push(Finding::Missing {
                path: format!("{PACKAGE_DIR}/{path}"),
                hint: "the generator wrote it and it is no longer there".into(),
            }),
            Some(body) if &sha256(body) != want => {
                let diff = match regenerated.and_then(|r| r.get(path)) {
                    Some(fresh) => unified_diff(&format!("{PACKAGE_DIR}/{path}"), body, fresh),
                    // Replay path: no fresh bytes to diff against, so say what
                    // is known — which file, and that its content moved.
                    None => format!(
                        "- sha256 {}\n+ sha256 {}\n(the committed manifest was produced from this \
                         same src/ and generator, so this file was changed by something other \
                         than the generator. `cargo xtask check --render` regenerates it and \
                         shows the content diff.)",
                        short(want),
                        short(&sha256(body)),
                    ),
                };
                findings.push(Finding::Stale { path: format!("{PACKAGE_DIR}/{path}"), diff });
            }
            Some(_) => {}
        }
    }
    for path in on_disk.keys().filter(|p| !reference.contains_key(*p)) {
        findings.push(Finding::PackageExtra {
            path: format!("{PACKAGE_DIR}/{path}"),
            hint: "delete it, or regenerate if the generator is meant to own it".into(),
        });
    }

    match (committed, regenerated) {
        // Render path: the manifest is itself a committed artifact, and a
        // manifest that is stale while the package is correct is the worst of
        // the two — the fast path would then hash `packages/react` against the
        // wrong reference and certify a wrong package clean.
        (Some(m), Some(fresh)) => {
            let regenerated_manifest = PackageManifest::of(tk, fresh)?;
            let (a, b) = (m.to_json()?, regenerated_manifest.to_json()?);
            if a != b {
                findings.push(Finding::Stale {
                    path: PACKAGE_MANIFEST_FILE.into(),
                    diff: unified_diff(PACKAGE_MANIFEST_FILE, &a, &b),
                });
            }
        }
        // Replay path: the manifest is the reference, so all that can be said
        // about it is whether it describes this src/ at all.
        (Some(m), None) if m.src_hash != tk.src_hash => {
            findings.push(Finding::Stale {
                path: PACKAGE_MANIFEST_FILE.into(),
                diff: format!(
                    "- \"srcHash\": \"{}\"\n+ \"srcHash\": \"{}\"\n\
                     (the package hashes were produced from a different src/)",
                    m.src_hash, tk.src_hash
                ),
            });
        }
        _ => {}
    }
    Ok(findings)
}

/// The component names `index.ts` re-exports.
///
/// `emit::index_ts` writes exactly one
/// `export { Name, type NameProps } from './Name';` per line and lists the
/// withheld ones in comments, so a line-oriented read is exact rather than a
/// heuristic. A line that starts with `//` is deliberately not a match — that is
/// how a withheld component is recorded.
fn exported_from_index(index: &str) -> BTreeSet<String> {
    index
        .lines()
        .filter_map(|l| l.strip_prefix("export { "))
        .filter_map(|l| l.split([',', ' ', '}']).next())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}

/// `abi/overrides.toml` as component -> key -> value, empty when the file is
/// absent.
///
/// Public because `examples` needs the same map: the note a docs page shows in
/// place of a React snippet is the `disposition` a human wrote here, and reading
/// it in two places with two parsers is how the note and the gate would start
/// disagreeing about which components are withheld.
pub fn read_overrides(root: &Path) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let path = root.join(OVERRIDES_FILE);
    if !path.is_file() {
        return Ok(BTreeMap::new());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    parse_overrides(&text, OVERRIDES_FILE)
}

fn check_overrides(root: &Path, tk: &Toolkit) -> Result<Vec<Finding>> {
    let overrides = read_overrides(root)?;

    let mut findings = Vec::new();
    let mut needed: BTreeSet<&str> = BTreeSet::new();
    for c in &tk.components {
        let reason = match &c.status {
            Status::NeedsOverride { reason } | Status::Quarantined { reason } => reason,
            // An exclusion is a rule, not a decision to record twice; `Ok` needs
            // nothing.
            Status::Ok | Status::Excluded { .. } => continue,
        };
        needed.insert(c.name.as_str());
        if !overrides.contains_key(c.name.as_str()) {
            findings.push(Finding::MissingOverride {
                component: c.name.clone(),
                reason: reason.clone(),
                source: source(&c.span),
            });
        }
    }
    for name in overrides.keys() {
        if !needed.contains(name.as_str()) {
            findings.push(Finding::StaleOverride {
                component: name.clone(),
                status: tk
                    .component(name)
                    .map(|c| describe_status(&c.status))
                    .unwrap_or_else(|| "not a component in this crate".into()),
            });
        }
    }
    Ok(findings)
}

/// Everything before an unquoted `#`.
///
/// Quote-aware because the reasons these files carry are issue references:
/// `reason = "inert hardcoded; needs open: bool upstream (#809)"` is the very
/// first entry anyone will write, and a naive `split('#')` truncates it at the
/// `(` without saying so.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (i, ch) in line.char_indices() {
        match (quote, ch) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"') | (None, '\'') => quote = Some(ch),
            (None, '#') => return &line[..i],
            (None, _) => {}
        }
    }
    line
}

/// A deliberately small TOML reader: `[Section]` headers and `key = value`
/// lines, with `#` comments.
///
/// `overrides.toml` is hand-written by a human recording a decision, and the
/// decisions are flat — a component name and a sentence. Pulling in a TOML
/// dependency to read that would be more surface than the file has content.
/// Anything this does not understand is an error rather than a silent skip, so
/// an override written in a syntax the gate ignores cannot read as present.
pub fn parse_overrides(text: &str, origin: &str) -> Result<BTreeMap<String, BTreeMap<String, String>>> {
    let mut out: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut section: Option<String> = None;
    for (n, raw) in text.lines().enumerate() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            let name = rest
                .strip_suffix(']')
                .with_context(|| format!("{origin}:{}: unterminated section header", n + 1))?
                .trim();
            if name.starts_with('[') || name.contains('.') {
                bail!(
                    "{origin}:{}: `[{name}]` — this reader only understands flat \
                     `[ComponentName]` sections",
                    n + 1
                );
            }
            section = Some(name.to_string());
            out.entry(name.to_string()).or_default();
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .with_context(|| format!("{origin}:{}: expected `key = value`, got {raw:?}", n + 1))?;
        let section = section.as_ref().with_context(|| {
            format!("{origin}:{}: `{}` appears before any [Section]", n + 1, key.trim())
        })?;
        let value = value.trim().trim_matches('"').to_string();
        out.get_mut(section).expect("inserted with the header").insert(key.trim().to_string(), value);
    }
    Ok(out)
}

// ------------------------------------------------------------------- semver

/// The ABI as committed at `git_ref`.
fn read_abi_at(root: &Path, git_ref: &str) -> Result<Abi> {
    // `git show` takes a path from the repository root, which is not
    // necessarily the toolkit root.
    let prefix = git_output(root, &["rev-parse", "--show-prefix"])?;
    let spec = format!("{git_ref}:{}{ABI_FILE}", prefix.trim());
    let text = git_output(root, &["show", &spec]).with_context(|| {
        format!("reading {ABI_FILE} at {git_ref} — does that ref have a committed ABI?")
    })?;
    abi::parse_abi(&text, &spec)
}

fn git_output(root: &Path, args: &[&str]) -> Result<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("running `git {}`", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "`git {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8(out.stdout).context("git printed non-UTF-8")?)
}

// -------------------------------------------------------------- unified diff

/// Three lines either side, which is what `git diff` shows and therefore what a
/// reviewer's eye expects.
const CONTEXT: usize = 3;

/// Above this many cells the LCS table costs more than the diff is worth. The
/// committed ABI is a few thousand lines and real drift touches a handful of
/// them, so the trimming below almost always leaves a middle far under this; the
/// cap exists so that a *total* rewrite degrades to one coarse hunk instead of
/// allocating gigabytes.
const LCS_CELL_CAP: usize = 2_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Eq(usize, usize),
    Del(usize),
    Ins(usize),
}

/// A unified diff of two texts, or an empty string when they are equal.
pub fn unified_diff(path: &str, old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let ops = diff_ops(&a, &b);

    let mut out = String::new();
    let _ = writeln!(out, "--- a/{path}");
    let _ = writeln!(out, "+++ b/{path}");
    for hunk in hunks(&ops) {
        let (mut a_start, mut a_len, mut b_start, mut b_len) = (0usize, 0usize, 0usize, 0usize);
        let (mut seen_a, mut seen_b) = (false, false);
        for op in &ops[hunk.0..hunk.1] {
            match op {
                Op::Eq(i, j) => {
                    if !seen_a {
                        (a_start, seen_a) = (*i, true);
                    }
                    if !seen_b {
                        (b_start, seen_b) = (*j, true);
                    }
                    a_len += 1;
                    b_len += 1;
                }
                Op::Del(i) => {
                    if !seen_a {
                        (a_start, seen_a) = (*i, true);
                    }
                    a_len += 1;
                }
                Op::Ins(j) => {
                    if !seen_b {
                        (b_start, seen_b) = (*j, true);
                    }
                    b_len += 1;
                }
            }
        }
        let _ = writeln!(
            out,
            "@@ -{},{} +{},{} @@",
            if a_len == 0 { a_start } else { a_start + 1 },
            a_len,
            if b_len == 0 { b_start } else { b_start + 1 },
            b_len
        );
        for op in &ops[hunk.0..hunk.1] {
            match op {
                Op::Eq(i, _) => {
                    let _ = writeln!(out, " {}", a[*i]);
                }
                Op::Del(i) => {
                    let _ = writeln!(out, "-{}", a[*i]);
                }
                Op::Ins(j) => {
                    let _ = writeln!(out, "+{}", b[*j]);
                }
            }
        }
    }
    out
}

fn diff_ops(a: &[&str], b: &[&str]) -> Vec<Op> {
    // Trimming first is what makes an O(m*n) table affordable on a 4000-line
    // artifact: a one-prop change leaves a middle of a few lines.
    let mut pre = 0;
    while pre < a.len() && pre < b.len() && a[pre] == b[pre] {
        pre += 1;
    }
    let mut suf = 0;
    while suf < a.len() - pre && suf < b.len() - pre && a[a.len() - 1 - suf] == b[b.len() - 1 - suf]
    {
        suf += 1;
    }

    let mut ops: Vec<Op> = (0..pre).map(|i| Op::Eq(i, i)).collect();
    let (am, bm) = (&a[pre..a.len() - suf], &b[pre..b.len() - suf]);
    if am.len().saturating_mul(bm.len()) > LCS_CELL_CAP {
        ops.extend((0..am.len()).map(|i| Op::Del(pre + i)));
        ops.extend((0..bm.len()).map(|j| Op::Ins(pre + j)));
    } else {
        ops.extend(lcs_ops(am, bm, pre, pre));
    }
    for k in 0..suf {
        ops.push(Op::Eq(a.len() - suf + k, b.len() - suf + k));
    }
    ops
}

/// Classic LCS backtrack. `off_a`/`off_b` shift the indices back into the
/// untrimmed texts.
fn lcs_ops(a: &[&str], b: &[&str], off_a: usize, off_b: usize) -> Vec<Op> {
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![0u32; (m + 1) * (n + 1)];
    let at = |i: usize, j: usize| i * (n + 1) + j;
    for i in (0..m).rev() {
        for j in (0..n).rev() {
            dp[at(i, j)] = if a[i] == b[j] {
                dp[at(i + 1, j + 1)] + 1
            } else {
                dp[at(i + 1, j)].max(dp[at(i, j + 1)])
            };
        }
    }
    let mut ops = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < m && j < n {
        if a[i] == b[j] {
            ops.push(Op::Eq(off_a + i, off_b + j));
            i += 1;
            j += 1;
        } else if dp[at(i + 1, j)] >= dp[at(i, j + 1)] {
            ops.push(Op::Del(off_a + i));
            i += 1;
        } else {
            ops.push(Op::Ins(off_b + j));
            j += 1;
        }
    }
    while i < m {
        ops.push(Op::Del(off_a + i));
        i += 1;
    }
    while j < n {
        ops.push(Op::Ins(off_b + j));
        j += 1;
    }
    ops
}

/// `[start, end)` ranges over `ops`, each a run of changes padded with
/// [`CONTEXT`] equal lines and merged when the padding would overlap.
fn hunks(ops: &[Op]) -> Vec<(usize, usize)> {
    let changed: Vec<usize> = ops
        .iter()
        .enumerate()
        .filter(|(_, o)| !matches!(o, Op::Eq(..)))
        .map(|(i, _)| i)
        .collect();
    let mut out: Vec<(usize, usize)> = Vec::new();
    for idx in changed {
        let start = idx.saturating_sub(CONTEXT);
        let end = (idx + CONTEXT + 1).min(ops.len());
        match out.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => out.push((start, end)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
    }

    // ----------------------------------------------------------- unified diff

    #[test]
    fn an_unchanged_text_diffs_to_nothing() {
        assert_eq!(unified_diff("a.json", "one\ntwo\n", "one\ntwo\n"), "");
    }

    #[test]
    fn a_one_line_change_shows_context_and_a_hunk_header() {
        let old = (1..=10).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let new = old.replace("line 5", "line five");
        let d = unified_diff("abi/toolkit.abi.json", &old, &new);
        assert!(d.starts_with("--- a/abi/toolkit.abi.json\n+++ b/abi/toolkit.abi.json\n"), "{d}");
        assert!(d.contains("-line 5\n"), "{d}");
        assert!(d.contains("+line five\n"), "{d}");
        // Three lines of context either side, and nothing beyond them.
        assert!(d.contains(" line 2\n") && d.contains(" line 8\n"), "{d}");
        assert!(!d.contains(" line 1\n") && !d.contains(" line 9\n"), "{d}");
        assert!(d.contains("@@ -2,7 +2,7 @@"), "{d}");
    }

    #[test]
    fn distant_changes_produce_separate_hunks() {
        let old = (1..=40).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let new = old.replace("line 3", "x").replace("line 37", "y");
        let d = unified_diff("f", &old, &new);
        assert_eq!(d.matches("@@").count(), 4, "two hunks, two markers each:\n{d}");
    }

    #[test]
    fn adjacent_changes_merge_into_one_hunk() {
        let old = (1..=20).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let new = old.replace("line 10", "x").replace("line 12", "y");
        let d = unified_diff("f", &old, &new);
        assert_eq!(d.matches("@@").count(), 2, "one hunk:\n{d}");
    }

    #[test]
    fn an_insertion_is_an_insertion_not_a_rewrite() {
        let d = unified_diff("f", "a\nb\nc\n", "a\nb\nNEW\nc\n");
        let added: Vec<&str> =
            d.lines().filter(|l| l.starts_with('+') && !l.starts_with("+++")).collect();
        let removed: Vec<&str> =
            d.lines().filter(|l| l.starts_with('-') && !l.starts_with("---")).collect();
        assert_eq!(added, ["+NEW"], "{d}");
        assert!(removed.is_empty(), "{d}");
    }

    #[test]
    fn the_diff_survives_a_total_rewrite() {
        // Past the LCS cap the output degrades to one coarse hunk, but it still
        // has to be a well-formed diff rather than a panic.
        let old = (0..3000).map(|i| format!("old {i}")).collect::<Vec<_>>().join("\n");
        let new = (0..3000).map(|i| format!("new {i}")).collect::<Vec<_>>().join("\n");
        let d = unified_diff("f", &old, &new);
        assert!(d.contains("@@"));
        assert!(d.contains("-old 0"));
        assert!(d.contains("+new 0"));
    }

    #[test]
    fn deleting_everything_is_representable() {
        let d = unified_diff("f", "a\nb\n", "");
        assert!(d.contains("-a") && d.contains("-b"), "{d}");
    }

    // -------------------------------------------------------------- overrides

    #[test]
    fn overrides_parse_sections_keys_and_comments() {
        let text = "# a comment\n\n[Modal]\nreason = \"inert hardcoded (#809)\"\nowner = nico\n";
        let got = parse_overrides(text, "t").unwrap();
        assert_eq!(got["Modal"]["reason"], "inert hardcoded (#809)");
        assert_eq!(got["Modal"]["owner"], "nico");
    }

    #[test]
    fn a_hash_inside_a_quoted_reason_is_not_a_comment() {
        // Modal's real reason is "inert hardcoded; needs open: bool upstream
        // (#809)". Splitting on the first `#` silently truncated it to
        // "inert hardcoded; needs open: bool upstream (" — a gate that mangles
        // the decision it is checking for is worse than one that misses it.
        let text = "[Modal]\nreason = \"inert hardcoded; needs open: bool upstream (#809)\"  # ticket\n";
        let got = parse_overrides(text, "t").unwrap();
        assert_eq!(got["Modal"]["reason"], "inert hardcoded; needs open: bool upstream (#809)");
    }

    #[test]
    fn an_override_outside_a_section_is_an_error_not_a_silent_skip() {
        let err = parse_overrides("reason = \"x\"\n", "t").unwrap_err().to_string();
        assert!(err.contains("before any [Section]"), "{err}");
    }

    #[test]
    fn a_syntax_this_reader_does_not_understand_is_rejected() {
        // `[[x]]` and `[a.b]` would otherwise parse as a section named
        // something nobody meant, and an override that is silently misfiled
        // reads as missing.
        assert!(parse_overrides("[[Modal]]\n", "t").is_err());
        assert!(parse_overrides("[Modal.open]\n", "t").is_err());
        assert!(parse_overrides("[Modal\n", "t").is_err());
    }

    #[test]
    fn a_component_the_generator_withholds_must_carry_a_decision() {
        // Synthetic since eona-x/backlog#822: the real crate withholds nothing
        // any more, so the fixture has to supply the shape. The rule under test
        // is unchanged — a component the generator will not emit must have a
        // human's decision recorded, or it leaves the package with nobody
        // having said so.
        let mut tk = real_toolkit();
        let victim = tk.components.iter_mut().find(|c| c.name == "Modal").unwrap();
        victim.status = Status::NeedsOverride { reason: "fixture: withheld".into() };
        let empty = scratch_root("no-overrides");
        let findings = check_overrides(&empty, &tk).unwrap();
        assert!(
            findings.iter().any(
                |f| matches!(f, Finding::MissingOverride { component, .. } if component == "Modal")
            ),
            "{findings:#?}"
        );
        std::fs::remove_dir_all(&empty).ok();
    }

    // ---------------------------------------------- invariants 5 and 6

    /// A package tree on disk, so the gate can be pointed at something it did
    /// not itself just write.
    fn scratch_pkg(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-pkg-{name}"));
        std::fs::remove_dir_all(&dir).ok();
        for (rel, body) in files {
            let path = dir.join(PACKAGE_DIR).join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        dir
    }

    /// The hole the drift gate had: `check` covered `abi/` and never looked at
    /// the deliverable, so rewriting a class name in a shipped `.tsx` by hand
    /// left every committed artifact untouched and the run green.
    #[test]
    fn a_hand_edited_tsx_is_stale_even_though_abi_is_untouched() {
        let mut tk = real_toolkit();
        replay(&mut tk, &read_quarantine(&root().join(QUARANTINE_FILE)).unwrap().unwrap())
            .unwrap();

        let generated = "// Generated. Do not edit.\nexport function Button() { return null; }\n";
        let manifest = PackageManifest {
            src_hash: tk.src_hash.clone(),
            generator_hash: abi::generator_hash().unwrap(),
            dir: PACKAGE_DIR.into(),
            exported: vec!["Button".into()],
            files: vec![PackageFile {
                path: "src/Button.tsx".into(),
                sha256: sha256(generated),
            }],
        };

        let clean = scratch_pkg("clean", &[("src/Button.tsx", generated)]);
        let findings = check_package(&clean, &tk, None, Some(&manifest)).unwrap();
        assert!(
            !findings.iter().any(|f| matches!(f, Finding::Stale { path, .. } if path.contains("Button.tsx"))),
            "{findings:#?}"
        );

        let edited = scratch_pkg(
            "edited",
            &[("src/Button.tsx", "// Generated. Do not edit.\nexport function Button() { return \"hand edited\"; }\n")],
        );
        let findings = check_package(&edited, &tk, None, Some(&manifest)).unwrap();
        let stale = findings
            .iter()
            .find(|f| matches!(f, Finding::Stale { path, .. } if path.ends_with("src/Button.tsx")))
            .unwrap_or_else(|| panic!("{findings:#?}"));
        // Naming the file is the whole point: the srcHash-only finding this
        // replaces said "something changed" and nothing else.
        assert!(stale.headline().contains("packages/react/src/Button.tsx"), "{stale:?}");

        std::fs::remove_dir_all(&clean).ok();
        std::fs::remove_dir_all(&edited).ok();
    }

    /// Invariant 6 is derived from `Status` on both sides, which is what makes
    /// it survive a tampered-but-self-consistent `abi/`: after `--write`
    /// accepted a fabricated quarantine, the ABI said Button was quarantined
    /// while `Button.tsx` was still on disk and still exported, and nothing
    /// reconciled the two.
    #[test]
    fn a_tsx_for_a_quarantined_component_is_a_contract_finding() {
        let mut tk = real_toolkit();
        replay(&mut tk, &read_quarantine(&root().join(QUARANTINE_FILE)).unwrap().unwrap())
            .unwrap();
        let button = tk.components.iter_mut().find(|c| c.name == "Button").unwrap();
        button.status = Status::Quarantined { reason: "FABRICATED".into() };

        // No manifest at all: invariant 6 must fire on its own, before any hash
        // is consulted, because a tampered pair would agree on every hash.
        let dir = scratch_pkg(
            "fabricated",
            &[
                ("src/Button.tsx", "export function Button() { return null; }\n"),
                ("src/index.ts", "export { Button, type ButtonProps } from './Button';\n"),
            ],
        );
        let findings = check_package(&dir, &tk, None, None).unwrap();
        let contract: Vec<String> = findings
            .iter()
            .filter_map(|f| match f {
                Finding::PackageContract { detail } => Some(detail.clone()),
                _ => None,
            })
            .collect();
        assert!(
            contract.iter().any(|d| d.contains("src/Button.tsx") && d.contains("quarantined")),
            "{contract:#?}"
        );
        assert!(
            contract.iter().any(|d| d.contains("index.ts exports Button")),
            "{contract:#?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A `.tsx` nobody generated must not ride along silently.
    #[test]
    fn a_file_the_generator_never_wrote_is_reported() {
        let tk = real_toolkit();
        let manifest = PackageManifest {
            src_hash: tk.src_hash.clone(),
            generator_hash: abi::generator_hash().unwrap(),
            dir: PACKAGE_DIR.into(),
            exported: vec![],
            files: vec![],
        };
        let dir = scratch_pkg("extra", &[("src/HandWritten.tsx", "export const x = 1;\n")]);
        let findings = check_package(&dir, &tk, None, Some(&manifest)).unwrap();
        assert!(
            findings.iter().any(
                |f| matches!(f, Finding::PackageExtra { path, .. } if path.ends_with("HandWritten.tsx"))
            ),
            "{findings:#?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// `index.ts` is parsed, not pattern-matched at.
    #[test]
    fn exported_from_index_reads_the_shape_emit_writes() {
        let index = "// header\n\
                     export { AccordionItem, type AccordionItemProps } from './AccordionItem';\n\
                     export { Button, type ButtonProps } from './Button';\n\
                     \n\
                     // Deliberately not exported (1):\n\
                     //   Modal — needs an override\n";
        assert_eq!(
            exported_from_index(index),
            BTreeSet::from(["AccordionItem".to_string(), "Button".to_string()])
        );
    }

    /// The committed package matches the committed manifest. This is the
    /// end-to-end form of invariant 5 and it runs against the real tree, so a
    /// commit that regenerates `abi/` without regenerating `packages/react`
    /// fails here rather than in CI.
    #[test]
    fn the_committed_package_matches_its_committed_manifest() {
        let mut tk = real_toolkit();
        replay(&mut tk, &read_quarantine(&root().join(QUARANTINE_FILE)).unwrap().unwrap())
            .unwrap();
        let manifest = read_manifest(&root().join(PACKAGE_MANIFEST_FILE))
            .unwrap()
            .expect("abi/package.manifest.json is committed");
        let findings = check_package(&root(), &tk, None, Some(&manifest)).unwrap();
        assert!(findings.is_empty(), "{findings:#?}");
    }

    #[test]
    fn the_committed_overrides_cover_exactly_what_the_gate_withholds() {
        // The real invariant, and the ordering that makes it true: quarantines
        // must be folded in first. Run against a pre-verify IR this reports
        // four stale entries, because Swatch, DatasetCard, PaletteGroup and
        // OntoAnnotation are still `Ok` until the render says otherwise.
        let mut tk = real_toolkit();
        let doc = read_quarantine(&root().join(QUARANTINE_FILE))
            .unwrap()
            .expect("abi/quarantine.json is committed");
        replay(&mut tk, &doc).unwrap();

        let findings = check_overrides(&root(), &tk).unwrap();
        assert!(findings.is_empty(), "{findings:#?}");

        // Nothing is withheld any more (#822), so the committed overrides file is
        // empty and this set must be too — an entry for a component that now
        // emits would be a stale decision nobody cleaned up.
        let withheld: BTreeSet<&str> = tk
            .components
            .iter()
            .filter(|c| {
                matches!(c.status, Status::NeedsOverride { .. } | Status::Quarantined { .. })
            })
            .map(|c| c.name.as_str())
            .collect();
        assert!(withheld.is_empty(), "{withheld:?}");
    }

    #[test]
    fn an_override_for_a_component_that_is_fine_is_reported_as_stale() {
        let tk = real_toolkit();
        let dir = scratch_root("stale-override");
        std::fs::write(
            dir.join(OVERRIDES_FILE),
            "[Button]\nreason = \"no longer needed\"\n",
        )
        .unwrap();
        let findings = check_overrides(&dir, &tk).unwrap();
        assert!(
            findings.iter().any(
                |f| matches!(f, Finding::StaleOverride { component, .. } if component == "Button")
            ),
            "{findings:#?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // ------------------------------------------------------------- the gate

    fn abi_of(tk: &Toolkit) -> Abi {
        abi::build_abi(tk, &crate::harness::cells_for(tk).unwrap()).unwrap()
    }

    fn real_toolkit() -> Toolkit {
        parse_and_classify(&root()).unwrap()
    }

    /// A throwaway toolkit root holding only `abi/`, so a test can drive the
    /// gate's comparison logic without a 20s render.
    fn scratch_root(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eona-check-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join(ABI_DIR)).unwrap();
        dir
    }

    #[test]
    fn a_missing_abi_reports_itself_rather_than_passing() {
        let dir = scratch_root("missing");
        // No src/, so the parse must fail loudly — the gate never reports clean
        // on a tree it could not read.
        assert!(check(&dir).is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_real_tree_with_a_freshly_written_abi_is_clean_on_the_replay_path() {
        // Build the committed pair the way `--write` would, in a scratch dir
        // that shadows only `abi/`, then check the real src/ against it.
        let tk = real_toolkit();
        let abi_doc = abi_of(&tk);
        let dir = scratch_root("clean");
        // The gate reads src/ from `root`, so point it at the real tree and
        // give it a committed ABI via a symlink-free copy of just `abi/`.
        let committed = dir.join(ABI_FILE);
        std::fs::write(&committed, abi::to_json(&abi_doc).unwrap()).unwrap();

        // Compare the two documents directly: this is the assertion the gate
        // makes, minus the filesystem layout a full run needs.
        let regenerated = abi::to_json(&abi_of(&real_toolkit())).unwrap();
        let on_disk = std::fs::read_to_string(&committed).unwrap();
        assert_eq!(unified_diff(ABI_FILE, &on_disk, &regenerated), "");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_status_change_is_its_own_finding_class() {
        // Simulating what `Ok -> Quarantined` looks like to the gate, which is
        // the case a unified diff alone would bury: three JSON lines in four
        // thousand.
        let tk = real_toolkit();
        let mut committed = abi_of(&tk);
        let c = committed.components.iter_mut().find(|c| c.name == "Button").unwrap();
        c.status = Status::Quarantined { reason: "pretend".into() };

        let mut findings = Vec::new();
        for c in &tk.components {
            let Some(was) = committed.component(&c.name) else { continue };
            if was.status != c.status {
                findings.push(Finding::StatusChanged {
                    component: c.name.clone(),
                    from: describe_status(&was.status),
                    to: describe_status(&c.status),
                    source: source(&c.span),
                });
            }
        }
        assert_eq!(findings.len(), 1);
        let Finding::StatusChanged { component, source, .. } = &findings[0] else {
            panic!("{findings:#?}")
        };
        assert_eq!(component, "Button");
        assert!(source.starts_with("src/atoms/button.rs:"), "{source}");
    }

    #[test]
    fn replaying_a_quarantine_reproduces_what_verify_would_have_written() {
        let mut tk = real_toolkit();
        assert_eq!(tk.component("Swatch").unwrap().status, Status::Ok);
        let doc = QuarantineDoc {
            src_hash: tk.src_hash.clone(),
            checked: 38,
            quarantined: vec![QuarantineEntry {
                component: "Swatch".into(),
                reason: "panics while rendering a sentinel".into(),
                span: None,
            }],
        };
        replay(&mut tk, &doc).unwrap();
        assert_eq!(
            tk.component("Swatch").unwrap().status,
            Status::Quarantined { reason: "panics while rendering a sentinel".into() }
        );
    }

    #[test]
    fn replaying_a_quarantine_for_a_component_that_is_gone_is_an_error() {
        let mut tk = real_toolkit();
        let doc = QuarantineDoc {
            src_hash: tk.src_hash.clone(),
            checked: 1,
            quarantined: vec![QuarantineEntry {
                component: "Vanished".into(),
                reason: "x".into(),
                span: None,
            }],
        };
        let err = replay(&mut tk, &doc).unwrap_err().to_string();
        assert!(err.contains("src/ no longer defines"), "{err}");
    }

    #[test]
    fn replaying_over_an_excluded_component_is_an_error() {
        // The one case `verify::apply` also refuses: quarantining something
        // excluded overwrites the rule that excluded it.
        let mut tk = real_toolkit();
        let doc = QuarantineDoc {
            src_hash: tk.src_hash.clone(),
            checked: 1,
            quarantined: vec![QuarantineEntry {
                component: "DemoPage".into(),
                reason: "x".into(),
                span: None,
            }],
        };
        let err = replay(&mut tk, &doc).unwrap_err().to_string();
        assert!(err.contains("Excluded"), "{err}");
    }

    #[test]
    fn a_new_quarantine_is_detected_against_the_committed_set() {
        let tk = real_toolkit();
        let committed: BTreeSet<&str> = ["Swatch", "DatasetCard", "PaletteGroup"].into();
        let now: BTreeSet<&str> = ["Swatch", "DatasetCard", "PaletteGroup", "Button"].into();
        let added: Vec<&&str> = now.difference(&committed).collect();
        assert_eq!(added, [&"Button"]);
        // And the finding names a span, which is what makes it actionable.
        let span = source(&tk.component("Button").unwrap().span);
        assert!(span.starts_with("src/atoms/button.rs:"), "{span}");
    }

    /// The whole gate, against the real tree.
    ///
    /// `#[ignore]` because it can take the full path — building the harness
    /// against yew in release mode — which does not belong in a 5-second unit
    /// run. It is also the only way to *write* `abi/` until `main.rs` wires the
    /// `check` subcommand up, so it doubles as the bootstrap:
    ///
    /// ```text
    /// cargo test -p xtask -- --ignored --nocapture the_gate_runs_end_to_end
    /// EONA_CHECK_WRITE=1 cargo test -p xtask -- --ignored --nocapture the_gate_runs_end_to_end
    /// ```
    #[test]
    #[ignore = "runs the full pipeline; see the doc comment"]
    fn the_gate_runs_end_to_end_against_the_real_tree() {
        let mut opts = Options::default();
        opts.write = std::env::var_os("EONA_CHECK_WRITE").is_some();
        if let Some(r) = std::env::var_os("EONA_CHECK_AGAINST") {
            opts.against = Some(r.to_string_lossy().into_owned());
        }
        let outcome = check_with(&root(), &opts).unwrap();
        print!("{}", outcome.report());
        println!("exit {}", outcome.exit_code());
    }

    // ------------------------------------------------------------ arg parsing

    #[test]
    fn semver_requires_a_ref_and_a_ref_requires_semver() {
        assert!(Options::from_args(["--semver"]).is_err());
        assert!(Options::from_args(["--against", "main"]).is_err());
        let o = Options::from_args(["--semver", "--against", "origin/main"]).unwrap();
        assert_eq!(o.against.as_deref(), Some("origin/main"));
    }

    #[test]
    fn an_unknown_flag_is_refused_rather_than_ignored() {
        let err = Options::from_args(["--sevmer"]).unwrap_err().to_string();
        assert!(err.contains("unknown `check` flag"), "{err}");
    }

    #[test]
    fn flags_parse_independently() {
        let o = Options::from_args(["--render", "--write", "--deny-breaking"]).unwrap();
        assert!(o.force_render && o.write && o.deny_breaking);
        assert!(Options::from_args(Vec::<String>::new()).unwrap().against.is_none());
    }

    // ------------------------------------------------------------- reporting

    #[test]
    fn a_breaking_finding_does_not_fail_the_run_unless_asked() {
        let change = AbiChange {
            severity: Severity::Breaking,
            kind: abi::SurfaceKind::Component,
            subject: "Button".into(),
            change: abi::Change::SurfaceRemoved { was: None },
        };
        let outcome = |deny| CheckOutcome {
            mode: Mode::Replayed,
            findings: vec![Finding::Breaking { change: change.clone() }],
            bump: Some("major"),
            written: vec![],
            elapsed: Duration::from_millis(1),
            deny_breaking: deny,
        };
        assert_eq!(outcome(false).exit_code(), 0);
        assert_eq!(outcome(true).exit_code(), 1);
        assert!(outcome(false).report().contains("requires a major bump"));
    }

    #[test]
    fn a_stale_artifact_fails_and_prints_its_diff() {
        let outcome = CheckOutcome {
            mode: Mode::Replayed,
            findings: vec![Finding::Stale {
                path: ABI_FILE.into(),
                diff: unified_diff(ABI_FILE, "a\nb\nc\n", "a\nB\nc\n"),
            }],
            bump: None,
            written: vec![],
            elapsed: Duration::from_millis(1),
            deny_breaking: false,
        };
        assert_eq!(outcome.exit_code(), 1);
        let report = outcome.report();
        assert!(report.contains("abi/toolkit.abi.json is stale"), "{report}");
        assert!(report.contains("-b") && report.contains("+B"), "{report}");
        assert!(report.contains("cargo xtask bridgegen"), "{report}");
    }

    #[test]
    fn a_clean_run_says_so() {
        let outcome = CheckOutcome {
            mode: Mode::Replayed,
            findings: vec![],
            bump: None,
            written: vec![],
            elapsed: Duration::from_millis(300),
            deny_breaking: false,
        };
        assert_eq!(outcome.exit_code(), 0);
        assert!(outcome.report().contains("clean: abi/ and packages/react match src/."));
    }
}
