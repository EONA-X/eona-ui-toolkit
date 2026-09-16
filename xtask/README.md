# `cargo xtask` — the eona-ui-toolkit React generator

This crate turns the Yew components in `../src` into a React component library
at `../packages/react`. It is **not** a Rust-to-TSX transpiler and there is no
WebAssembly anywhere in it.

The mechanism, in one paragraph: for each component the generator emits a
zero-prop harness wrapper whose props are filled with Unicode private-use
sentinels, renders it through Yew's own `LocalServerRenderer` with
`hydratable(false)`, and then splices JSX expressions back over the byte ranges
where the sentinels landed. The markup React ships is therefore markup Yew
produced, not markup anybody re-implemented.

```
cargo xtask dump-ir     # parse + classify only; prints the scope arithmetic
cargo xtask render      # ... + matrix + harness + render + verify
cargo xtask bridgegen   # ... + splice + emit + examples + write abi/, bridge/ and packages/react
cargo xtask check       # is abi/ and packages/react what this src/ would produce?
```

## The pipeline, stage by stage

Each stage's own module doc is the detailed contract; this is the map.

| Stage | File | In | Out |
|---|---|---|---|
| `parse` | `src/parse.rs` | `../src/**/*.rs` via `syn` | every `#[function_component]` in the IR, all `Status::Ok` |
| `classify` | `src/classify.rs` | IR | every prop mapped to one `PropKind`; #809's scope rules applied as `Status` |
| `matrix` | `src/matrix.rs` | IR | render *cells* — one per combination of choices that can change the markup's **shape** |
| `harness` | `src/harness.rs` | IR + cells | a throwaway crate in `out/harness` that renders every cell twice, once per sentinel alphabet |
| `render` | `src/render.rs` | that crate | builds it, runs it, collects `out/renders.json`; a panic is data, not a crash |
| `verify` | `src/verify.rs` | renders | the safety gate — see below. Produces `abi/quarantine.json` |
| `splice` | `src/splice.rs` | renders + plan | one JSX tree per component, recovering conditionals and list templates |
| `emit` | `src/emit.rs` | IR + JSX | `packages/react` — one self-contained `.tsx` per component, plus `index.ts`, `types.ts`, `package.json`, `tsconfig.json`, `README.md` |
| `abi` | `src/abi.rs` | IR + cells | `abi/toolkit.abi.json`, and the semver diff behind `check --semver` |
| `examples` | `src/examples.rs` | the ABI + IR + `abi/overrides.toml` | `abi/examples.json` and `bridge/examples.rs` — one usage snippet per component, in Yew **and** React |
| `check` | `src/check.rs` | all of the above | the drift gate |

The IR that crosses every boundary is `src/ir.rs`. Nothing downstream of `parse`
re-reads Rust.

### What is generated, and what is not

38 of the crate's 42 components are in scope: a component is generated iff it is
presentational **and** takes props. Every exclusion is recorded in the IR as
`Status::Excluded` with the rule as its reason — nothing is silently dropped.

| Rule | Count |
|---|---|
| `zero-prop demo section` (`ComponentsGallery`, `IntegrationSection`, `OntologySection`) | 3 |
| `crate showcase page` (`DemoPage`) | 1 |

Two rules match nothing today and are kept as tripwires, not as dead code:
`interactive tier` (everything under `src/interactive/`) and the `Callback`
rule. Both are keyed on shape rather than on a name, and the tier's subject can
come back — `eona-vocabulary-ui`, which owns it now, depends on this crate by
relative path. The name-keyed `build.rs OUT_DIR type` rule was deleted with
`BeeNest`; the registered-struct check in `classify` guards that shape more
generally, and the crate has no `build.rs` left.

Of the 38 that remain, 34 are emitted and 33 are exported. The four that are not
emitted, and the one that is emitted but withheld from `index.ts`, each carry an
`abi/overrides.toml` entry: see **Handling a quarantine**.

The scope arithmetic moved with the crate and is worth restating, because the
numbers in #809 were taken before it: the toolkit carried 76 components when that
issue was written. 34 of them left — 18 zero-prop page sections and the 8 BeeNest
variants to `developer.eona-x.eu/crates/site`, the 8 stateful ontology components
to `eona-vocabulary-ui`. **Every one of them was already `Status::Excluded`**, so
the in-scope set did not move: 38 before, 38 after, 34 emitted, 33 exported. That
is why the move is checkable — regenerating against the slimmed tree reproduces
every `.tsx` byte for byte, and only `index.ts`'s `srcHash` and its "out of scope
by rule" tally change.

### Where the consumer-facing docs live

Three places, and only one of them is hand-written:

| Document | Written by | Audience |
|---|---|---|
| `packages/react/README.md` | `emit::package_readme` | someone who has the npm package |
| the components page, section 09 | `examples` -> `bridge/examples.rs` | someone comparing one component across both stacks |
| the components page, sections 10-19 | `../src/organisms/integration_section.rs`, by hand | someone wiring the crate or the package into a host |

A claim about what a consumer must do belongs in exactly one of them, and the
first two are regenerated, so a correction to either is an edit to `emit.rs` or
`examples.rs` and not to the artifact. `check` invariant 5 will report the
artifact edit; it will not tell you the prose was wrong.

## The usage snippets

The components page has to show every component twice: the Yew `html!` call and
the React JSX that does the same thing. Writing the second one by hand would put,
in two panes on one page, exactly the drift this generator exists to remove — the
Yew half is checked by the compiler and a hand-written React half is checked by
nobody.

So `examples` derives both from `abi/toolkit.abi.json`. The ABI supplies the
member list, the TS type, optionality and the default; the IR supplies the
`PropKind` and the Rust variant names, which the ABI deliberately does not carry
(it records the TS union, not the Rust enum). A prop added in `src/` therefore
reaches both panes on the next `bridgegen`, with no second place to update. The
props *table* is not restated — that is what the ABI already is.

Two files, because they are read by different things:

- **`abi/examples.json`** is the reviewable half: sorted by component, pretty
  printed, no timestamps, byte-diffed by `check` exactly like the ABI. It is
  **total** — all 42 components, excluded ones included.
- **`bridge/examples.rs`** is the consumable half: `&'static str` tables the
  toolkit can pull in with `#[path = "../bridge/examples.rs"] mod`. It exists
  because the toolkit's whole dependency set is `yew`, so nothing in the crate
  could parse the JSON at runtime and there is no `build.rs` left to parse it at
  build time.

`bridge/`, not `src/`: `src_hash` is taken over `src/**/*.rs`, and a generated
file inside `src/` would be an input to the hash of the document that describes
it — `check` would need two passes to converge.

### What the snippets are, and are not

They document the *shape* of the call: which props exist, how each is spelled in
each stack. They are not copy. Values come from a closed `PLACEHOLDERS` table
keyed on prop (or field, or tuple-element) name, falling back to the name itself
in title case; the table exists only for the names where a title-cased name would
be actively wrong — `href="Href"` is not a link and `hex="Hex"` is not a colour.

Three rules are worth knowing before adding a component:

- **A prop is shown at the value that is not its default.** `<CodeBlock
  copyable={true} />` is the same call as `<CodeBlock />` and documents nothing,
  so the bool with a `true` default shows `false` and an enum shows the first
  variant that is not its default.
- **A variant whose payload names a type `parse` never registered is skipped.**
  `parse::expand_data_enums` turns `BadgeVariant::Kind(TermKind)` into its eight
  inhabitants but does not register `TermKind` — no prop names it — so a snippet
  spelling `BadgeVariant::Kind(TermKind::Ontology)` would need a `use` line this
  stage has no span for. `OntoBadge` shows `BadgeVariant::Lang` instead; the full
  twelve-member union is in the ABI, which is where a reader looks for the set.
  An enum with *no* writable variant is a hard error, not a guess.
- **Quarantined and withheld components get the Yew call and a note, never
  JSX.** `Modal`, `Swatch`, `PaletteGroup`, `DatasetCard` and `OntoAnnotation`
  exist in the crate and not in the React package, and the note is the
  `disposition` a human wrote in `abi/overrides.toml` — read from the same file
  the gate reads, so the page and the gate cannot disagree about who is withheld.

## The verify gate

Everything downstream assumes one thing: that a component's markup is the *same
function of its props* for every value those props could take. `splice` cannot
check that — it sees one string of HTML and a set of byte offsets. This is where
the assumption is tested, per (component, cell):

1. **Sentinel integrity.** Every planted sentinel comes back exactly once and
   intact, and nothing sentinel-shaped comes back that was not planted.
2. **List order.** A list's sentinels come back in the order they were planted.
   `splice` emits `.map()` over the array in index order, so a component that
   reverses or sorts would get JSX that renders a different list.
3. **Differential alphabet.** The cell is rendered twice with two sentinel
   alphabets that disagree on everything a component could branch on. Normalise
   the sentinels away and the two markups must be byte-identical.
4. **Panic capture.** A render that panics quarantines its component; the run
   continues.

### Why the differential alphabet's body looks like line noise

`Primary` is `\u{E000}<id>\u{E001}`. `Differential` is
`\u{F0000}<id:04><PAD>\u{F0001}`, where `PAD` (`src/verify.rs`) is 70 mixed-case
ASCII characters containing `#` and `/`.

The delimiters alone only disagree on leading character, character count, byte
length and UTF-8 boundaries, and the body used to be digits only. That left the
gate blind to any transform that is the **identity on both payloads** — and a
digits-only payload has no case, no `#`, no `/`, and is one or two characters
long. Measured blind spots, all of which now fail:

- `to_uppercase()` / `to_lowercase()` — digits have no case.
- Truncation at any length above 6.
- A class or attribute chosen by `len()` — detection used to need the threshold
  to fall strictly between 7 and 12 bytes. The window is now 7..82.
- `rfind('#')`-then-slice. This one shipped: `src/molecules/annotation.rs:24`
  returns a datatype IRI's local name, `short_datatype` was the identity on
  every probe, and `OntoAnnotation` passed the gate while rendering
  `http://www.w3.org/2001/XMLSchema#string` where Yew renders `string`.

**What is still invisible:** a branch gated on a *literal* value
(`if label == "danger"`). No probe alphabet can reach it, because the gate cannot
guess a constant it was never told about. `verify.rs`'s
`a_branch_on_a_literal_value_is_still_invisible_and_that_is_recorded` pins that
limit as a test rather than leaving it as a claim in this file. Nothing in the
crate has that shape today; if you add one, the generator will be confidently
wrong about it.

## Adding a component

Nothing in `xtask/` needs editing. Write the Yew component in `../src`, then:

```sh
cargo xtask bridgegen
```

and commit `packages/react/`, `abi/toolkit.abi.json`, `abi/quarantine.json`,
`abi/package.manifest.json`, `abi/examples.json` and `bridge/examples.rs` along
with it.

It will be picked up automatically if it is presentational (not under
`src/interactive/`) and takes a props struct. If it takes no props it is
excluded as a `zero-prop demo section`; that is the rule, not a bug.

Things that will make the run fail loudly rather than silently do the wrong
thing:

- **A prop type `classify` does not recognise.** Hard error naming the span.
  There is no fallback kind, because the fallback is a TypeScript `any`.
- **A `Callback<T>` prop.** Forces `NeedsOverride`: a callback cannot be
  pre-rendered.
- **A prop whose type `parse` never registered** — a type generated into
  `OUT_DIR` by a `build.rs`, say. Hard error, unless the component is already
  excluded. `BeeNest`'s `&'static BeeNestSpec` was the instance until BeeNest
  moved to the portal.
- **A component the verify gate rejects.** See below.

If your component reads a prop's *content* — slices it, cases it, takes
`chars().next()`, sorts a list by it — expect a quarantine, and expect it to be
correct. The generator can only splice a prop it can see arrive unchanged.

## Handling a quarantine

A quarantine is not a failure of the run. It is the gate saying *this component's
markup is not a pass-through of its props, so no splice of it can be right.* The
component keeps its place in the ABI with the evidence attached, so it is missing
from the React package loudly rather than quietly.

When `bridgegen` reports a quarantine:

1. **Read the evidence.** `abi/quarantine.json` names the cell, the alphabet, the
   failing check, a byte offset, an excerpt of the markup, and a `file:line`.
   The excerpt is usually enough on its own — `OntoAnnotation`'s was a badge
   reading `oJ/`, the tail of `PAD` after its last `/`.
2. **Decide what to do, and record it in `abi/overrides.toml`.** `check` requires
   an entry for every component whose `Status` is not `ok`; a component absent
   from the package without one fails the build. The entry says *reason*,
   *source* and *disposition* in a person's words — normally "hand-port", or
   "fix upstream in `src/` and regenerate".
3. **If it is expected, add it to `verify::EXPECTED_QUARANTINES`.** That list is
   how the gate reports its own failure: a run in which `Swatch` *passes* is a
   run in which the panic capture stopped working, and a silent pass would look
   exactly like success.

The five current entries:

| Component | Why |
|---|---|
| `Swatch` | `src/molecules/swatch.rs:11` slices `&hex[0..2]`; a sentinel's first char is 3 or 4 bytes, so byte 2 is never a char boundary and every cell panics. |
| `PaletteGroup` | `src/molecules/palette_group.rs:52` renders a `<Swatch>` per `ColorSpec`, so the panic propagates — and so does `Swatch`'s derived foreground, which makes `PaletteGroup`'s markup a transform of `colors` too. |
| `DatasetCard` | `src/molecules/dataset_card.rs:43` derives `initial` from `title.chars().next()`. |
| `OntoAnnotation` | `src/molecules/annotation.rs:24` shortens the datatype IRI. |
| `Modal` | `NeedsOverride`, not quarantined: `src/molecules/modal.rs:22` hardcodes `inert={true}`, so only the closed state is renderable. Emitted and withheld from `index.ts` pending `open: bool` upstream (#809). |

`Swatch` and `PaletteGroup` look like artifacts of the probe alphabet — they
render fine for real callers, and they fail only because a multi-byte sentinel is
not a legal input to a byte slice. An audit proposed an ASCII, hex-safe alphabet
to recover both. **That was tried and refused, and the reason matters more than
the two components do.**

`contrast_color` (`src/molecules/swatch.rs:7-16`) parses the prop as hex and
picks a foreground from the YIQ luminance, falling back to `unwrap_or(0)`. No
sentinel is valid hex under *any* alphabet, so every channel reads 0, the
luminance is 0, and both alphabets render the same constant `#ffffff`. The two
renders agree and the sentinel itself passes through intact, so the gate would
**pass** `Swatch` and emit a component whose foreground is frozen to white —
unreadable on every light swatch in the palette. The panic is the loud symptom of
a transform that would otherwise be silent, and removing the panic removes the
symptom, not the transform.

This is pinned by
`verify::tests::a_hex_safe_alphabet_would_not_recover_swatch_it_would_silence_it`,
which runs the proposed change through the real gate. If that test ever starts
failing, the gate has learned to see a value-keyed branch and both entries should
be revisited.

## The drift gate

`cargo xtask check` answers one question: **is `abi/` and `packages/react` what
this `src/` would produce right now?** It exits non-zero with a unified diff when
the answer is no. Seven invariants get their own finding class so a reviewer does
not have to read a 4000-line diff to learn which one broke:

1. the ABI's `srcHash` matches `src/` as it is now;
2. every prop is mapped, and every component that is not `Ok` has an
   `abi/overrides.toml` entry;
3. no component's `Status` changed;
4. the quarantine set equals `abi/quarantine.json` exactly;
5. every file under `packages/react` hashes to what the emitter wrote
   (`abi/package.manifest.json`), none is missing, and none is present that no
   stage wrote;
6. the package and the ABI agree about which components exist — derived from
   `Status` on both sides, so it holds even when every hash agrees;
7. `abi/examples.json` and `bridge/examples.rs` are what the current ABI plus
   `abi/overrides.toml` would produce.

Invariant 7 is recomputed on both paths and never replayed: the snippets need no
render, and editing a `disposition` moves neither `srcHash` nor `generatorHash`,
so a hash comparison would not see it.

### The two paths

- **Replay (both hashes match): 0.11s.** The committed ABI carries the `srcHash`
  of the toolkit and the `generatorHash` of `xtask/src`. Those are the *only*
  inputs to the render, so when both match, the committed verdicts and package
  hashes are what regenerating would produce. The gate replays them onto a
  freshly parsed IR, then recomputes and byte-diffs the ABI and hashes every file
  under `packages/react`. Everything except the render verdicts is recomputed.
- **Render (either hash moved): 30-38s.** `harness` -> `render` -> `verify` ->
  `splice` -> `emit` runs for real, and a differing `.tsx` gets a real content
  diff rather than a hash.

**The 30-second budget in #809 is met on the replay path and missed on the render
path, every time.** Five separate one-line edits to `src/` measured 33.6 / 34.5 /
35.0 / 35.6 / 36.4s. Essentially all of it is cargo — a release rebuild of the
toolkit plus a harness relink; the render itself replays in 0.03-0.08s. Cutting
it means cutting cargo time (a lower `opt-level` for the harness profile, or
making the render a separate CI job keyed on `srcHash`), not tuning `check`.

CI should run plain `cargo xtask check`. `--render` is the escape hatch for "I do
not believe the cache"; it costs 0.17s on a warm render cache and re-derives the
verdicts instead of replaying them.

### `--write`

`cargo xtask check --write` accepts the current state into `abi/` and
`packages/react` instead of reporting it stale. It **refuses** to publish a
status change it did not render: on the replay path the verdicts come from
`abi/quarantine.json`, so a hand-edited quarantine file would otherwise be copied
into the ABI and certified clean by the next plain `check`. Re-run as
`check --render --write`, or just `bridgegen`.

## What a contributor must do when they change `src/`

1. `cargo xtask bridgegen`
2. `cargo xtask check` — must be clean
3. `cargo test -p xtask` and `cargo test --features ssr`
4. Commit `packages/react/`, `abi/toolkit.abi.json`, `abi/quarantine.json`,
   `abi/package.manifest.json`, `abi/examples.json` and `bridge/examples.rs`
   **in the same commit** as the `src/` change.

The gate fails on a whitespace-only edit to `src/`, because `srcHash` is over raw
bytes: a blank line moves every `source: "src/atoms/button.rs:NN"` in the ABI.
That churn is real and costs a 35s regenerate. Hashing a normalised token stream
instead would avoid it and is not implemented.

If you change anything under `xtask/src/`, `generatorHash` moves and the same
regenerate-and-commit applies, even when `src/` did not change.

## Changing the style helper

`splice::STYLE_HELPER_TS` is the one piece of hand-written TypeScript the
generator emits (`src/molecules/logo_download_card.rs:30` hands raw CSS text to
Yew's `style` attribute, and React's `style` prop takes an object). No probe in
the pipeline exercises it, so its Rust test asserts its *source*. Verify the
behaviour with node after any edit — extract the constant, strip the type
annotations, and check at least these, which are the two defects a differential
render found in it plus the cases that were already right:

| Input | Expected |
|---|---|
| `background-image:url(data:image/svg+xml;base64,AAAA)` | `{backgroundImage: "url(data:image/svg+xml;base64,AAAA)"}` — was truncated at the `;` inside `url(...)` |
| `BACKGROUND:#fff` | `{background: "#fff"}` — was passed through as `BACKGROUND`, which React re-hyphenates to `-b-a-c-k-g-r-o-u-n-d` and which styles nothing |
| `--brand:#fff;color:var(--brand)` | `{"--brand": "#fff", color: "var(--brand)"}` — custom properties are case-sensitive and must skip the fold |
| `font-family:"Foo; Bar", serif` | one declaration, not two |
| `background :  #fff ` | `{background: "#fff"}` |

## Scratch directories

`xtask/out/` is gitignored in full. It holds the generated harness crate, its
cargo target dir (~360MB, reused between runs — deleting it costs ~30s on the
next render), the render cache, and the raw `renders.json`. Nothing in it is an
input to anything; a clean checkout regenerates all of it.

The committed artifacts are `abi/` and `packages/react/` — with
`packages/*/dist/` and `packages/*/node_modules/` ignored, since those are tsup's
and npm's output rather than any stage's.
