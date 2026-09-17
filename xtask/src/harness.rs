//! Stage: `harness`. See xtask/README.md for the pipeline contract.
//!
//! Generates a throwaway binary crate that renders every in-scope component
//! once per render cell and dumps the resulting HTML as JSON. This is the stage
//! that makes the whole generator work without a Rust -> TSX transpiler: the
//! markup React will ship is produced by Yew's own SSR renderer, so it cannot
//! drift from what the Yew components actually emit.
//!
//! The trick is that every *text* leaf of every prop is filled with a
//! private-use sentinel (`\u{E000}<n>\u{E001}`) rather than a plausible value.
//! Yew escapes attribute values with `html_escape::encode_double_quoted_attribute`
//! and text nodes with `html_escape::encode_text`, both of which touch only
//! `& < > "`, so a sentinel arrives in the output byte-for-byte and `splice` can
//! find where each prop landed. Structural choices (present/absent, enum
//! variant, list arity, nested struct fields) are not sentinels — they are the
//! render cells `matrix` derived, one wrapper component each.
//!
//! Three things the generated crate does that are not obvious:
//!
//! - `hydratable(false)`. Without it every Yew component is wrapped in
//!   `<!--<[path]>-->` / `<!--</[path]>-->` hydration markers, which are noise
//!   the splicer would have to strip and which encode the *Yew* component tree,
//!   not the DOM.
//! - Each render runs on its own thread, inside `catch_unwind`. `Swatch` is
//!   *expected* to panic: `src/molecules/swatch.rs:11` slices `&hex[0..2]` and a
//!   sentinel's first char is three bytes, so byte 2 is not a char boundary.
//!   The verify gate needs that panic as a signal (it is what quarantines
//!   `Swatch`), not as a dead process, and a fresh thread also keeps Yew's
//!   thread-local renderer state from carrying a half-unwound render into the
//!   next cell.
//! - The wrappers name **components only**, never `*Props` types: 36 of the
//!   crate's 39 props structs are not re-exported from the crate root (only
//!   `OntoAnnotationProps`, `DatasetCardProps` and `OntoBadgeProps` are), so
//!   element syntax — `html! { <Button label={..}></Button> }` — is the only
//!   form that compiles for all 38.
//!
//! The mapping from sentinel index back to a prop path is not recoverable from
//! the HTML, so it is written next to the crate as `plan.json` and is also
//! available in-process from [`plan`]. `splice` must read one of the two rather
//! than re-deriving it: the traversal order that assigns indices lives here.

#![allow(dead_code, unused_imports)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path as FsPath, PathBuf};

use anyhow::{bail, Context, Result};
use proc_macro2::{Ident, Literal, Span as PmSpan, TokenStream};
use quote::{quote, ToTokens};
use serde::{Deserialize, Serialize};

use crate::ir::*;
use crate::matrix::{Cell, Selection};
use crate::verify::Alphabet;

// ---------------------------------------------------------------- sentinels

/// Private-use area, so nothing in the toolkit's own markup or CSS can collide
/// with it and no escaping rule in `html_escape` rewrites it.
pub const SENTINEL_OPEN: char = '\u{E000}';
pub const SENTINEL_CLOSE: char = '\u{E001}';

/// Marks where a `Slot` prop (`Html` / `Children`) was rendered, so `splice`
/// knows where React `children` — or a render prop — has to go.
pub const SLOT_TAG: &str = "template";
pub const SLOT_ATTR: &str = "data-eona-slot";

/// The text written into the `n`th text leaf of a cell.
pub fn sentinel(n: usize) -> String {
    format!("{SENTINEL_OPEN}{n}{SENTINEL_CLOSE}")
}

/// Reads a sentinel's index back. `splice` matches on the delimiters itself
/// while scanning; this is for the exact-match case (a whole attribute value).
pub fn sentinel_index(s: &str) -> Option<usize> {
    let inner = s.strip_prefix(SENTINEL_OPEN)?.strip_suffix(SENTINEL_CLOSE)?;
    inner.parse().ok()
}

/// The marker element the harness renders for slot `n`, as it appears in the
/// SSR output. Exposed so `splice` and the tests agree on one spelling.
pub fn slot_marker(n: usize) -> String {
    format!("<{SLOT_TAG} {SLOT_ATTR}=\"{n}\"></{SLOT_TAG}>")
}

// ---------------------------------------------------------------- the plan

/// One leaf the harness filled: which sentinel (or slot) index it got, which
/// prop path it came from, and the declaration a human can open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub index: usize,
    /// Concrete path with list positions resolved — `colors[1].hex`, not the
    /// matrix's element-uniform `colors[].hex`.
    pub path: String,
    /// The matrix axis path this leaf sits under, or `None` when the leaf is
    /// not itself an axis (a plain text field never branches the markup).
    pub axis: Option<String>,
    pub rust_ty: String,
    pub span: Span,
}

/// One rendered cell: the wrapper that produces it and everything spliced back
/// into it afterwards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellPlan {
    pub component: String,
    pub cell: String,
    /// `"<component>/<cell-id>"` — the key in the render JSON.
    pub key: String,
    /// The generated zero-prop component that renders this cell.
    pub wrapper: String,
    /// The component's IR status, so `verify` can tell a `NeedsOverride` render
    /// (Modal, `src/molecules/modal.rs`) from a plain one without re-reading the IR.
    pub status: Status,
    /// The cell's choices, formatted the way `matrix::Choice` displays them.
    pub choices: Vec<String>,
    pub sentinels: Vec<Binding>,
    pub slots: Vec<Binding>,
    /// Anything the builder had to assume. Non-empty means a leaf was filled
    /// without a matrix axis backing it, which is exactly the silent-wrongness
    /// the nested-axis rule exists to prevent, so it is recorded rather than
    /// swallowed.
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessPlan {
    /// Copied from the IR so a render JSON can be proven to match the source it
    /// came from.
    pub src_hash: String,
    pub sentinel_open: String,
    pub sentinel_close: String,
    pub slot_tag: String,
    pub slot_attr: String,
    pub cells: Vec<CellPlan>,
}

impl HarnessPlan {
    pub fn cell(&self, key: &str) -> Option<&CellPlan> {
        self.cells.iter().find(|c| c.key == key)
    }
    /// Every note across the run, prefixed with the cell it came from.
    pub fn notes(&self) -> Vec<String> {
        self.cells
            .iter()
            .flat_map(|c| c.notes.iter().map(move |n| format!("{}: {n}", c.key)))
            .collect()
    }
}

/// One entry of the render JSON. Exactly one of `html` / `panic` is set;
/// `render` and `verify` should read this through [`read_renders`] rather than
/// matching the JSON shape themselves, because that shape is decided here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rendered {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panic: Option<String>,
    /// `file:line:col` of the panic, from the panic hook — the span the verify
    /// gate quotes at a human (`src/molecules/swatch.rs:11` for `Swatch`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

impl Rendered {
    pub fn is_panic(&self) -> bool {
        self.panic.is_some()
    }
    /// The panic reason as the verify gate wants to print it.
    pub fn panic_reason(&self) -> Option<String> {
        let msg = self.panic.as_deref()?;
        Some(match &self.at {
            Some(at) => format!("{msg} ({at})"),
            None => msg.to_string(),
        })
    }
}

/// Reads back what the generated binary wrote to its argv path.
pub fn read_renders(path: &FsPath) -> Result<BTreeMap<String, Rendered>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading the harness output {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("parsing the harness output {}", path.display()))
}

/// The cells the harness renders: every component the IR did not exclude.
/// Thin wrapper over [`crate::matrix::all_cells`] so a driver does not have to
/// reshape the tuple, and so the harness and the matrix can never disagree
/// about which components are in scope.
pub fn cells_for(tk: &Toolkit) -> Result<BTreeMap<String, Vec<Cell>>> {
    Ok(crate::matrix::all_cells(tk)?
        .into_iter()
        .map(|(name, cells, _)| (name, cells))
        .collect())
}

// ---------------------------------------------------------------- entry point

/// Writes the harness crate (`Cargo.toml`, `src/main.rs`) and its `plan.json`
/// into `out`.
pub fn emit_harness(
    tk: &Toolkit,
    cells: &BTreeMap<String, Vec<Cell>>,
    out: &FsPath,
) -> Result<()> {
    let plan = plan(tk, cells)?;
    let main_rs = main_rs(tk, cells, &plan)?;

    std::fs::create_dir_all(out.join("src"))
        .with_context(|| format!("creating {}", out.join("src").display()))?;

    // Canonicalising only now: `out` may not have existed a moment ago, and the
    // path dependency below has to resolve from the file's own directory.
    let out_abs = out
        .canonicalize()
        .with_context(|| format!("canonicalising {}", out.display()))?;
    let toolkit = toolkit_root();
    let toolkit_abs = toolkit
        .canonicalize()
        .with_context(|| format!("canonicalising {}", toolkit.display()))?;

    std::fs::write(out.join("Cargo.toml"), cargo_toml(&out_abs, &toolkit_abs)?)
        .with_context(|| format!("writing {}", out.join("Cargo.toml").display()))?;
    std::fs::write(out.join("src/main.rs"), main_rs)
        .with_context(|| format!("writing {}", out.join("src/main.rs").display()))?;
    // Its own workspace declares its own build artefacts; keeping them out of
    // git matters because `out/` is only ignored one directory up (xtask/.gitignore).
    std::fs::write(out.join(".gitignore"), "target/\nCargo.lock\n")
        .with_context(|| format!("writing {}", out.join(".gitignore").display()))?;

    let plan_json = serde_json::to_string_pretty(&plan).context("serialising the harness plan")?;
    std::fs::write(out.join("plan.json"), format!("{plan_json}\n"))
        .with_context(|| format!("writing {}", out.join("plan.json").display()))?;

    for note in plan.notes() {
        eprintln!("harness: assumed a value with no matrix axis behind it — {note}");
    }
    Ok(())
}

/// The sentinel/slot mapping, without writing anything. `emit_harness` writes
/// this to `plan.json`; `splice` can call it directly.
pub fn plan(tk: &Toolkit, cells: &BTreeMap<String, Vec<Cell>>) -> Result<HarnessPlan> {
    let mut out = HarnessPlan {
        src_hash: tk.src_hash.clone(),
        sentinel_open: SENTINEL_OPEN.to_string(),
        sentinel_close: SENTINEL_CLOSE.to_string(),
        slot_tag: SLOT_TAG.to_string(),
        slot_attr: SLOT_ATTR.to_string(),
        cells: Vec::new(),
    };
    for (name, cells) in cells {
        let c = component(tk, name)?;
        for cell in cells {
            out.cells.push(Builder::new(tk, c, cell).run()?.0);
        }
    }
    Ok(out)
}

/// The toolkit crate root, the same way `main.rs` derives it: xtask's parent.
/// Used for the generated crate's path dependency, so the harness always points
/// at the source the IR was parsed from.
fn toolkit_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask always has a parent directory")
        .to_path_buf()
}

fn component<'a>(tk: &'a Toolkit, name: &str) -> Result<&'a Component> {
    let Some(c) = tk.component(name) else {
        bail!("the matrix produced cells for `{name}`, which is not in the IR");
    };
    // An excluded component reaching the harness means two stages disagree about
    // scope, and the harness would then emit a wrapper for something the ABI
    // says was never generated. Fail loudly rather than render it.
    if let Status::Excluded { reason } = &c.status {
        bail!(
            "{}:{}: `{name}` is Excluded ({reason}) but the matrix produced cells for it; \
             the scope rule and the cell enumerator disagree",
            c.span.file,
            c.span.line,
        );
    }
    Ok(c)
}

// ---------------------------------------------------------------- code gen

/// `Cargo.toml` for the generated crate.
///
/// `[workspace]` with no members is load-bearing: `out/harness/` sits inside the
/// toolkit's directory tree, and the root `Cargo.toml` lists `members = [".",
/// "xtask"]`, so without its own workspace table cargo refuses to build it —
/// and the root manifest is one of the files this generator must not touch.
fn cargo_toml(out_abs: &FsPath, toolkit_abs: &FsPath) -> Result<String> {
    let dep_path = relative(out_abs, toolkit_abs);
    let yew = yew_requirement(toolkit_abs)?;
    Ok(format!(
        "# @generated by xtask/src/harness.rs — do not edit; `cargo xtask bridgegen` rewrites it.\n\
         #\n\
         # Its own workspace on purpose: the toolkit's root Cargo.toml lists\n\
         # `members = [\".\", \"xtask\"]` and is not ours to modify.\n\
         [workspace]\n\
         \n\
         [package]\n\
         name = \"eona-ui-harness\"\n\
         version = \"0.0.0\"\n\
         edition = \"2021\"\n\
         publish = false\n\
         \n\
         [[bin]]\n\
         name = \"eona-ui-harness\"\n\
         path = \"src/main.rs\"\n\
         \n\
         [dependencies]\n\
         # `ssr` only: the toolkit is presentational, and the harness renders its\n\
         # 38 in-scope components to HTML strings. `csr` would buy nothing here.\n\
         eona-ui-toolkit = {{ path = \"{dep_path}\", features = [\"ssr\"] }}\n\
         # Kept in step with the toolkit's own requirement so cargo unifies the two\n\
         # — a second yew would make `BaseComponent` a different trait.\n\
         yew = {{ version = \"{yew}\", features = [\"ssr\"] }}\n\
         # `rt` alone: LocalSet lives there, and LocalServerRenderer needs no timers.\n\
         tokio = {{ version = \"1\", features = [\"rt\"] }}\n\
         serde_json = {{ version = \"1\", features = [\"preserve_order\"] }}\n",
    ))
}

/// The toolkit's own `yew` version requirement, read out of its manifest rather
/// than hard-coded: a bump there that the harness did not follow would resolve
/// to two yews and fail with an inscrutable trait mismatch.
fn yew_requirement(toolkit_abs: &FsPath) -> Result<String> {
    let manifest = toolkit_abs.join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .with_context(|| format!("reading {}", manifest.display()))?;
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("yew") else { continue };
        let Some(rest) = rest.trim_start().strip_prefix('=') else { continue };
        let rest = rest.trim();
        // `yew = "0.23"` — the plain form. The dev-dependency uses a table and is
        // skipped, which is right: the harness wants the library requirement.
        if let Some(v) = rest.strip_prefix('"').and_then(|r| r.split('"').next()) {
            return Ok(v.to_string());
        }
    }
    bail!(
        "{}: no `yew = \"...\"` line under [dependencies]; the harness cannot pin \
         the same yew the toolkit compiles against",
        manifest.display()
    )
}

/// `to` expressed relative to `from`, both absolute. Cargo path dependencies
/// are resolved against the manifest's directory, and a relative one keeps the
/// generated manifest identical on every machine — which is what lets `check`
/// diff a regenerated tree.
fn relative(from: &FsPath, to: &FsPath) -> String {
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let shared = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut segs: Vec<String> = std::iter::repeat("..".to_string())
        .take(from.len() - shared)
        .collect();
    segs.extend(to[shared..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    if segs.is_empty() {
        ".".to_string()
    } else {
        segs.join("/")
    }
}

/// The sentinel text, precomputed for every index and both alphabets.
///
/// Built by calling [`Alphabet::sentinel`] rather than by formatting the
/// delimiters here, because the gate searches the markup for exactly what that
/// function returns (xtask/src/verify.rs:118) and a second spelling of the same
/// format string is how the two would drift apart. A table also means the
/// generated `sent` returns `&'static str` without leaking, which the three
/// `&'static str` props require.
fn sentinel_table(plan: &HarnessPlan) -> TokenStream {
    let width = plan.cells.iter().map(|c| c.sentinels.len()).max().unwrap_or(0);
    let rows = (0..width).map(|i| {
        let cols = Alphabet::ALL.map(|a| Literal::string(&a.sentinel(i as u32)));
        let cols = cols.iter();
        quote! { [#(#cols),*] }
    });
    let width_lit = Literal::usize_unsuffixed(width);
    let n_alpha = Literal::usize_unsuffixed(Alphabet::ALL.len());
    let names = Alphabet::ALL.map(|a| Literal::string(a.name()));
    let names = names.iter();

    quote! {
        /// Row = sentinel index, column = alphabet. Generated from
        /// `verify::Alphabet::sentinel`, so the planted text and the text the
        /// verify gate scans for are the same bytes by construction.
        static SENTINELS: [[&str; #n_alpha]; #width_lit] = [#(#rows),*];

        /// The alphabet names, in the same column order as `SENTINELS`. Written
        /// into each render's key as `<Component>/<cell>#<name>`.
        static ALPHABETS: [&str; #n_alpha] = [#(#names),*];

        /// Which column `sent` reads. A process global rather than a
        /// thread-local: `render_one` spawns a fresh thread per cell (a panic
        /// unwinding through yew's renderer must not be inherited by the next
        /// one), so the choice has to cross that boundary. Renders are
        /// sequential, so there is no race to lose.
        static ALPHABET: ::std::sync::atomic::AtomicUsize =
            ::std::sync::atomic::AtomicUsize::new(0);

        /// The placeholder for text leaf `n` of the cell currently rendering.
        fn sent(n: usize) -> &'static str {
            SENTINELS[n][ALPHABET.load(::std::sync::atomic::Ordering::SeqCst)]
        }
    }
}

/// The generated `src/main.rs`, formatted.
fn main_rs(
    tk: &Toolkit,
    cells: &BTreeMap<String, Vec<Cell>>,
    plan: &HarnessPlan,
) -> Result<String> {
    let mut wrappers = TokenStream::new();
    let mut calls = TokenStream::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for (name, comp_cells) in cells {
        let c = component(tk, name)?;
        for cell in comp_cells {
            let (cell_plan, body) = Builder::new(tk, c, cell).run()?;
            if !seen.insert(cell_plan.wrapper.clone()) {
                bail!(
                    "{}:{}: two cells both want the wrapper `{}`; cell ids are supposed to \
                     be unique within a component (see matrix::cell_id)",
                    c.span.file,
                    c.span.line,
                    cell_plan.wrapper,
                );
            }
            let wrapper = ident(&cell_plan.wrapper);
            let func = ident(&format!("render_{}", to_snake(&cell_plan.wrapper)));
            let key = Literal::string(&cell_plan.key);
            let doc = Literal::string(&format!(
                " {} / {} — {}",
                cell_plan.component,
                cell_plan.cell,
                if cell_plan.choices.is_empty() {
                    "no structural axes".to_string()
                } else {
                    cell_plan.choices.join(", ")
                },
            ));
            wrappers.extend(quote! {
                #[doc = #doc]
                #[::yew::function_component(#wrapper)]
                fn #func() -> ::yew::Html {
                    #body
                }
            });
            calls.extend(quote! {
                render_both::<#wrapper>(&mut out_map, #key);
            });
        }
    }

    let count = Literal::usize_unsuffixed(plan.cells.len());
    let renders = Literal::usize_unsuffixed(plan.cells.len() * Alphabet::ALL.len());
    let src_hash = Literal::string(&plan.src_hash);
    let sentinels = sentinel_table(plan);

    let file: syn::File = syn::parse2(quote! {
        #![allow(non_camel_case_types, non_snake_case, unused_imports)]

        // Glob imports, and only components are ever named below: 37 of the 48
        // `*Props` structs are private to their modules, so nothing here may
        // mention one. `eona_ui_toolkit::*` re-exports atoms/molecules/organisms
        // (src/lib.rs); `ontology::*` carries the model types the ontology
        // components are typed on (`LiteralValue`, `TermKind`) and is a separate
        // module, so it needs its own glob.
        use ::eona_ui_toolkit::ontology::*;
        use ::eona_ui_toolkit::*;
        use ::yew::prelude::*;

        /// The IR this harness was generated from. Written into the output so a
        /// stale render can never be spliced onto fresh source.
        const SRC_HASH: &str = #src_hash;

        #sentinels

        #wrappers

        fn main() {
            let Some(out) = ::std::env::args().nth(1) else {
                eprintln!("usage: eona-ui-harness <renders.json>");
                ::std::process::exit(2);
            };
            let out = ::std::path::PathBuf::from(out);

            install_panic_capture();

            let mut out_map = ::serde_json::Map::new();
            out_map.insert(
                "__meta".to_string(),
                ::serde_json::json!({
                    "src_hash": SRC_HASH,
                    "cells": #count,
                    "renders": #renders,
                }),
            );
            #calls

            let json = ::serde_json::to_string_pretty(&::serde_json::Value::Object(out_map))
                .expect("serialising the render map");
            if let Err(e) = ::std::fs::write(&out, format!("{json}\n")) {
                eprintln!("harness: writing {}: {e}", out.display());
                ::std::process::exit(1);
            }
            eprintln!(
                "harness: {} cells x {} alphabets = {} renders -> {}",
                #count,
                ALPHABETS.len(),
                #renders,
                out.display(),
            );
        }

        /// One cell, once per alphabet. The two renders differ only in the text
        /// `sent` hands the component, so any difference in the normalised markup
        /// is the component reading the prop's bytes rather than passing it
        /// through — which is the whole point of check 2 (xtask/src/verify.rs:22).
        fn render_both<W>(out: &mut ::serde_json::Map<String, ::serde_json::Value>, key: &str)
        where
            W: ::yew::BaseComponent + 'static,
            W::Properties: ::std::default::Default,
        {
            for (i, name) in ALPHABETS.iter().enumerate() {
                ALPHABET.store(i, ::std::sync::atomic::Ordering::SeqCst);
                let id = format!("{key}#{name}");
                let value = render_one::<W>(&id);
                out.insert(id, value);
            }
        }

        /// Renders one wrapper in isolation and reports the outcome as JSON.
        ///
        /// Its own thread per cell: `Swatch` panics by design here
        /// (`src/molecules/swatch.rs:11` slices `&hex[0..2]`, and a sentinel's
        /// first char is three bytes wide), and a panic unwinding through Yew's
        /// renderer leaves thread-local state that the next cell would inherit.
        fn render_one<W>(key: &str) -> ::serde_json::Value
        where
            W: ::yew::BaseComponent + 'static,
            W::Properties: ::std::default::Default,
        {
            let spawned = ::std::thread::Builder::new()
                .name(key.to_owned())
                // Deeply nested `html!` expansions recurse; the default 2 MiB is
                // enough for this crate but leaves no headroom for a bigger one.
                .stack_size(16 * 1024 * 1024)
                .spawn(render_isolated::<W>);

            let outcome = match spawned {
                Ok(handle) => match handle.join() {
                    Ok(outcome) => outcome,
                    // A panic `catch_unwind` did not see (one raised while
                    // unwinding, or an abort-adjacent path). Recorded, not lost.
                    Err(payload) => Err((payload_message(&*payload), None)),
                },
                Err(e) => Err((format!("could not spawn a render thread: {e}"), None)),
            };

            match outcome {
                Ok(html) => ::serde_json::json!({ "html": html }),
                Err((message, at)) => {
                    eprintln!(
                        "harness: {key}: PANIC {message}{}",
                        at.as_deref().map(|a| format!(" at {a}")).unwrap_or_default(),
                    );
                    let mut entry = ::serde_json::Map::new();
                    entry.insert("panic".to_string(), ::serde_json::Value::String(message));
                    if let Some(at) = at {
                        entry.insert("at".to_string(), ::serde_json::Value::String(at));
                    }
                    ::serde_json::Value::Object(entry)
                }
            }
        }

        /// `hydratable(false)` is what makes the output spliceable: with it on,
        /// every component is wrapped in `<!--<[..]>-->` markers describing Yew's
        /// own tree, which is not the DOM React has to reproduce.
        fn render_isolated<W>() -> ::std::result::Result<String, (String, Option<String>)>
        where
            W: ::yew::BaseComponent + 'static,
            W::Properties: ::std::default::Default,
        {
            let rendered = ::std::panic::catch_unwind(|| {
                let rt = ::tokio::runtime::Builder::new_current_thread()
                    .build()
                    .expect("building a current-thread tokio runtime");
                // LocalServerRenderer's future is `!Send`; a LocalSet is what lets
                // the `spawn_local` yew uses internally resolve on this thread.
                let local = ::tokio::task::LocalSet::new();
                local.block_on(&rt, async {
                    ::yew::LocalServerRenderer::<W>::new()
                        .hydratable(false)
                        .render()
                        .await
                })
            });
            match rendered {
                Ok(html) => Ok(html),
                Err(payload) => Err((payload_message(&*payload), take_panic_location())),
            }
        }

        ::std::thread_local! {
            static PANIC_AT: ::std::cell::RefCell<Option<String>> =
                ::std::cell::RefCell::new(None);
        }

        fn take_panic_location() -> Option<String> {
            PANIC_AT.with(|slot| slot.borrow_mut().take())
        }

        /// Replaces the default hook, which would print a backtrace per panic.
        /// `Swatch` panicking is an expected outcome of this run, so the default
        /// hook would bury 38 components' worth of output in noise; the location
        /// is kept instead, because that is the span the verify gate quotes.
        fn install_panic_capture() {
            ::std::panic::set_hook(::std::boxed::Box::new(|info| {
                let at = info
                    .location()
                    .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
                PANIC_AT.with(|slot| *slot.borrow_mut() = at);
            }));
        }

        fn payload_message(payload: &(dyn ::std::any::Any + Send)) -> String {
            if let Some(s) = payload.downcast_ref::<&'static str>() {
                return (*s).to_string();
            }
            if let Some(s) = payload.downcast_ref::<String>() {
                return s.clone();
            }
            "panicked with a non-string payload".to_string()
        }
    })
    .context("the generated harness is not parseable as a Rust file")?;

    Ok(format!(
        "// @generated by xtask/src/harness.rs — do not edit.\n\
         //\n\
         // One zero-prop wrapper per (component, render cell). Text leaves carry\n\
         // private-use sentinels that survive Yew's escaping; structural choices\n\
         // are baked into the wrapper. src_hash {}\n\n{}",
        plan.src_hash,
        prettyplease::unparse(&file),
    ))
}

// ---------------------------------------------------------------- value builder

/// Builds one cell's `html!` body, assigning sentinel and slot indices as it
/// goes. The traversal order — props in declaration order, depth first, list
/// elements in order — is the contract `plan.json` records; nothing downstream
/// should try to re-derive it.
struct Builder<'a> {
    tk: &'a Toolkit,
    comp: &'a Component,
    cell: &'a Cell,
    next_sentinel: usize,
    next_slot: usize,
    sentinels: Vec<Binding>,
    slots: Vec<Binding>,
    notes: Vec<String>,
    /// Struct names currently being built, so a self-referential type reports
    /// instead of recursing forever (`matrix` guards the same way).
    stack: Vec<String>,
}

impl<'a> Builder<'a> {
    fn new(tk: &'a Toolkit, comp: &'a Component, cell: &'a Cell) -> Self {
        Builder {
            tk,
            comp,
            cell,
            next_sentinel: 0,
            next_slot: 0,
            sentinels: Vec::new(),
            slots: Vec::new(),
            notes: Vec::new(),
            stack: Vec::new(),
        }
    }

    fn run(mut self) -> Result<(CellPlan, TokenStream)> {
        let mut attrs = TokenStream::new();
        for p in &self.comp.props {
            let ty = parse_ty(&p.rust_ty, &p.span, &p.name)?;
            let value = self.value(&ty, &p.kind, &p.name, &p.name, &p.span)?;
            let name = ident(&p.name);
            attrs.extend(quote! { #name={#value} });
        }

        // Element syntax with an explicit closing tag. Explicit rather than
        // self-closing because the two are equivalent to yew's parser and an
        // explicit tag never produces a `/` `>` pair for prettyplease to
        // re-space. `children` is passed as a named attribute like any other
        // prop, which yew allows as long as the element carries no body — see
        // `children_renderer` in yew-macro's props/component.rs.
        let comp = ident(&self.comp.name);
        let body = quote! { ::yew::html! { <#comp #attrs></#comp> } };

        let wrapper = format!("W_{}__{}", self.comp.name, self.cell.id);
        let plan = CellPlan {
            component: self.comp.name.clone(),
            cell: self.cell.id.clone(),
            key: format!("{}/{}", self.comp.name, self.cell.id),
            wrapper,
            status: self.comp.status.clone(),
            choices: self.cell.choices.iter().map(|c| c.to_string()).collect(),
            sentinels: self.sentinels,
            slots: self.slots,
            notes: self.notes,
        };
        Ok((plan, body))
    }

    /// `axis` is the path as `matrix::Path` displays it (`values[].language`),
    /// which is how a cell's choices are keyed. `path` is the same route with
    /// list positions resolved (`values[1].language`), which is what a human —
    /// and the splicer — needs.
    fn value(
        &mut self,
        ty: &syn::Type,
        kind: &PropKind,
        axis: &str,
        path: &str,
        span: &Span,
    ) -> Result<TokenStream> {
        if let Some(inner) = generic_arg(ty, "Option") {
            return match self.selection(axis) {
                Some(Selection::Present) => {
                    let v = self.value(inner, kind, axis, path, span)?;
                    Ok(quote! { ::std::option::Option::Some(#v) })
                }
                Some(Selection::Absent) => Ok(quote! { ::std::option::Option::<#inner>::None }),
                Some(other) => bail!(
                    "{}:{}: `{path}` is `{}` but the matrix chose {other} for it",
                    span.file,
                    span.line,
                    ty.to_token_stream(),
                ),
                None => {
                    // Only reachable when MAX_NEST_DEPTH stopped axis derivation
                    // above this field, so `matrix` already reported it. Filling
                    // `None` is the baseline; saying so is what keeps it from
                    // looking like coverage.
                    self.note(format!(
                        "`{path}` ({}:{}) has no matrix axis — rendered as None, so any \
                         branch on it is unrendered",
                        span.file, span.line
                    ));
                    Ok(quote! { ::std::option::Option::<#inner>::None })
                }
            };
        }

        match kind {
            PropKind::Text => Ok(self.text_leaf(ty, axis, path, span)),
            PropKind::Slot => Ok(self.slot_leaf(ty, axis, path, span)),
            PropKind::Bool => {
                let v = match self.selection(axis) {
                    Some(Selection::Bool(v)) => *v,
                    Some(other) => bail!(
                        "{}:{}: `{path}` is a bool but the matrix chose {other} for it",
                        span.file,
                        span.line,
                    ),
                    None => {
                        self.note(format!(
                            "`{path}` ({}:{}) is a bool with no matrix axis — rendered false",
                            span.file, span.line
                        ));
                        false
                    }
                };
                Ok(if v { quote!(true) } else { quote!(false) })
            }
            PropKind::Num { rust } => {
                // No prop in scope is numeric, so this is the honest fallback
                // rather than a tested path: a number has no sentinel — it
                // renders as digits — so `splice` cannot recover it from the
                // markup and the leaf is recorded as unrecoverable.
                self.note(format!(
                    "`{path}` ({}:{}) is `{rust}`; numbers carry no sentinel, so it renders \
                     as its Default and cannot be spliced back",
                    span.file, span.line
                ));
                Ok(quote! { <#ty as ::std::default::Default>::default() })
            }
            PropKind::UnitEnum { name } => self.enum_leaf(name, axis, path, span),
            PropKind::List { item } => self.list(ty, item, axis, path, span),
            PropKind::Tuple { items } => self.tuple(ty, items, axis, path, span),
            PropKind::Struct { name } => self.plain_struct(name, axis, path, span),
            PropKind::Callback { arg } => {
                // classify marks a component with a callback `NeedsOverride`, so
                // this only renders the markup around it. A noop keeps the cell
                // renderable; the note is what stops it reading as supported.
                self.note(format!(
                    "`{path}` ({}:{}) is `Callback<{arg}>` — rendered as a noop; no React \
                     handler can be recovered from the markup",
                    span.file, span.line
                ));
                Ok(quote! { <#ty as ::std::default::Default>::default() })
            }
        }
    }

    fn text_leaf(&mut self, ty: &syn::Type, axis: &str, path: &str, span: &Span) -> TokenStream {
        let index = self.next_sentinel;
        self.next_sentinel += 1;
        let axis_key = self.selection(axis).map(|_| axis.to_string());
        self.sentinels.push(Binding {
            index,
            path: path.to_string(),
            axis: axis_key,
            rust_ty: ty.to_token_stream().to_string(),
            span: span.clone(),
        });
        // A call, not a literal: `verify` renders every cell twice in two
        // different sentinel alphabets (xtask/src/verify.rs:94) and compares the
        // normalised markups, so the placeholder has to be chosen at render time.
        // `sent` returns `&'static str`, which is what the three `&'static str`
        // props still need (src/molecules/nav_dropdown.rs:18,
        // member_filter_card.rs:14, pillar_card.rs:9).
        let n = Literal::usize_unsuffixed(index);
        let lit = quote! { sent(#n) };
        match text_form(ty) {
            TextForm::AttrValue => quote! { ::yew::AttrValue::from(#lit) },
            TextForm::String => quote! { ::std::string::String::from(#lit) },
            // `&'static str` props — nav_dropdown.rs:18, member_filter_card.rs:14
            // and pillar_card.rs:9 — take the literal itself, which is exactly
            // what makes them fine to leave alone rather than widen.
            TextForm::Str => quote! { #lit },
            TextForm::Cow => quote! { ::std::borrow::Cow::Borrowed(#lit) },
            // Reflexive `From<T> for T` covers a text type this list has not
            // seen, so an unfamiliar alias still compiles instead of bailing.
            TextForm::Other => {
                quote! { <#ty as ::std::convert::From<&'static str>>::from(#lit) }
            }
        }
    }

    fn slot_leaf(&mut self, ty: &syn::Type, _axis: &str, path: &str, span: &Span) -> TokenStream {
        let index = self.next_slot;
        self.next_slot += 1;
        self.slots.push(Binding {
            index,
            path: path.to_string(),
            axis: None,
            rust_ty: ty.to_token_stream().to_string(),
            span: span.clone(),
        });
        let lit = Literal::string(&index.to_string());
        let tag = Ident::new(SLOT_TAG, PmSpan::call_site());
        let attr = syn::parse_str::<TokenStream>(SLOT_ATTR)
            .expect("SLOT_ATTR is a fixed dashed identifier");
        // A marker element rather than a sentinel string: a slot's content is a
        // subtree, so the splicer needs a position in the tree, not a position in
        // a text run. `<template>` carries no styling and no layout, and its
        // children are inert in a real parser, so it cannot perturb the markup
        // around it.
        quote! { ::yew::html! { <#tag #attr={#lit}></#tag> } }
    }

    fn enum_leaf(
        &mut self,
        name: &str,
        axis: &str,
        path: &str,
        span: &Span,
    ) -> Result<TokenStream> {
        let Some(def) = self.tk.unit_enum(name) else {
            bail!(
                "{}:{}: `{path}` is `{name}`, which is not in the IR's enums; the harness \
                 cannot write a value for a variant set it cannot see",
                span.file,
                span.line,
            );
        };
        let variant = match self.selection(axis) {
            Some(Selection::Variant(v)) => v.clone(),
            Some(other) => bail!(
                "{}:{}: `{path}` is the enum `{name}` but the matrix chose {other} for it",
                span.file,
                span.line,
            ),
            None => {
                let Some(first) = def.variants.first() else {
                    bail!(
                        "{}:{}: `{name}` has no variants, so `{path}` has no value to render",
                        def.span.file,
                        def.span.line,
                    );
                };
                self.note(format!(
                    "`{path}` ({}:{}) is `{name}` with no matrix axis — rendered as `{first}`, \
                     so the other {} arms are unrendered",
                    span.file,
                    span.line,
                    def.variants.len().saturating_sub(1),
                ));
                first.clone()
            }
        };
        // `variants` carries the constructible form, not the bare ident:
        // `BadgeVariant::Kind(TermKind::Class)` is what `src/atoms/onto_badge.rs:20`
        // actually declares, and flattening it to `Class` would both fail to
        // compile and collapse 8 of the 12 renderings at onto_badge.rs:40-46.
        let expr = format!("{name}::{variant}");
        let expr = syn::parse_str::<syn::Expr>(&expr).map_err(|e| {
            anyhow::anyhow!(
                "{}:{}: `{path}` wants the variant `{expr}`, which is not a valid Rust \
                 expression ({e}); the IR's variant spelling has to be constructible",
                def.span.file,
                def.span.line,
            )
        })?;
        Ok(quote! { #expr })
    }

    fn list(
        &mut self,
        ty: &syn::Type,
        item: &PropKind,
        axis: &str,
        path: &str,
        span: &Span,
    ) -> Result<TokenStream> {
        let Some(elem_ty) = generic_arg(ty, "Vec") else {
            bail!(
                "{}:{}: `{path}` is classified as a list but its type is `{}`, which is not \
                 a `Vec<_>`",
                span.file,
                span.line,
                ty.to_token_stream(),
            );
        };
        let arity = match self.selection(axis) {
            Some(Selection::Arity(n)) => *n,
            Some(other) => bail!(
                "{}:{}: `{path}` is a list but the matrix chose {other} for it",
                span.file,
                span.line,
            ),
            None => {
                self.note(format!(
                    "`{path}` ({}:{}) is a list with no matrix axis — rendered with one \
                     element, so neither the empty case nor any separator is covered",
                    span.file, span.line
                ));
                1
            }
        };
        // The item axes are element-uniform by construction (`PathSeg::Item` in
        // matrix.rs means "every element's ..."), so every element takes the same
        // structural choices and differs only in its sentinels.
        let item_axis = format!("{axis}[]");
        let mut elems = TokenStream::new();
        for i in 0..arity {
            let elem_path = format!("{path}[{i}]");
            let v = self.value(elem_ty, item, &item_axis, &elem_path, span)?;
            elems.extend(quote! { #v, });
        }
        Ok(quote! { ::std::vec![#elems] })
    }

    fn tuple(
        &mut self,
        ty: &syn::Type,
        items: &[PropKind],
        axis: &str,
        path: &str,
        span: &Span,
    ) -> Result<TokenStream> {
        let syn::Type::Tuple(tup) = ty else {
            bail!(
                "{}:{}: `{path}` is classified as a tuple but its type is `{}`",
                span.file,
                span.line,
                ty.to_token_stream(),
            );
        };
        if tup.elems.len() != items.len() {
            bail!(
                "{}:{}: `{path}` is `{}` ({} elements) but the IR records {} element kinds",
                span.file,
                span.line,
                ty.to_token_stream(),
                tup.elems.len(),
                items.len(),
            );
        }
        let mut elems = TokenStream::new();
        for (i, (elem_ty, kind)) in tup.elems.iter().zip(items).enumerate() {
            let v = self.value(
                elem_ty,
                kind,
                &format!("{axis}.{i}"),
                &format!("{path}.{i}"),
                span,
            )?;
            elems.extend(quote! { #v, });
        }
        Ok(quote! { (#elems) })
    }

    fn plain_struct(
        &mut self,
        name: &str,
        axis: &str,
        path: &str,
        span: &Span,
    ) -> Result<TokenStream> {
        let Some(def) = self.tk.plain_struct(name) else {
            bail!(
                "{}:{}: `{path}` is `{name}`, which is not in the IR's structs; its fields \
                 are matrix axes and cannot be guessed (see src/molecules/props_table.rs:59)",
                span.file,
                span.line,
            );
        };
        if self.stack.iter().any(|s| s == name) {
            bail!(
                "{}:{}: `{name}` is reachable from itself at `{path}`; the harness cannot \
                 build a finite value for it",
                def.span.file,
                def.span.line,
            );
        }
        self.stack.push(name.to_string());
        let mut fields = TokenStream::new();
        let mut err = None;
        for f in &def.fields {
            let field_axis = format!("{axis}.{}", f.name);
            let field_path = format!("{path}.{}", f.name);
            match parse_ty(&f.rust_ty, &f.span, &field_path)
                .and_then(|ty| self.value(&ty, &f.kind, &field_axis, &field_path, &f.span))
            {
                Ok(v) => {
                    let fname = ident(&f.name);
                    fields.extend(quote! { #fname: #v, });
                }
                Err(e) => {
                    err = Some(e);
                    break;
                }
            }
        }
        self.stack.pop();
        if let Some(e) = err {
            return Err(e);
        }
        let sname = ident(name);
        Ok(quote! { #sname { #fields } })
    }

    fn selection(&self, axis: &str) -> Option<&Selection> {
        self.cell.get(axis)
    }

    fn note(&mut self, note: String) {
        self.notes.push(note);
    }
}

// ---------------------------------------------------------------- type helpers

enum TextForm {
    AttrValue,
    String,
    Str,
    Cow,
    Other,
}

fn text_form(ty: &syn::Type) -> TextForm {
    match ty {
        syn::Type::Reference(r) => match &*r.elem {
            syn::Type::Path(p) if p.path.is_ident("str") => TextForm::Str,
            _ => TextForm::Other,
        },
        syn::Type::Path(p) => match p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default()
            .as_str()
        {
            "AttrValue" => TextForm::AttrValue,
            "String" => TextForm::String,
            "Cow" => TextForm::Cow,
            "str" => TextForm::Str,
            _ => TextForm::Other,
        },
        _ => TextForm::Other,
    }
}

/// The single type argument of `Wrapper<T>`, matched on the last path segment so
/// `std::option::Option<T>` and `Option<T>` behave the same.
fn generic_arg<'a>(ty: &'a syn::Type, wrapper: &str) -> Option<&'a syn::Type> {
    let syn::Type::Path(p) = ty else { return None };
    let seg = p.path.segments.last()?;
    if seg.ident != wrapper {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    args.args.iter().find_map(|a| match a {
        syn::GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

fn parse_ty(rust_ty: &str, span: &Span, path: &str) -> Result<syn::Type> {
    syn::parse_str::<syn::Type>(rust_ty).map_err(|e| {
        anyhow::anyhow!(
            "{}:{}: `{path}` is typed `{rust_ty}`, which the harness could not parse as a \
             Rust type ({e})",
            span.file,
            span.line,
        )
    })
}

/// Prop and field names are Rust identifiers already, but a raw one (`r#type`)
/// would arrive here unescaped, so it is re-raised rather than dropped.
fn ident(name: &str) -> Ident {
    syn::parse_str::<Ident>(name)
        .unwrap_or_else(|_| Ident::new_raw(name.trim_start_matches("r#"), PmSpan::call_site()))
}

fn to_snake(name: &str) -> String {
    let mut out = String::new();
    let mut prev_upper = false;
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i != 0 && !prev_upper && !out.ends_with('_') {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_upper = true;
        } else {
            out.push(ch);
            prev_upper = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real crate, parsed and classified, so these tests break when the
    /// toolkit changes rather than when a fixture goes stale — the lesson from
    /// the parse/classify seam, where both stages' own tests passed while the
    /// pipeline could not run.
    fn toolkit() -> Toolkit {
        let root = toolkit_root();
        let mut tk = crate::parse::parse(&root).expect("parsing the toolkit");
        crate::classify::classify(&mut tk).expect("classifying the toolkit");
        tk
    }

    fn emit_to_out() -> (Toolkit, BTreeMap<String, Vec<Cell>>, PathBuf) {
        let tk = toolkit();
        let cells = cells_for(&tk).expect("deriving render cells");
        let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("out/harness");
        emit_harness(&tk, &cells, &out).expect("emitting the harness");
        (tk, cells, out)
    }

    #[test]
    fn sentinels_round_trip() {
        assert_eq!(sentinel(0), "\u{E000}0\u{E001}");
        assert_eq!(sentinel(41), "\u{E000}41\u{E001}");
        assert_eq!(sentinel_index(&sentinel(41)), Some(41));
        assert_eq!(sentinel_index("41"), None);
        // The whole mechanism rests on this: html_escape's encode_text and
        // encode_double_quoted_attribute rewrite only `& < > "`.
        assert!(!sentinel(0).chars().any(|c| matches!(c, '&' | '<' | '>' | '"')));
    }

    #[test]
    fn the_real_crate_emits_a_harness() {
        let (tk, cells, out) = emit_to_out();

        // The scope #809 specifies: everything the IR did not exclude.
        let in_scope = tk
            .components
            .iter()
            .filter(|c| !matches!(c.status, Status::Excluded { .. }))
            .count();
        assert_eq!(in_scope, 41, "in-scope components");
        assert_eq!(cells.len(), 41, "components with cells");

        let source = std::fs::read_to_string(out.join("src/main.rs")).expect("main.rs");
        assert!(source.starts_with("// @generated by xtask/src/harness.rs"));
        // Never a `*Props` type: 37 of 48 are private to their modules. Tested
        // against the IR's own list rather than a bare `contains("Props")` —
        // `PropsTable` is a component and its wrapper names legitimately carry
        // the substring, so the blunt form fails on a correct harness.
        for ty in tk.components.iter().filter_map(|c| c.props_ty.as_deref()) {
            assert!(
                !source.contains(ty),
                "the harness named the props type `{ty}`, which does not resolve from \
                 the crate root"
            );
        }
        assert!(source.contains("hydratable(false)"), "hydration markers must be off");
        assert!(source.contains("catch_unwind"));

        let manifest = std::fs::read_to_string(out.join("Cargo.toml")).expect("Cargo.toml");
        assert!(manifest.contains("[workspace]"), "must not join the toolkit's workspace");
        assert!(manifest.contains("features = [\"ssr\"]"));
        // Comment lines skipped: the manifest's own comment names `csr` to say
        // why it is not enabled, and matching that text would fail on the very
        // manifest it is describing. What must not appear is `csr` in a
        // dependency's feature list.
        for line in manifest.lines().filter(|l| !l.trim_start().starts_with('#')) {
            assert!(
                !line.contains("csr"),
                "the harness enabled the csr feature, which pulls wasm-bindgen/web-sys \
                 in to render 38 presentational components: {line}"
            );
        }
    }

    #[test]
    fn every_cell_gets_exactly_one_wrapper() {
        let (tk, cells, _) = emit_to_out();
        let plan = plan(&tk, &cells).expect("plan");
        let total: usize = cells.values().map(|c| c.len()).sum();
        assert_eq!(plan.cells.len(), total);
        let wrappers: BTreeSet<&str> = plan.cells.iter().map(|c| c.wrapper.as_str()).collect();
        assert_eq!(wrappers.len(), plan.cells.len(), "wrapper names collided");
        let keys: BTreeSet<&str> = plan.cells.iter().map(|c| c.key.as_str()).collect();
        assert_eq!(keys.len(), plan.cells.len(), "render keys collided");
    }

    #[test]
    fn sentinel_indices_are_dense_and_unique_per_cell() {
        let (tk, cells, _) = emit_to_out();
        let plan = plan(&tk, &cells).expect("plan");
        for cell in &plan.cells {
            let idx: Vec<usize> = cell.sentinels.iter().map(|b| b.index).collect();
            assert_eq!(idx, (0..idx.len()).collect::<Vec<_>>(), "{}", cell.key);
            let slots: Vec<usize> = cell.slots.iter().map(|b| b.index).collect();
            assert_eq!(slots, (0..slots.len()).collect::<Vec<_>>(), "{}", cell.key);
        }
    }

    /// Nothing in this crate should need an assumed value: every branch the
    /// generator renders has to be a matrix axis, or the emitter silently
    /// hard-codes one arm (the failure mode #809 calls out).
    #[test]
    fn no_leaf_was_filled_without_an_axis() {
        let (tk, cells, _) = emit_to_out();
        let plan = plan(&tk, &cells).expect("plan");
        assert!(plan.notes().is_empty(), "{:#?}", plan.notes());
    }

    /// `src/molecules/props_table.rs:59` branches `if let Some(default)` on a
    /// *field of a list item*. If nested axes did not reach the harness both
    /// cells would carry the same value and the `Some` arm would never render.
    #[test]
    fn nested_struct_fields_vary() {
        let (tk, cells, _) = emit_to_out();
        let plan = plan(&tk, &cells).expect("plan");
        let props_table: Vec<&CellPlan> =
            plan.cells.iter().filter(|c| c.component == "PropsTable").collect();
        let with_default = props_table
            .iter()
            .any(|c| c.choices.iter().any(|ch| ch == "props[].default=Some"));
        let without = props_table
            .iter()
            .any(|c| c.choices.iter().any(|ch| ch == "props[].default=None"));
        assert!(with_default && without, "{:#?}", props_table.iter().map(|c| &c.choices).collect::<Vec<_>>());

        // src/molecules/annotation.rs:33-41 matches three ways on
        // LiteralValue.language / .datatype.
        let anno: BTreeSet<String> = plan
            .cells
            .iter()
            .filter(|c| c.component == "OntoAnnotation")
            .flat_map(|c| c.choices.iter().cloned())
            .collect();
        assert!(anno.contains("values[].language=Some"), "{anno:#?}");
        assert!(anno.contains("values[].datatype=Some"), "{anno:#?}");
    }

    /// `src/atoms/onto_badge.rs:20` declares `BadgeVariant::Kind(TermKind)`, so
    /// the harness has to write the payload path, not the bare variant ident.
    #[test]
    fn data_carrying_variants_are_written_as_constructible_paths() {
        let (tk, cells, _) = emit_to_out();
        let source = {
            let plan = plan(&tk, &cells).expect("plan");
            let _ = plan;
            std::fs::read_to_string(
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("out/harness/src/main.rs"),
            )
            .expect("main.rs")
        };
        let flat = source.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains("BadgeVariant :: Kind (TermKind :: Class)")
                || flat.contains("BadgeVariant::Kind(TermKind::Class)"),
            "the harness flattened a data-carrying variant"
        );
    }

    /// Both expected quarantine candidates must still be *rendered*: the verify
    /// gate can only see the `&hex[0..2]` panic (src/molecules/swatch.rs:11) and
    /// the `title` -> `initial` transform (src/molecules/dataset_card.rs:43) in
    /// the harness output.
    #[test]
    fn the_quarantine_candidates_are_rendered_not_skipped() {
        let (tk, cells, _) = emit_to_out();
        let plan = plan(&tk, &cells).expect("plan");
        for name in ["Swatch", "DatasetCard", "Modal"] {
            assert!(
                plan.cells.iter().any(|c| c.component == name),
                "{name} has no cells in the harness"
            );
        }
        // Modal is NeedsOverride (src/molecules/modal.rs hardcodes inert), and
        // the plan carries that so verify need not re-read the IR.
        let modal = plan.cells.iter().find(|c| c.component == "Modal").unwrap();
        assert!(matches!(modal.status, Status::NeedsOverride { .. }), "{:?}", modal.status);
    }

    #[test]
    fn slots_get_a_marker_element() {
        let (tk, cells, _) = emit_to_out();
        let plan = plan(&tk, &cells).expect("plan");
        let pill = plan.cells.iter().find(|c| c.component == "Pill").expect("Pill");
        assert_eq!(pill.slots.len(), 1);
        assert_eq!(pill.slots[0].path, "children");
        assert_eq!(slot_marker(0), "<template data-eona-slot=\"0\"></template>");
    }

    #[test]
    fn rendered_entries_round_trip() {
        let ok = Rendered { html: Some("<b>x</b>".into()), ..Rendered::default() };
        let boom = Rendered {
            panic: Some("byte index 2 is not a char boundary".into()),
            at: Some("src/molecules/swatch.rs:11:9".into()),
            ..Rendered::default()
        };
        let map: BTreeMap<String, Rendered> =
            [("Button/base".to_string(), ok), ("Swatch/base".to_string(), boom)]
                .into_iter()
                .collect();
        let json = serde_json::to_string(&map).unwrap();
        assert!(!json.contains("\"panic\":null"), "absent fields must be omitted: {json}");
        let back: BTreeMap<String, Rendered> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, map);
        assert_eq!(
            back["Swatch/base"].panic_reason().as_deref(),
            Some("byte index 2 is not a char boundary (src/molecules/swatch.rs:11:9)")
        );
    }

    /// The whole Button wrapper, printed, so a reviewer sees exactly what the
    /// generator emits for the simplest real component.
    #[test]
    fn button_wrapper_is_element_syntax() {
        let tk = toolkit();
        let cells = cells_for(&tk).expect("cells");
        let button = tk.component("Button").expect("Button");
        for cell in &cells["Button"] {
            let (plan, body) = Builder::new(&tk, button, cell).run().expect("build");
            let file: syn::File = syn::parse2(quote! {
                #[::yew::function_component(Wrapper)]
                fn wrapper() -> ::yew::Html { #body }
            })
            .expect("parse");
            println!("--- Button/{} ({})\n{}", plan.cell, plan.choices.join(", "), prettyplease::unparse(&file));
        }
    }
}
