//! `cargo xtask` — the eona-ui-toolkit React generator.
//!
//! Pipeline: parse -> classify -> matrix -> abi -> harness -> render -> verify
//! -> splice -> emit -> examples, with `check` re-running the whole thing and
//! diffing.

mod abi;
mod check;
mod classify;
mod emit;
mod examples;
mod harness;
mod ir;
mod matrix;
mod parse;
mod render;
mod splice;
mod verify;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::ir::{Status, Tier, Toolkit};

fn main() -> Result<()> {
    let cmd = std::env::args().nth(1).unwrap_or_default();
    match cmd.as_str() {
        "dump-ir" => dump_ir(),
        "render" => render(),
        "bridgegen" => bridgegen(),
        "check" => check_cmd(),
        other => bail!(
            "unknown subcommand {other:?}; expected `dump-ir`, `render`, `bridgegen` or `check`"
        ),
    }
}

/// The toolkit crate root: xtask's parent, so the command works from anywhere.
fn toolkit_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Run the front half of the pipeline and write the IR out, so the scope rules
/// can be inspected before any rendering budget is spent. Deliberately stops
/// before `harness`: the numbers this prints are the ones the later stages are
/// checked against, so they have to come from the IR alone.
fn dump_ir() -> Result<()> {
    let root = toolkit_root();
    let mut tk = parse::parse(&root)
        .with_context(|| format!("parsing {}", root.join("src").display()))?;
    classify::classify(&mut tk).context("classifying props")?;

    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("out");
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("creating {}", out_dir.display()))?;
    let out = out_dir.join("ir.json");
    let json = serde_json::to_string_pretty(&tk).context("serialising the IR")?;
    std::fs::write(&out, format!("{json}\n"))
        .with_context(|| format!("writing {}", out.display()))?;

    summary(&tk, &root, &out)
}

fn summary(tk: &Toolkit, root: &Path, out: &Path) -> Result<()> {
    println!("eona-ui-toolkit IR");
    println!("  source   {}", root.join("src").display());
    println!("  src_hash {}", tk.src_hash);
    println!("  written  {}", out.display());
    println!();

    let props_structs =
        tk.components.iter().filter_map(|c| c.props_ty.as_deref()).collect::<std::collections::BTreeSet<_>>();
    println!("components      {}", tk.components.len());
    println!("  with props    {}", tk.components.iter().filter(|c| c.props_ty.is_some()).count());
    println!("  zero-prop     {}", tk.components.iter().filter(|c| c.props_ty.is_none()).count());
    println!("props structs   {}", props_structs.len());
    println!("enums           {}", tk.enums.len());
    println!("plain structs   {}", tk.structs.len());
    println!(
        "tiers           {} presentational / {} interactive",
        tk.components.iter().filter(|c| c.tier == Tier::Presentational).count(),
        tk.components.iter().filter(|c| c.tier == Tier::Interactive).count(),
    );
    println!();

    // Grouped by the exact rule string, because "how many were excluded" is not
    // actionable but "which rule excluded them" is.
    let mut buckets: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for c in &tk.components {
        let key = match &c.status {
            Status::Ok => "ok".to_string(),
            Status::NeedsOverride { reason } => format!("needs-override: {reason}"),
            Status::Quarantined { reason } => format!("quarantined: {reason}"),
            Status::Excluded { reason } => format!("excluded: {reason}"),
        };
        buckets.entry(key).or_default().push(&c.name);
    }
    println!("by status");
    for (key, names) in &buckets {
        println!("  {:>3}  {key}", names.len());
        if !key.starts_with("ok") {
            println!("       {}", names.join(", "));
        }
    }

    let in_scope: Vec<&str> = tk
        .components
        .iter()
        .filter(|c| !matches!(c.status, Status::Excluded { .. }))
        .map(|c| c.name.as_str())
        .collect();
    println!();
    println!("in scope (everything not excluded): {}", in_scope.len());
    let mut sorted = in_scope.clone();
    sorted.sort_unstable();
    println!("  {}", sorted.join(", "));

    println!();
    // One pass, built from `cells_with_report` rather than `all_cells` +
    // `summarise`: those two each walk the whole crate and each log the same
    // dominance edge to stderr, so the report below would print twice. The
    // filter matches `all_cells` — an excluded component is never rendered.
    let mut per: Vec<(String, usize)> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    for c in tk.components.iter().filter(|c| !matches!(c.status, Status::Excluded { .. })) {
        let (cells, report) = matrix::cells_with_report(c, tk)
            .with_context(|| format!("deriving render cells for {}", c.name))?;
        per.push((c.name.clone(), cells.len()));
        let name = &c.name;
        for edge in &report.dominance {
            notes.push(format!(
                "  {name}: assumed `{}` ({}:{}) is only read under `{}` ({}:{}) [{}] — {} cells pruned, probe {}",
                edge.dominated,
                edge.dominated_span.file,
                edge.dominated_span.line,
                edge.dominator,
                edge.dominator_span.file,
                edge.dominator_span.line,
                edge.rule,
                edge.pruned,
                edge.probe.as_deref().unwrap_or("none"),
            ));
        }
        for d in &report.depth_limited {
            notes.push(format!(
                "  {name}: depth limit at `{}` ({}) — {}:{}",
                d.path, d.ty, d.span.file, d.span.line
            ));
        }
        if let Some(t) = &report.truncated {
            notes.push(format!(
                "  {name}: cross product {} exceeded cap {} — emitted {} one-factor cells, axis interactions lost",
                t.product, t.cap, t.emitted
            ));
        }
    }
    let total: usize = per.iter().map(|(_, n)| n).sum();
    let mut counts: Vec<usize> = per.iter().map(|(_, n)| *n).collect();
    counts.sort_unstable();
    let median = if counts.is_empty() {
        0.0
    } else if counts.len() % 2 == 1 {
        counts[counts.len() / 2] as f64
    } else {
        (counts[counts.len() / 2 - 1] + counts[counts.len() / 2]) as f64 / 2.0
    };
    println!("render cells");
    println!("  components  {}", per.len());
    println!("  total       {total}");
    if let Some((name, n)) = per.iter().max_by_key(|(_, n)| *n) {
        println!("  max         {n} ({name})");
    }
    println!("  median      {median}");
    let mut busiest: Vec<&(String, usize)> = per.iter().filter(|(_, n)| *n > 2).collect();
    busiest.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!(
        "  busiest     {}",
        busiest.iter().map(|(n, c)| format!("{n}={c}")).collect::<Vec<_>>().join(" ")
    );

    if !notes.is_empty() {
        println!();
        println!("matrix decisions to check ({}):", notes.len());
        for n in notes {
            println!("{n}");
        }
    }
    Ok(())
}

/// parse -> classify -> matrix -> harness -> render -> verify.
///
/// The one command that actually executes the toolkit's own components: it
/// writes a throwaway crate into `xtask/out/harness`, builds it against the real
/// `eona-ui-toolkit` with `ssr`, and renders every cell twice — once per sentinel
/// alphabet, which is what makes `verify`'s differential check a check rather
/// than a restatement of the first one.
///
/// It deliberately does not stop at the first quarantine. `Swatch` and
/// `DatasetCard` are *expected* to fail here (`src/molecules/swatch.rs:11`,
/// `src/molecules/dataset_card.rs:43`), so a run that aborts on a quarantine
/// could never finish; `VerifyReport::self_check` is what turns a *surprising*
/// one into an error, and it runs last so the artefacts are on disk either way.
fn render() -> Result<()> {
    let root = toolkit_root();
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("out");
    let Rendered { mut tk, results, report, renders_json, .. } = run_render(&root, &out_dir)?;

    let quarantine_json = out_dir.join("quarantine.json");
    report.write(&quarantine_json)?;
    report.emit();
    verify::apply(&mut tk, &report).context("folding quarantines back into the IR")?;
    let json = serde_json::to_string_pretty(&tk).context("serialising the IR")?;
    std::fs::write(out_dir.join("ir.json"), format!("{json}\n"))
        .with_context(|| format!("writing {}", out_dir.join("ir.json").display()))?;

    render_summary(&tk, &report, &results, &renders_json, &quarantine_json);
    report.self_check()
}

/// `harness::HarnessPlan` -> the per-render plans `verify::collect` joins on.
///
/// The two stages number the same thing differently — `harness` hands out a
/// `usize` per text leaf, `verify` keys sentinels by `u32` — and only `harness`
/// knows the traversal order that assigned them, so the translation lives here
/// rather than being re-derived on either side. The key has to match what the
/// generated `render_both` writes: `<Component>/<cell>#<alphabet>`.
fn render_plans(plan: &harness::HarnessPlan) -> BTreeMap<String, verify::RenderPlan> {
    let mut out = BTreeMap::new();
    for cell in &plan.cells {
        for alphabet in verify::Alphabet::ALL {
            let sentinels = cell
                .sentinels
                .iter()
                .map(|b| verify::Sentinel {
                    id: b.index as u32,
                    // The concrete path (`colors[1].hex`), because that is what a
                    // human opens; the axis it sits under is in plan.json.
                    path: b.path.clone(),
                    span: b.span.clone(),
                })
                .collect();
            out.insert(
                format!("{}#{}", cell.key, alphabet.name()),
                verify::RenderPlan {
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

fn render_summary(
    tk: &Toolkit,
    report: &verify::VerifyReport,
    results: &BTreeMap<String, render::RenderResult>,
    renders_json: &Path,
    quarantine_json: &Path,
) {
    let panicked = render::panics(results).len();
    println!();
    println!("renders        {} ({} html, {panicked} panicked)", results.len(), results.len() - panicked);
    println!("  written      {}", renders_json.display());
    println!("components     {} checked", report.components.len());
    println!("  emitted      {}", report.emitted_count());
    println!("  quarantined  {}", report.quarantined().count());
    println!(
        "  noted        {} (passed, but splice must honour a finding)",
        report.passed().filter(|c| !c.notes.is_empty()).count()
    );
    println!("  written      {}", quarantine_json.display());

    let mut by_status: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for c in &tk.components {
        let key = match &c.status {
            Status::Ok => "ok".to_string(),
            Status::NeedsOverride { reason } => format!("needs-override: {reason}"),
            Status::Quarantined { reason } => format!("quarantined: {reason}"),
            Status::Excluded { reason } => format!("excluded: {reason}"),
        };
        by_status.entry(key).or_default().push(&c.name);
    }
    println!();
    println!("IR after verify");
    for (key, names) in &by_status {
        println!("  {:>3}  {key}", names.len());
        if !key.starts_with("ok") {
            println!("       {}", names.join(", "));
        }
    }
}

/// parse -> classify -> matrix -> harness -> render -> verify, the half of the
/// pipeline that has to execute the toolkit's own components.
///
/// Factored out of [`render`] so `bridgegen` cannot drift from the `render`
/// subcommand people debug with: the two commands differ only in what they do
/// with the result, never in how the result was produced.
struct Rendered {
    tk: Toolkit,
    cells: BTreeMap<String, Vec<matrix::Cell>>,
    plan: harness::HarnessPlan,
    results: BTreeMap<String, render::RenderResult>,
    report: verify::VerifyReport,
    renders_json: PathBuf,
}

fn run_render(root: &Path, out_dir: &Path) -> Result<Rendered> {
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("creating {}", out_dir.display()))?;

    let mut tk = parse::parse(root)
        .with_context(|| format!("parsing {}", root.join("src").display()))?;
    classify::classify(&mut tk).context("classifying props")?;

    let cells = harness::cells_for(&tk).context("deriving render cells")?;
    let harness_dir = out_dir.join("harness");
    harness::emit_harness(&tk, &cells, &harness_dir)
        .with_context(|| format!("writing the harness crate to {}", harness_dir.display()))?;
    let plan = harness::plan(&tk, &cells).context("rebuilding the harness plan")?;

    let renders_json = out_dir.join("renders.json");
    let (results, run) = render::render_all_with(&harness_dir, &renders_json, &tk.src_hash)
        .context("building and running the harness")?;
    println!("{}", run.summary());

    let plans = render_plans(&plan);
    let collected = verify::collect(&results, &plans)
        .context("joining the harness plan to the render results")?;
    let report = verify::verify(&tk, &collected).context("running the verify gate")?;
    Ok(Rendered { tk, cells, plan, results, report, renders_json })
}

/// The whole generator: parse -> classify -> matrix -> harness -> render ->
/// verify -> splice -> emit -> write_abi.
///
/// Order matters in one place that is easy to get wrong. `verify::apply` folds
/// the render verdicts into the IR *before* `splice` and `emit` run, because
/// both of them dispatch on `Status` — `splice_toolkit` skips a quarantined
/// component and `emit` refuses to write a package that is silently one
/// component short. Running either against the pre-verify IR would emit TSX for
/// `Swatch`, whose sentinels never survived `src/molecules/swatch.rs:11`.
fn bridgegen() -> Result<()> {
    let root = toolkit_root();
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("out");
    let Rendered { mut tk, cells, plan, results, report, renders_json } =
        run_render(&root, &out_dir)?;

    let quarantine_json = out_dir.join("quarantine.json");
    report.write(&quarantine_json)?;
    report.emit();
    verify::apply(&mut tk, &report).context("folding quarantines back into the IR")?;

    let spliced = splice::splice_toolkit(&tk, &cells, &plan, &results)
        .context("splicing the rendered markup back into JSX")?;

    // A component that rendered cleanly and then could not be reconciled into a
    // single tree is a generator defect, not a property of the toolkit: the
    // verify gate already proved every sentinel arrived intact. Failing here
    // rather than quietly dropping it is what stops the package from shipping
    // 34 components while claiming 35.
    if !spliced.failures.is_empty() {
        for (name, why) in &spliced.failures {
            eprintln!("splice: {name} — {why}");
        }
        bail!(
            "{} component(s) passed the verify gate but could not be spliced: {}",
            spliced.failures.len(),
            spliced.failures.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
        );
    }

    let jsx = to_emit_jsx(&spliced, &report);
    // `render_package` then `write_package`, not `emit::emit`: the drift gate's
    // manifest has to carry hashes of the exact bytes that were written, and
    // routing both through one map is what makes that true by construction
    // rather than by two code paths agreeing.
    let (package, warnings) =
        emit::render_package(&tk, &jsx).context("rendering the React package")?;
    emit::write_files(&package, &root.join(emit::PACKAGE_DIR))
        .context("writing the React package")?;
    for w in &warnings {
        eprintln!("emit: {w}");
    }
    emit::report_package(&package, &root.join(emit::PACKAGE_DIR));

    // Built once and then both written and handed on: `examples` derives its
    // member list, TS types, optionality and defaults from this exact document,
    // so rebuilding it there could only introduce a way for the two to disagree.
    let abi_doc = abi::build_abi(&tk, &cells).context("building the ABI")?;
    std::fs::create_dir_all(root.join(check::ABI_DIR))?;
    std::fs::write(root.join(check::ABI_FILE), abi::to_json(&abi_doc)?)
        .with_context(|| format!("writing {}", check::ABI_FILE))?;
    report.write(&root.join(check::QUARANTINE_FILE))?;
    check::write_package(&root, &tk, &package).context("writing the package manifest")?;

    let overrides = check::read_overrides(&root)?;
    let snippets = examples::build(&abi_doc, &tk, &overrides)
        .context("deriving the per-component usage snippets")?;
    let example_files = examples::write(&root, &snippets)
        .context("writing the usage snippets")?;

    render_summary(&tk, &report, &results, &renders_json, &quarantine_json);
    println!();
    println!("wrote");
    println!("  {}", root.join(emit::PACKAGE_DIR).display());
    println!("  {}", root.join(check::ABI_FILE).display());
    println!("  {}", root.join(check::QUARANTINE_FILE).display());
    println!("  {}", root.join(check::PACKAGE_MANIFEST_FILE).display());
    for rel in &example_files {
        println!("  {}", root.join(rel).display());
    }
    report.self_check()
}

/// `splice::Jsx` (a tree) -> `emit::Jsx` (a file body plus its provenance).
///
/// The two stages deliberately do not share a type: `splice` owns the shape of
/// the markup and `emit` owns the shape of the file, and the only thing that
/// crosses is text. `to_tsx` lives on splice's tree so the tree and its
/// spelling cannot disagree.
///
/// `to_return_body`, not `to_tsx`: `emit` puts the body straight inside a
/// `return ( ... )`, which is expression position.
///
/// `notes` come from the verify gate rather than from splice, because they are
/// evidence about the *render* — that `src/molecules/nav_dropdown.rs:27,29,35,43`
/// writes `id` four times, say — and a reader of the generated file needs that
/// next to the markup that looks redundant because of it.
fn to_emit_jsx(
    spliced: &splice::SpliceReport,
    report: &verify::VerifyReport,
) -> BTreeMap<String, emit::Jsx> {
    let mut out = BTreeMap::new();
    for (name, tree) in &spliced.components {
        // One note per (check, cell, prop), not one per alphabet. Every cell is
        // rendered twice and a note that is a fact about the *component* —
        // "`language` appears twice" — is therefore reported twice, differing
        // only in which sentinel text it quotes. Both copies used to reach the
        // generated file's header, which since `verify::PAD` grew means 70
        // characters of probe alphabet in a comment a human is meant to read.
        // The full pair stays in `abi/quarantine.json`, which is where evidence
        // belongs; this is the reader's copy.
        //
        // Keyed on the span rather than on the text: two props noted in the same
        // cell have different declarations, so they survive as two notes.
        let mut seen = std::collections::BTreeSet::new();
        let notes = report
            .components
            .iter()
            .find(|c| &c.component == name)
            .map(|c| {
                c.notes
                    .iter()
                    .filter(|e| {
                        seen.insert((e.check, e.cell.clone(), e.span.file.clone(), e.span.line))
                    })
                    .map(|e| e.to_string())
                    .collect()
            })
            .unwrap_or_default();
        out.insert(
            name.clone(),
            emit::Jsx {
                component: name.clone(),
                helpers: helpers_for(tree),
                body: tree.to_return_body(),
                notes,
            },
        );
    }
    out
}

/// The module-level declarations a spliced tree depends on.
///
/// Only one exists today: `splice::STYLE_HELPER`, which
/// `src/molecules/logo_download_card.rs:30` forces — `preview_style` is raw CSS
/// text and React's `style` prop takes an object, so there is no way to write
/// that attribute without a parse. It is emitted per file rather than into a
/// shared module because `emit` writes one self-contained file per component
/// (see `emit.rs`'s header: no generated component imports another).
fn helpers_for(tree: &splice::Jsx) -> Vec<String> {
    let tsx = tree.to_return_body();
    let mut out = Vec::new();
    if tsx.contains(&format!("{}(", splice::STYLE_HELPER)) {
        out.push(splice::STYLE_HELPER_TS.to_string());
    }
    out
}

/// The drift gate. Exits non-zero rather than returning an error, because its
/// findings are a report a human reads, not a Rust backtrace.
fn check_cmd() -> Result<()> {
    let opts = check::Options::from_args(std::env::args().skip(2))?;
    let outcome = check::check_with(&toolkit_root(), &opts)?;
    print!("{}", outcome.report());
    std::process::exit(outcome.exit_code())
}
