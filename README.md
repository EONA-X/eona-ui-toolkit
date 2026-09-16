> **This is the source of truth.** Development happens here, on GitHub, from `v0.4.0` onward.
> EONA-X's internal GitLab (`eona-x/web/eona-ui-toolkit`) holds the pre-`v0.4.0` history as an
> archive and is no longer the place to raise changes — open a pull request here instead.
>
> The crate is consumed as a git submodule by
> [`eona-vocabulary-ui`](https://github.com/EONA-X/eona-vocabulary-ui) and by EONA-X's developer
> portal.

# eona-ui-toolkit

Shared component library for the EONA-X design system — atoms → molecules → organisms.
Authored once in Yew; consumed from Yew directly, and from React through
[`packages/react`](packages/react), which is generated from this source rather than written.

Extracted from [`developer.eona-x.eu`](https://gitlab.eona-x.org/eona-x/web/developer.eona-x.eu)
(`crates/eona-ds`); the git history of that crate is preserved here.

## What is in here, and what is not

**Reusable components and the stylesheets that make them look like EONA-X.** Nothing else: no
state, no data fetching, no page. The whole dependency list is `yew = "0.23"`.

Three groups of components that lived here had exactly one caller each, so they went back to
the projects that own them:

| What | Where it went | Why |
| --- | --- | --- |
| The charter slides and dataspace-architecture page sections (`ColorSection`, `DataspaceLayers`, `IdsRamLayers`, …) | `developer.eona-x.eu`, `crates/site/src/sections/` | Zero-prop pages, rendered only by the portal |
| `BeeNest` and its variants, `build.rs` and `data/*.ttl` | `developer.eona-x.eu`, `crates/site/` | Fixed diagrams of EONA-X architecture; they change on the architecture's schedule |
| The stateful ontology browser (`src/interactive/`, the JSON-LD parser) | [`eona-vocabulary-ui`](https://github.com/Eona-X/eona-vocabulary-ui) | Stateful, `csr`-only, and its only consumer wrote it |

Every consumer was paying for all three in compile time and API surface. What that move
bought is measurable: `oxttl`, `oxrdf`, `serde`, `serde_json`, `gloo-timers`, `js-sys`,
`wasm-bindgen`, `wasm-bindgen-futures` and `web-sys` all left the dependency graph, and the
`build.rs` went with them.

`OntoBadge`, `OntoAnnotation` and `DatasetCard` stayed — see
[The ontology components](#the-ontology-components).

## Using it

Pick a renderer — the components are renderer-agnostic, so neither feature is on by default:
`ssr` renders to HTML strings at build time, `csr` mounts them in a browser.

**An app that just needs the components** wants an ordinary git dependency. Cargo resolves
it, `Cargo.lock` pins the revision, and there is no submodule to forget to initialise:

```toml
# browser / WASM host (Trunk apps, the Streamlit component bridge)
eona-ui-toolkit = { git = "ssh://git@gitlab.eona-x.org:29418/eona-x/web/eona-ui-toolkit.git", rev = "<commit>", features = ["csr"] }
```

Pin a tag or a rev, never a branch: a branch re-resolves whenever the lockfile is
regenerated, turning an unrelated `cargo update` into a design-system change nobody asked
for. Check which refs exist before pinning one — `git ls-remote --tags` returns `v0.2.0` and
`v0.3.0` today, both cut before the crate was slimmed, so a rev is the honest pin until a
release is tagged off the current history.

**A site that publishes [the demo page](#the-demo-page)** wants a git submodule plus a path
dependency instead — which is what `developer.eona-x.eu` does:

```toml
[workspace]
# Required: this crate is its own workspace root (it carries `xtask`), and a path
# dependency inside the host's workspace directory is otherwise pulled in as a
# member — `error: multiple workspace roots found in the same workspace`.
exclude = ["crates/eona-ui-toolkit"]

[dependencies]
# static site generation
eona-ui-toolkit = { path = "crates/eona-ui-toolkit", features = ["ssr"] }
```

That is a deployment constraint, not a general recommendation. The portal's Dockerfile builds
from `COPY . .`, so a submodule arrives in the image as ordinary files while a git dependency
on a private remote would need credentials inside the build. Anywhere a credential *can* live
— a dev machine, CI with an SSH key or deploy token — the git dependency is the simpler
choice.

Markup and CSS classes are ported verbatim from the brand charter, so consumers **must**
ship the stylesheets or the components render unstyled. There are three, and they are crate
constants, so nothing needs to reach into this repo's on-disk layout:

```rust
std::fs::write(dist.join("tokens.css"), eona_ui_toolkit::TOKENS_CSS)?;
std::fs::write(dist.join("components.css"), eona_ui_toolkit::COMPONENTS_CSS)?;
// Only a host that renders OntoBadge or OntoAnnotation needs the third.
std::fs::write(dist.join("ontology.css"), eona_ui_toolkit::ONTOLOGY_CSS)?;
```

```html
<!-- conventional order; only tokens.css declares custom properties, and those
     resolve per element rather than in stylesheet order, so the order is not
     load-bearing. Omitting tokens.css is what breaks the page. -->
<link rel="stylesheet" href="tokens.css">
<link rel="stylesheet" href="components.css">
<link rel="stylesheet" href="ontology.css">
```

`assets/components.css` is written for a page this crate owns end to end: it opens with a
page-shell reset (`:13-30`) and reserves 156px of `body` padding for `SiteHeader` (`:393`).
That is right for a Yew host rendering the whole document and wrong for anything importing it
into an existing app — see `@eona-x/ui-toolkit-css`
([eona-x/backlog#808](https://gitlab.eona-x.org/eona-x/backlog/-/issues/808) Layer 3), which
drops those rules and scopes the rest under `.eona-ui`.

## The demo page

`demo::DemoPage` is the crate's own showcase — the component gallery, every preview paired
with the `html!` that renders it, followed by an integration guide covering the Cargo
features, the stylesheet constants and the submodule layout. It documents *this* crate, so
it lives here rather than in the site that publishes it.

Section 09 is the exception to "written here": it prints one canonical call per component in
both stacks, straight out of `bridge/examples.rs`, which `cargo xtask bridgegen` derives from
`abi/toolkit.abi.json`. That is the same document that types the React package, so the two
panes cannot describe different components, and `cargo xtask check` fails if either is edited
by hand. Components the React package does not ship carry the generator's own reason in place
of the JSX rather than an import that would not resolve.

```rust
use eona_ui_toolkit::demo::{DemoPage, DEMO_JS, DEMO_SHELL};
use yew::ServerRenderer;

let body = ServerRenderer::<DemoPage>::new().render().await;
std::fs::write(dist.join("components.html"), DEMO_SHELL.replace("__APP__", &body))?;
std::fs::write(dist.join("demo-page.js"), DEMO_JS)?;
```

`DemoPage` takes no required props; pass `nav` / `footer_text` / `footer_link` to seat it
inside a host's own navigation, which is what `developer.eona-x.eu` does to publish it as
`components.html`.

Two assets in `DEMO_SHELL` are **not** crate constants and the host has to supply both, but
they fail differently. `site-header.js` is the scroll-collapse glue for `SiteHeader`; without
it the header simply never collapses — degraded, not broken. `logo/logo-transparent.svg` is
the EONA-X mark, linked as the shell's favicon (`assets/demo-shell.html:14`) and hardcoded
twice more by `SiteHeader` (`src/organisms/site_header.rs:101,105`); this crate ships no
`logo/` directory, so a host that does not copy that file in renders a broken image. Both
belong to the portal.

`DEMO_JS` is the page's *only* interactivity: every component here is presentational, so the
dropdowns, the modal and the copy buttons are that file operating on rendered markup.

## The ontology components

Everything in `atoms`/`molecules`/`organisms` is presentational — no `use_state`, no
`Callback`, no event handlers, identical output under SSR and CSR. That property is
load-bearing, and it is now the whole crate: the stateful ontology-browsing tier that used to
live behind the `csr` feature in `interactive` moved back to
[`eona-vocabulary-ui`](https://github.com/Eona-X/eona-vocabulary-ui), its only consumer.

Three components stayed, because they hold no state and the portal's own component gallery
renders them:

```rust
use eona_ui_toolkit::{DatasetCard, OntoAnnotation, OntoBadge};
```

`OntoBadge` tags an RDF/OWL entity kind, a language or a datatype; `OntoAnnotation` renders a
labelled list of literals; `DatasetCard` is a catalogue entry. Their classes live in
`ONTOLOGY_CSS` rather than `COMPONENTS_CSS`, which is why `assets/ontology.css` is still a
separate stylesheet.

`src/ontology.rs` is what is left of the model: `TermKind` and `LiteralValue`, and nothing
else. They are the prop types of those two components, so they could not travel with the
parser — and an app that owns the rest of the model must re-export these two from here rather
than redefine them, or its `Term.kind` will not fit `OntoBadgeProps::variant`.

## The React package

`packages/react` is `@eona-x/ui-toolkit-react`: **33 of the 42 components, as TypeScript, with
no WebAssembly.** It is generated by `cargo xtask bridgegen`, which renders each Yew component
with sentinel values in its props using Yew's own server renderer and splices JSX back over
the byte ranges the sentinels landed in. The markup React ships is markup Yew produced, so a
bug in a React component is a bug in the Rust component named in its `file:line` header.

```sh
cargo xtask bridgegen   # regenerate packages/react and abi/ from src/
cargo xtask check       # is packages/react what this src/ would produce? drift fails, exit 1
```

Nine components are absent and each absence is on the record rather than an omission: four
derive their markup from their props (`Swatch`, `PaletteGroup`, `DatasetCard`,
`OntoAnnotation`) so no pass-through splice can be correct, `Modal` hardcodes `inert` and has
no renderable open state, and four are out of scope by rule (`DemoPage` and the three zero-prop
demo sections). `abi/overrides.toml` records what is being done about each and
`packages/react/src/index.ts` prints the list to consumers.

**The package is not published yet** — that is Layer 4 of
[eona-x/backlog#809](https://gitlab.eona-x.org/eona-x/backlog/-/issues/809), whose prerequisite
is #808. Meanwhile, copy `packages/react/src/*.tsx` into the app, or vendor this repository and
run `npm install && npm run build` inside `packages/react` first: every entry point in its
`package.json` is under `dist/`, which is generated and not committed. `react >= 19` is a peer
dependency because `NavDropdown` renders `inert`.

Section 15-19 of the demo page is the consumer-facing version of all of this, and
`packages/react/README.md` is generated alongside the package.

## Theming

`TOKENS_CSS` carries the charter palette plus a semantic layer named after the job rather
than the ink — `--eona-surface-page`, `--eona-text-primary`, `--eona-border-subtle`,
`--eona-accent`. Those are the themeable ones; the raw palette never changes.

Dark mode is opt-in via an explicit attribute and is deliberately **not** wired to
`prefers-color-scheme`, so a consumer that never sets it keeps the appearance it has today:

```html
<html data-theme="dark">
```

Its values are the charter's own `websiteDarkNeutrals` (slide 5). The tokens live in
[`eona-design-system`](https://gitlab.eona-x.org/eona-x/poc-rnd/design-system) on the
`charte-graphique-design-system` lineage; `assets/tokens.css` here is a byte-identical mirror
of that repo's `brand/tokens.css`.

Note that `COMPONENTS_CSS` still reads the raw palette directly, so the presentational
components do not follow `data-theme` yet — re-pointing them is a separate change, since it
would alter developer.eona-x.eu's rendering.

## Status

Yew **0.23** (the latest release), which is also what `edc-web-components` targets — so
components from both libraries can compose in one binary.

Every component here is presentational: no `use_state`, no `Callback`, no event handlers.
They render identically under SSR and CSR. That is not a style preference — it is the property
that makes the React generation legal, and a component that held state could not be generated
at all. Interaction belongs to the host: `DEMO_JS` for the Yew demo page, a hand-written client
component for a React app.

## Development

```sh
cargo test --features ssr                               # SSR parity + demo-page contracts
cargo check --target wasm32-unknown-unknown --features csr
cargo clippy --all-targets --features ssr
cargo test -p xtask                                     # the generator's own suite
cargo xtask check                                       # packages/react and abi/ match src/
```

`cargo xtask check` is the gate that keeps the two stacks from drifting, and it is a command
rather than a pipeline: this repository has no CI configuration yet
([eona-x/backlog#808](https://gitlab.eona-x.org/eona-x/backlog/-/issues/808)). Run it before
committing anything under `src/`. `xtask/README.md` documents the stages and the seven
invariants it enforces.

`tests/ssr_parity.rs` pins the rendered markup against the portal's committed output. Read
its module comment before changing any element that CSS selects on by attribute.

`tests/ontology_css.rs` guards the stylesheet boundaries: that `ontology.css` resolves
entirely against `tokens.css` (no generic `--accent`/`--card` names that would bind to the
host's) and that every semantic token has a dark override.

`tests/demo_page.rs` guards what the compiler cannot see: that `DEMO_SHELL` still has its
`__APP__` hole and links the assets the integration guide tells hosts to write, and that
every selector in `assets/demo-page.js` still matches something the components render. A
component rename that orphans a handler compiles cleanly and fails there instead.
