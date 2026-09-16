//! Stage: `render`. Builds the generated harness crate, runs it, and collects
//! the HTML it wrote.
//!
//! This stage owns no rendering logic of its own. `harness` writes a crate whose
//! binary renders every cell `matrix` derived — one zero-prop wrapper per cell,
//! props filled with the `\u{E000}<n>\u{E001}` sentinels, through
//! `yew::LocalServerRenderer::<W>::new().hydratable(false)` — and writes the
//! results out as JSON. Everything here is process plumbing around that binary:
//! run it, don't let it hang, don't let a build failure look like a successful
//! run that found nothing, and don't rebuild when nothing has changed.
//!
//! # The output contract
//!
//! The harness is invoked as `<harness-bin> <out_json>` and must write a JSON
//! object keyed by cell id:
//!
//! ```json
//! {
//!   "Button/base":      { "html":  "<button class=\"btn btn-primary\" …>" },
//!   "Swatch/base":      { "panic": "byte index 2 is not a char boundary …" }
//! }
//! ```
//!
//! Ids are opaque here: `matrix::Cell::id` is unique only within a component, so
//! the harness qualifies it with the component name. [`component_of`] is the only
//! place that looks inside an id, and only for reporting.
//!
//! [`parse_results`] also accepts the shapes a harness is likely to produce by
//! accident — a `{"results": {…}}` wrapper, an array of `{"id": …}` records, a
//! bare string value, and serde's own externally tagged `{"Html": …}` — because
//! the alternative is a cross-stage break that only shows up as an empty map.
//! Emitting the canonical shape above is still what `harness` should do.
//!
//! # Why a panic is a result and not an error
//!
//! Two components are known to panic on sentinel input, and both are *correct*
//! to: `src/molecules/swatch.rs:11` slices `&hex[0..2]`, which is not a char
//! boundary in a multi-byte sentinel, and any component that transforms rather
//! than passes through a prop can fault the same way. Those are findings for the
//! `verify` gate to turn into `Status::Quarantined`, so they travel as
//! [`RenderResult::Panicked`] values. A panic that takes the whole process down
//! is a different thing and is reported as an error, because then the results
//! that did not get written are simply missing.
//!
//! # Cache
//!
//! Renders are cached at `<out_json's directory>/render-cache/<key>.json`, i.e.
//! `xtask/out/render-cache/` in the real pipeline. The key is a sha256 over:
//!
//! - the toolkit's `src_hash` (the same field `parse` puts in the IR), because a
//!   component's body can change without the harness changing a byte, and
//! - a hash of every file in the harness crate except its `target/` directory,
//!   which covers the IR decisions baked into the generated wrappers *and* the
//!   `Cargo.lock` that pins what they were rendered against.
//!
//! Cargo writes a `Cargo.lock` into the harness crate on the first build, so the
//! key a run starts with and the key the same inputs produce afterwards differ
//! exactly once. A run therefore files its output under both, and every run after
//! that hits.
//!
//! To bust it: delete `xtask/out/render-cache/` (or the one `<key>.json`), or set
//! `EONA_RENDER_NO_CACHE=1` for a run. Editing the toolkit's `src/**/*.rs` or
//! regenerating the harness changes the key on its own.
//!
//! # target/
//!
//! The child cargo is given `CARGO_TARGET_DIR=<harness>/../harness-target`
//! (`xtask/out/harness-target`), so the harness's build artifacts never land in
//! the toolkit's own `target/` and survive the harness directory being wiped and
//! regenerated — which matters, because the expensive part of this stage is
//! compiling yew and the toolkit in release mode, not rendering.
//!
//! Environment overrides: `EONA_RENDER_TIMEOUT_SECS` (default 1800),
//! `EONA_HARNESS_TARGET_DIR`, `EONA_RENDER_NO_CACHE`, `EONA_SRC_HASH`.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

/// A generous default: the first run of this stage compiles yew, the toolkit and
/// ~200 generated wrappers in release mode from cold.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(1800);

/// Compiler output is carried verbatim into the error, but a wall of it buries
/// the first `error[E….]` that actually matters.
const MAX_DIAGNOSTIC_LINES: usize = 400;

/// What the harness got out of one cell.
///
/// `Panicked` is a finding, not a failure — see the module docs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderResult {
    Html(String),
    Panicked(String),
}

impl RenderResult {
    pub fn html(&self) -> Option<&str> {
        match self {
            RenderResult::Html(h) => Some(h),
            RenderResult::Panicked(_) => None,
        }
    }

    pub fn is_panicked(&self) -> bool {
        matches!(self, RenderResult::Panicked(_))
    }

    /// The payload either way, for diagnostics that don't care which it is.
    pub fn message(&self) -> &str {
        match self {
            RenderResult::Html(h) => h,
            RenderResult::Panicked(m) => m,
        }
    }
}

impl Serialize for RenderResult {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let mut map = ser.serialize_map(Some(1))?;
        match self {
            RenderResult::Html(h) => map.serialize_entry("html", h)?,
            RenderResult::Panicked(m) => map.serialize_entry("panic", m)?,
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for RenderResult {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        de.deserialize_any(ResultVisitor)
    }
}

struct ResultVisitor;

impl<'de> Visitor<'de> for ResultVisitor {
    type Value = RenderResult;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(r#"a render result: {"html": "…"} or {"panic": "…"}"#)
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        Ok(RenderResult::Html(v.to_string()))
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut html: Option<String> = None;
        let mut panic: Option<String> = None;
        // `tag` is only consulted when neither payload key is present, so a
        // `{"status":"ok","html":…}` record is read off `html` rather than off a
        // tag whose vocabulary we would be guessing at.
        let mut tag: Option<String> = None;

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "html" | "ok" | "Html" | "rendered" | "markup" => {
                    html = Some(map.next_value()?);
                }
                "panic" | "panicked" | "Panicked" | "error" | "err" | "message" => {
                    panic = Some(map.next_value()?);
                }
                "status" | "kind" | "result" => {
                    tag = map.next_value::<Option<String>>()?;
                }
                _ => {
                    let _: serde::de::IgnoredAny = map.next_value()?;
                }
            }
        }

        match (html, panic, tag.as_deref()) {
            // A harness that writes both keys has a bug we cannot resolve from
            // here, and picking one would hide it.
            (Some(_), Some(_), _) => Err(serde::de::Error::custom(
                "render result carries both an html and a panic payload",
            )),
            (Some(h), None, _) => Ok(RenderResult::Html(h)),
            (None, Some(p), _) => Ok(RenderResult::Panicked(p)),
            (None, None, Some("panicked" | "panic" | "error")) => {
                Ok(RenderResult::Panicked(String::new()))
            }
            (None, None, Some("ok" | "html" | "rendered")) => Ok(RenderResult::Html(String::new())),
            (None, None, _) => Err(serde::de::Error::custom(
                "render result has neither an `html` nor a `panic` key",
            )),
        }
    }
}

/// What one invocation of this stage did, for the pipeline's own report.
#[derive(Clone, Debug)]
pub struct RenderRun {
    /// True when the harness was not rebuilt or run at all.
    pub cached: bool,
    pub cache_key: String,
    pub cache_path: PathBuf,
    pub out_json: PathBuf,
    pub target_dir: PathBuf,
    pub elapsed: Duration,
    /// `None` on a cache hit, and on a child killed by a signal.
    pub exit_code: Option<i32>,
    pub total: usize,
    pub ok: usize,
    pub panicked: usize,
}

impl RenderRun {
    pub fn summary(&self) -> String {
        let how = if self.cached { "cached" } else { "built + ran" };
        format!(
            "render: {how} in {:.1}s — {} cells, {} html, {} panicked\n  out   {}\n  cache {}\n  target {}",
            self.elapsed.as_secs_f64(),
            self.total,
            self.ok,
            self.panicked,
            self.out_json.display(),
            self.cache_path.display(),
            self.target_dir.display(),
        )
    }
}

/// Build and run the harness, and return every cell it rendered.
///
/// The toolkit's `src_hash` is resolved for the cache key, in order: the
/// `EONA_SRC_HASH` override, the `src_hash` field of the `ir.json` sitting next
/// to `out_json` (the IR the harness was generated from), and failing both, a
/// fresh `parse::collect` over the toolkit source.
pub fn render_all(harness_dir: &Path, out_json: &Path) -> Result<BTreeMap<String, RenderResult>> {
    Ok(render_all_reported(harness_dir, out_json)?.0)
}

/// [`render_all`] plus what it did, for callers that report on the stage.
pub fn render_all_reported(
    harness_dir: &Path,
    out_json: &Path,
) -> Result<(BTreeMap<String, RenderResult>, RenderRun)> {
    let src_hash = toolkit_src_hash(out_json)?;
    render_all_with(harness_dir, out_json, &src_hash)
}

/// [`render_all_reported`] with the toolkit hash supplied rather than resolved —
/// the entry point for a caller that already holds the `Toolkit` it generated the
/// harness from, and the one the tests drive.
pub fn render_all_with(
    harness_dir: &Path,
    out_json: &Path,
    src_hash: &str,
) -> Result<(BTreeMap<String, RenderResult>, RenderRun)> {
    let started = Instant::now();

    let manifest = harness_dir.join("Cargo.toml");
    if !manifest.is_file() {
        bail!(
            "no harness crate at {} — the `harness` stage writes its Cargo.toml there",
            manifest.display()
        );
    }
    if let Some(dir) = out_json.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating {}", dir.display()))?;
    }

    let key = cache_key(harness_dir, src_hash)?;
    let cache_path = cache_dir(out_json).join(format!("{key}.json"));

    if cache_enabled() {
        if let Some(text) = read_if_present(&cache_path)? {
            let results = parse_results(&text, &cache_path)?;
            // Downstream stages read `out_json`, not the cache, so a hit still
            // has to leave that file on disk and matching.
            std::fs::write(out_json, &text)
                .with_context(|| format!("writing {}", out_json.display()))?;
            let run = run_stats(&results, true, key, cache_path, out_json, harness_dir, None, started);
            return Ok((results, run));
        }
    }

    let target_dir = target_dir(harness_dir);
    let exit = run_harness(harness_dir, &manifest, out_json, &target_dir, timeout())?;

    let text = read_if_present(out_json)?.ok_or_else(|| {
        anyhow::anyhow!(
            "the harness at {} exited {} without writing {} — it is invoked as \
             `<bin> <out_json>` and must write its results to that path",
            manifest.display(),
            describe_exit(exit),
            out_json.display()
        )
    })?;

    let results = parse_results(&text, out_json)
        .with_context(|| format!("reading the harness output at {}", out_json.display()))?;

    if results.is_empty() {
        bail!(
            "the harness at {} wrote {} but it holds no render results; refusing to \
             hand an empty map to `verify` and `splice`, which would then report \
             success over nothing",
            manifest.display(),
            out_json.display()
        );
    }

    if cache_enabled() {
        let dir = cache_dir(out_json);
        std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
        std::fs::write(&cache_path, &text)
            .with_context(|| format!("writing {}", cache_path.display()))?;
        // The first build writes a Cargo.lock into the harness directory, which
        // changes the very hash the key is built from: file the result under the
        // post-run key too, or the next run misses its own output and rebuilds.
        let settled = cache_key(harness_dir, src_hash)?;
        if settled != key {
            std::fs::write(dir.join(format!("{settled}.json")), &text)
                .with_context(|| format!("writing {}", dir.join(format!("{settled}.json")).display()))?;
        }
    }

    let run =
        run_stats(&results, false, key, cache_path, out_json, harness_dir, exit, started);
    Ok((results, run))
}

#[allow(clippy::too_many_arguments)]
fn run_stats(
    results: &BTreeMap<String, RenderResult>,
    cached: bool,
    cache_key: String,
    cache_path: PathBuf,
    out_json: &Path,
    harness_dir: &Path,
    exit: Option<i32>,
    started: Instant,
) -> RenderRun {
    let panicked = results.values().filter(|r| r.is_panicked()).count();
    RenderRun {
        cached,
        cache_key,
        cache_path,
        out_json: out_json.to_path_buf(),
        target_dir: target_dir(harness_dir),
        elapsed: started.elapsed(),
        exit_code: exit,
        total: results.len(),
        ok: results.len() - panicked,
        panicked,
    }
}

// ------------------------------------------------------------------ running

/// Run `cargo run --manifest-path … --release -- <out_json>` and return its exit
/// code. A build failure, a non-zero exit with no usable output, or a timeout is
/// an error carrying the compiler's own output.
fn run_harness(
    harness_dir: &Path,
    manifest: &Path,
    out_json: &Path,
    target_dir: &Path,
    timeout: Duration,
) -> Result<Option<i32>> {
    // Absolute, because the child runs with `harness_dir` as its cwd.
    let out_arg = absolute(out_json)?;
    std::fs::create_dir_all(target_dir)
        .with_context(|| format!("creating {}", target_dir.display()))?;

    let mut cmd = Command::new(cargo_bin());
    cmd.arg("run")
        .arg("--manifest-path")
        .arg(manifest)
        .arg("--release")
        .arg("--")
        .arg(&out_arg)
        .current_dir(harness_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // xtask is itself run under `cargo run`, which exports CARGO_MANIFEST_DIR,
    // CARGO_PKG_* and friends. A nested cargo that inherits those resolves the
    // wrong package for build scripts and proc macros, so the child gets a clean
    // CARGO_* environment apart from CARGO_HOME (the shared registry cache).
    for (key, _) in std::env::vars_os() {
        let k = key.to_string_lossy();
        if k.starts_with("CARGO_") && k != "CARGO_HOME" {
            cmd.env_remove(&key);
        }
    }
    cmd.env("CARGO_TARGET_DIR", target_dir);

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning `{} run` for {}", cargo_bin().to_string_lossy(), manifest.display()))?;

    // Drained on threads: the harness prints per-cell progress and cargo prints
    // every diagnostic, and either can fill a 64K pipe buffer and deadlock a
    // child we are only polling for exit.
    let mut out_pipe = child.stdout.take().expect("stdout is piped");
    let mut err_pipe = child.stderr.take().expect("stderr is piped");
    let out_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = out_pipe.read_to_end(&mut buf);
        buf
    });
    let err_thread = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = err_pipe.read_to_end(&mut buf);
        buf
    });

    let started = Instant::now();
    let status = loop {
        match child.try_wait().context("waiting on the harness process")? {
            Some(status) => break status,
            None => {
                if started.elapsed() >= timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    let stderr = joined(err_thread);
                    bail!(
                        "the harness at {} was killed after {}s (raise \
                         EONA_RENDER_TIMEOUT_SECS if a cold release build needs longer)\n\
                         --- last output ---\n{}",
                        manifest.display(),
                        timeout.as_secs(),
                        tail(&stderr)
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
    };

    let stdout = joined(out_thread);
    let stderr = joined(err_thread);

    if !status.success() {
        // A compile error and a panicking harness both exit non-zero, and they
        // need different fixes, so say which one happened.
        if looks_like_build_failure(&stderr) {
            let hint = if stderr.contains("believes it's in a workspace") {
                "\nhint: the harness crate sits inside the toolkit's workspace; its \
                 Cargo.toml needs an empty `[workspace]` table to stand alone"
            } else {
                ""
            };
            bail!(
                "the harness crate at {} failed to build{hint}\n--- cargo ---\n{}",
                manifest.display(),
                tail(&stderr)
            );
        }
        // The harness may still have written a complete file before failing —
        // `render_all_with` reads it next and only errors if it did not.
        if !out_json.is_file() {
            bail!(
                "the harness at {} exited {} and wrote no results\n--- stderr ---\n{}\n--- stdout ---\n{}",
                manifest.display(),
                describe_exit(status.code()),
                tail(&stderr),
                tail(&stdout)
            );
        }
        eprintln!(
            "render: the harness exited {} but did write {}; continuing with what it wrote\n{}",
            describe_exit(status.code()),
            out_json.display(),
            tail(&stderr)
        );
    }

    Ok(status.code())
}

fn joined(handle: std::thread::JoinHandle<Vec<u8>>) -> String {
    match handle.join() {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    }
}

/// Cargo reports a failed compile on stderr; these are the phrases it uses that
/// a harness's own runtime panic never produces.
fn looks_like_build_failure(stderr: &str) -> bool {
    stderr.contains("could not compile")
        || stderr.contains("error: failed to")
        || stderr.contains("error[E")
        || stderr.contains("believes it's in a workspace")
        || stderr.contains("no targets specified")
        || stderr.contains("failed to parse manifest")
}

fn describe_exit(code: Option<i32>) -> String {
    match code {
        Some(c) => format!("with code {c}"),
        None => "on a signal".to_string(),
    }
}

fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= MAX_DIAGNOSTIC_LINES {
        return text.trim_end().to_string();
    }
    let skipped = lines.len() - MAX_DIAGNOSTIC_LINES;
    format!(
        "… {skipped} earlier lines elided …\n{}",
        lines[skipped..].join("\n").trim_end()
    )
}

fn cargo_bin() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn timeout() -> Duration {
    match std::env::var("EONA_RENDER_TIMEOUT_SECS").ok().and_then(|v| v.parse::<u64>().ok()) {
        Some(secs) => Duration::from_secs(secs),
        None => DEFAULT_TIMEOUT,
    }
}

/// Kept out of the toolkit's own `target/`, and kept *outside* the harness
/// directory so regenerating the harness does not throw away a release build of
/// yew. Defaults to `<harness>/../harness-target` — `xtask/out/harness-target`.
fn target_dir(harness_dir: &Path) -> PathBuf {
    if let Some(dir) = std::env::var_os("EONA_HARNESS_TARGET_DIR") {
        return PathBuf::from(dir);
    }
    match harness_dir.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join("harness-target"),
        _ => harness_dir.join("target"),
    }
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let cwd = std::env::current_dir().context("reading the current directory")?;
    Ok(cwd.join(path))
}

// -------------------------------------------------------------------- cache

fn cache_enabled() -> bool {
    !matches!(std::env::var("EONA_RENDER_NO_CACHE").as_deref(), Ok("1") | Ok("true"))
}

fn cache_dir(out_json: &Path) -> PathBuf {
    out_json.parent().unwrap_or_else(|| Path::new(".")).join("render-cache")
}

/// sha256 over the toolkit hash and every file of the harness crate. Both halves
/// are needed: the harness alone misses a change to a component's body, and the
/// toolkit alone misses a change to the cells the harness renders.
fn cache_key(harness_dir: &Path, src_hash: &str) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(b"eona-render-cache-v1\0");
    hasher.update(src_hash.as_bytes());
    hasher.update(b"\0release\0");
    hasher.update(harness_hash(harness_dir)?.as_bytes());
    Ok(format!("{:x}", hasher.finalize()))
}

/// sha256 over the harness crate's sorted `(relative path, bytes)` pairs, each
/// length-prefixed so a rename cannot collide with a content change — the same
/// construction `parse::src_hash` uses on the toolkit. `target/` is skipped: it
/// is output, and it is where this stage puts its build.
fn harness_hash(harness_dir: &Path) -> Result<String> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in WalkDir::new(harness_dir).sort_by_file_name() {
        let entry = entry.with_context(|| format!("walking {}", harness_dir.display()))?;
        let path = entry.path();
        let rel = path
            .strip_prefix(harness_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        if rel == "target" || rel.starts_with("target/") || rel.starts_with(".git") {
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
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

/// The toolkit hash for the cache key. Reading it from the IR next to `out_json`
/// is exact — that is the IR the harness was generated from — and re-parsing is
/// the fallback for a caller that never wrote one.
fn toolkit_src_hash(out_json: &Path) -> Result<String> {
    if let Ok(hash) = std::env::var("EONA_SRC_HASH") {
        if !hash.is_empty() {
            return Ok(hash);
        }
    }
    let ir = out_json.parent().unwrap_or_else(|| Path::new(".")).join("ir.json");
    if let Some(text) = read_if_present(&ir)? {
        let value: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("parsing {}", ir.display()))?;
        if let Some(hash) = value.get("src_hash").and_then(|h| h.as_str()) {
            return Ok(hash.to_string());
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask always has a parent")
        .to_path_buf();
    Ok(crate::parse::collect(&root)
        .with_context(|| {
            format!("hashing {} for the render cache key", root.join("src").display())
        })?
        .src_hash)
}

fn read_if_present(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
    }
}

// ------------------------------------------------------------------ parsing

/// Read the harness's JSON into the result map. See the module docs for the
/// canonical shape and the tolerated ones.
pub fn parse_results(text: &str, source: &Path) -> Result<BTreeMap<String, RenderResult>> {
    let value: serde_json::Value = serde_json::from_str(text)
        .with_context(|| format!("{} is not JSON", source.display()))?;

    // Unwrap a single-key envelope, so a harness that writes {"results": {…}} is
    // not a silent empty map.
    let value = match &value {
        serde_json::Value::Object(map) if map.len() == 1 => {
            let (key, inner) = map.iter().next().expect("len == 1");
            match key.as_str() {
                "results" | "cells" | "renders" | "rendered" => inner.clone(),
                _ => value,
            }
        }
        _ => value,
    };

    let mut out: BTreeMap<String, RenderResult> = BTreeMap::new();
    match value {
        serde_json::Value::Object(map) => {
            for (id, raw) in map {
                // The harness writes a `__meta` record (src_hash, cell count) so a
                // stale render cannot be spliced onto fresh source. It is not a
                // cell, and `verify::collect` bails on any id the harness plan does
                // not know, so it is dropped here rather than deserialised as one.
                if id.starts_with("__") {
                    continue;
                }
                let result: RenderResult = serde_json::from_value(raw)
                    .with_context(|| format!("render result for {id:?} in {}", source.display()))?;
                insert_unique(&mut out, id, result, source)?;
            }
        }
        serde_json::Value::Array(items) => {
            for (i, raw) in items.into_iter().enumerate() {
                let obj = raw.as_object().cloned().ok_or_else(|| {
                    anyhow::anyhow!(
                        "{}: entry {i} is not an object; an array of results must hold \
                         {{\"id\": …, \"html\"|\"panic\": …}} records",
                        source.display()
                    )
                })?;
                let id = ["id", "cell", "key", "name"]
                    .iter()
                    .find_map(|k| obj.get(*k).and_then(|v| v.as_str()))
                    .ok_or_else(|| {
                        anyhow::anyhow!("{}: entry {i} has no `id`", source.display())
                    })?
                    .to_string();
                let result: RenderResult =
                    serde_json::from_value(serde_json::Value::Object(obj)).with_context(|| {
                        format!("render result for {id:?} in {}", source.display())
                    })?;
                insert_unique(&mut out, id, result, source)?;
            }
        }
        other => bail!(
            "{}: expected an object keyed by cell id, found {}",
            source.display(),
            kind_of(&other)
        ),
    }
    Ok(out)
}

fn insert_unique(
    out: &mut BTreeMap<String, RenderResult>,
    id: String,
    result: RenderResult,
    source: &Path,
) -> Result<()> {
    if let Some(existing) = out.get(&id) {
        if existing != &result {
            bail!(
                "{}: two different results for cell {id:?}; cell ids must be unique, \
                 so `harness` is qualifying two cells the same way",
                source.display()
            );
        }
        return Ok(());
    }
    out.insert(id, result);
    Ok(())
}

fn kind_of(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

// ---------------------------------------------------------------- reporting

/// The component half of a cell id, for reports only. The pipeline writes ids as
/// `Component/cell-id` because `matrix::Cell::id` is unique only within a
/// component; an id with no separator is its own component.
pub fn component_of(id: &str) -> &str {
    match id.find(['/', '#']) {
        Some(i) => &id[..i],
        None => id,
    }
}

/// Results grouped by component, in id order.
pub fn by_component(
    results: &BTreeMap<String, RenderResult>,
) -> BTreeMap<&str, Vec<(&str, &RenderResult)>> {
    let mut out: BTreeMap<&str, Vec<(&str, &RenderResult)>> = BTreeMap::new();
    for (id, result) in results {
        out.entry(component_of(id)).or_default().push((id.as_str(), result));
    }
    out
}

/// Every cell that panicked, as `(id, message)` — what `verify` turns into
/// `Status::Quarantined`.
pub fn panics(results: &BTreeMap<String, RenderResult>) -> Vec<(&str, &str)> {
    results
        .iter()
        .filter_map(|(id, r)| match r {
            RenderResult::Panicked(m) => Some((id.as_str(), m.as_str())),
            RenderResult::Html(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "xtask-render-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn p(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    #[test]
    fn canonical_shape_round_trips() {
        let mut map = BTreeMap::new();
        map.insert("Button/base".to_string(), RenderResult::Html("<button/>".into()));
        map.insert("Swatch/base".to_string(), RenderResult::Panicked("char boundary".into()));
        let text = serde_json::to_string(&map).unwrap();
        assert_eq!(parse_results(&text, &p("x.json")).unwrap(), map);
    }

    #[test]
    fn tolerates_the_shapes_a_harness_writes_by_accident() {
        // A `results` envelope.
        let wrapped = r#"{"results":{"Button/base":{"html":"<button/>"}}}"#;
        assert_eq!(
            parse_results(wrapped, &p("x.json")).unwrap()["Button/base"],
            RenderResult::Html("<button/>".into())
        );
        // An array of records.
        let array = r#"[{"id":"Button/base","html":"<button/>"},
                        {"id":"Swatch/base","panic":"boom"}]"#;
        let got = parse_results(array, &p("x.json")).unwrap();
        assert_eq!(got["Swatch/base"], RenderResult::Panicked("boom".into()));
        // Bare strings, and serde's own externally tagged enum.
        let bare = r#"{"A/base":"<i/>","B/base":{"Panicked":"boom"},"C/base":{"Html":"<b/>"}}"#;
        let got = parse_results(bare, &p("x.json")).unwrap();
        assert_eq!(got["A/base"], RenderResult::Html("<i/>".into()));
        assert_eq!(got["B/base"], RenderResult::Panicked("boom".into()));
        assert_eq!(got["C/base"], RenderResult::Html("<b/>".into()));
    }

    #[test]
    fn a_result_with_no_payload_is_an_error_not_an_empty_string() {
        let err = parse_results(r#"{"A/base":{"note":"hi"}}"#, &p("x.json")).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("neither an `html` nor a `panic`"), "{msg}");
        assert!(msg.contains("\"A/base\""), "{msg}");
    }

    #[test]
    fn colliding_cell_ids_are_an_error() {
        // Only reachable through the array form; a JSON object cannot repeat a key.
        let array = r#"[{"id":"A/base","html":"<i/>"},{"id":"A/base","html":"<b/>"}]"#;
        let err = parse_results(array, &p("x.json")).unwrap_err();
        assert!(format!("{err:#}").contains("two different results"), "{err:#}");
    }

    #[test]
    fn sentinels_survive_the_json_round_trip() {
        // The whole mechanism depends on \u{E000}0\u{E001} arriving at `splice`
        // byte-identical, and JSON is the only transport between the two.
        let cell = format!("<button>{}</button>", "\u{E000}0\u{E001}");
        let mut map = BTreeMap::new();
        map.insert("Button/base".to_string(), RenderResult::Html(cell.clone()));
        let text = serde_json::to_string(&map).unwrap();
        assert_eq!(parse_results(&text, &p("x.json")).unwrap()["Button/base"].html(), Some(&*cell));
    }

    #[test]
    fn component_ids_split_on_either_separator() {
        assert_eq!(component_of("Button/href_some"), "Button");
        assert_eq!(component_of("Button#href_some"), "Button");
        assert_eq!(component_of("Button"), "Button");
    }

    #[test]
    fn harness_hash_ignores_target_and_tracks_sources() {
        let dir = scratch("hash");
        std::fs::write(dir.join("Cargo.toml"), "[package]\n").unwrap();
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src/main.rs"), "fn main() {}").unwrap();
        let before = harness_hash(&dir).unwrap();

        std::fs::create_dir_all(dir.join("target/release")).unwrap();
        std::fs::write(dir.join("target/release/harness"), "binary").unwrap();
        assert_eq!(harness_hash(&dir).unwrap(), before, "target/ must not key the cache");

        std::fs::write(dir.join("src/main.rs"), "fn main() { println!() }").unwrap();
        assert_ne!(harness_hash(&dir).unwrap(), before, "a source edit must bust the cache");

        // The toolkit half of the key matters on its own: the same harness
        // renders different HTML when a component's body changes.
        assert_ne!(cache_key(&dir, "aaa").unwrap(), cache_key(&dir, "bbb").unwrap());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_harness_names_the_manifest_it_wanted() {
        let dir = scratch("absent");
        let err = render_all_with(&dir.join("harness"), &dir.join("render.json"), "hash")
            .unwrap_err();
        assert!(format!("{err:#}").contains("Cargo.toml"), "{err:#}");
        std::fs::remove_dir_all(&dir).ok();
    }

    // ---- the seam: a real cargo child ------------------------------------
    //
    // These build an actual crate. They are dependency-free on purpose, so they
    // cost a second or two and never touch the network — the point is to prove
    // the process plumbing (argv, cwd, CARGO_TARGET_DIR, exit handling, cache),
    // not to re-render the toolkit.

    fn fixture(dir: &Path, main: &str) {
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"harness-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[workspace]\n",
        )
        .unwrap();
        std::fs::write(dir.join("src/main.rs"), main).unwrap();
    }

    const WRITES_TWO_CELLS: &str = r#"
fn main() {
    let out = std::env::args().nth(1).expect("<out_json>");
    std::fs::write(&out, "{\"Button/base\":{\"html\":\"<button/>\"},\"Swatch/base\":{\"panic\":\"boom\"}}").unwrap();
}
"#;

    #[test]
    fn builds_runs_and_then_caches() {
        let root = scratch("e2e");
        let harness = root.join("harness");
        fixture(&harness, WRITES_TWO_CELLS);
        let out = root.join("out/render.json");

        let (results, run) = render_all_with(&harness, &out, "src-hash-1").unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results["Button/base"].html(), Some("<button/>"));
        assert!(results["Swatch/base"].is_panicked());
        assert!(!run.cached);
        assert_eq!(run.panicked, 1);
        assert!(out.is_file(), "downstream stages read {}", out.display());
        // The build must land in xtask/out/, never in the toolkit's target/.
        assert_eq!(run.target_dir, root.join("harness-target"));
        assert!(run.target_dir.is_dir());
        assert!(run.cache_path.starts_with(root.join("out/render-cache")));

        // A second run with the same inputs must not rebuild. Deleting the
        // binary is what proves it: a rebuild would put it back.
        std::fs::remove_dir_all(root.join("harness-target")).unwrap();
        std::fs::remove_file(&out).unwrap();
        let (again, run2) = render_all_with(&harness, &out, "src-hash-1").unwrap();
        assert!(run2.cached);
        assert_eq!(again, results);
        assert!(out.is_file(), "a cache hit still has to leave {}", out.display());
        assert!(!root.join("harness-target").exists(), "a cache hit must not build");

        // A different toolkit hash is a different key, so it does rebuild.
        let (_, run3) = render_all_with(&harness, &out, "src-hash-2").unwrap();
        assert!(!run3.cached);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_build_failure_carries_the_compiler_output() {
        let root = scratch("broken");
        let harness = root.join("harness");
        fixture(&harness, "fn main() { let x: u32 = \"not a number\"; }");
        let err = render_all_with(&harness, &root.join("out/render.json"), "hash").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("failed to build"), "{msg}");
        assert!(msg.contains("E0308"), "the compiler output must survive: {msg}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_harness_that_writes_nothing_is_an_error_not_an_empty_map() {
        let root = scratch("silent");
        let harness = root.join("harness");
        fixture(&harness, "fn main() {}");
        let err = render_all_with(&harness, &root.join("out/render.json"), "hash").unwrap_err();
        assert!(format!("{err:#}").contains("without writing"), "{err:#}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn an_empty_result_set_is_an_error() {
        let root = scratch("empty");
        let harness = root.join("harness");
        fixture(
            &harness,
            "fn main() { std::fs::write(std::env::args().nth(1).unwrap(), \"{}\").unwrap(); }",
        );
        let err = render_all_with(&harness, &root.join("out/render.json"), "hash").unwrap_err();
        assert!(format!("{err:#}").contains("no render results"), "{err:#}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_hung_harness_is_killed_and_reported() {
        let root = scratch("hang");
        let harness = root.join("harness");
        fixture(&harness, "fn main() { loop { std::thread::sleep(std::time::Duration::from_secs(1)); } }");
        let out = root.join("out/render.json");
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        let target = target_dir(&harness);
        // Built first, so the short timeout below is timing the run and can
        // never be a slow compile on a loaded machine.
        let built = Command::new(cargo_bin())
            .args(["build", "--release", "--manifest-path"])
            .arg(harness.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", &target)
            .status()
            .unwrap();
        assert!(built.success());
        let err = run_harness(
            &harness,
            &harness.join("Cargo.toml"),
            &out,
            &target,
            Duration::from_secs(5),
        );
        let msg = format!("{:#}", err.unwrap_err());
        assert!(msg.contains("was killed after"), "{msg}");
        assert!(msg.contains("EONA_RENDER_TIMEOUT_SECS"), "{msg}");
        std::fs::remove_dir_all(&root).ok();
    }
}
