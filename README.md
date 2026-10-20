> **This is the source of truth.** Development happens here, on GitHub, from `v0.4.0` onward.
> EONA-X's internal GitLab (`eona-x/web/eona-ui-toolkit`) holds the pre-`v0.4.0` history as an
> archive and is no longer the place to raise changes — open a pull request here instead.
>
> The crate is consumed as a git submodule by
> [`eona-vocabulary-ui`](https://github.com/EONA-X/eona-vocabulary-ui) and by EONA-X's developer
> portal.

# eona-ui-toolkit

Shared component library for the EONA-X design system — atoms → molecules → organisms.
Authored once in Yew; consumed from Yew directly, and from React and Svelte through
[`packages/react`](packages/react) and [`packages/svelte`](packages/svelte), which are generated
from this source rather than written.

**Which stack:** Rust with Yew, or Svelte, are recommended — `npm create @eona-x/app` starts a
branded Svelte site. React is supported for existing React apps. See
[Starting a project](#starting-a-project) and the [documentation](https://eona-x.github.io/eona-ui-toolkit/).

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

`npm install @eona-x/ui-toolkit-react` — see [The npm packages](#the-npm-packages). `react >= 19`
is a peer dependency because `NavDropdown` renders `inert`.

Section 15-19 of the demo page is the consumer-facing version of all of this, and
`packages/react/README.md` is generated alongside the package.

## The Svelte package

`packages/svelte` is `@eona-x/ui-toolkit-svelte`: **the same 41 components, as Svelte 5
`.svelte` files, with no WebAssembly**, plus an opt-in entry that mounts a Yew component compiled
to WebAssembly for the ones the generator cannot reach.

| Import | What | WebAssembly |
| --- | --- | --- |
| `@eona-x/ui-toolkit-svelte` | the generated components | none |
| `@eona-x/ui-toolkit-svelte/wasm` | `<YewMount>` and `use:yew` | the module you pass it, loaded on demand |

The first is written by the same `cargo xtask bridgegen` run, from the same `splice` tree the
React package is written from (`xtask/src/svelte.rs`), so the two cannot disagree about markup
unless one spelling is wrong — and `tests/bridge-parity` renders every component through both,
338 prop sets derived from `abi/toolkit.abi.json`, and fails on any difference. `cargo xtask
check` owns `packages/svelte` exactly as it owns `packages/react`, through its own
`abi/package.svelte.manifest.json`. A Rust `Children` prop is a Svelte `Snippet`.

The second exists because everything in this crate is presentational, so the generator covers it
all, and the interactive tier lives elsewhere (`eona-vocabulary-ui`). The contract with the Rust
side is a single `mount(target, component, props)` export; `examples/svelte-wasm` is a working
one, mounting the toolkit's `Button` and a stateful `Counter`. It costs what #809 measured — one
component over WASM is 74,593 B gzip against 112 B pre-rendered (the example, with two, builds
to 130 KB gzip), nothing tree-shakes across the boundary, it renders nothing on the server, and a CSP must allow
`wasm-unsafe-eval` — so reach for it only for what the first entry does not export.

```sh
cargo xtask bridgegen                                    # regenerates packages/react and packages/svelte
cd packages/svelte && npm install --no-save ../css && npm run check   # svelte-check, warnings fail
cd tests/bridge-parity && npm ci && npm test             # React and Svelte render the same HTML
(cd tests/bridge-parity && npm run test:css)             # the published CSS styles them like the crate's
wasm-pack build --target web examples/svelte-wasm && (cd tests/bridge-parity && npm run test:wasm)
```

`npm install @eona-x/ui-toolkit-svelte` — see [The npm packages](#the-npm-packages).

## Starting a project

| Stack | Start from | What you get |
| --- | --- | --- |
| Svelte (recommended) | `npm create @eona-x/app my-site` ([`packages/create-app`](packages/create-app)) | SvelteKit + `adapter-static` on Vite: a branded two-page static site, dependencies installed |
| Rust / Yew (recommended) | [`examples/yew-ssg`](examples/yew-ssg) | a branded two-page static site rendered by Yew at build time, no WebAssembly |
| Rust / Yew | [`examples/yew-trunk`](examples/yew-trunk) | a branded Yew WebAssembly app with state, built by Trunk |
| React (supported) | [the React guide](https://eona-x.github.io/eona-ui-toolkit/react/getting-started/) | a Vite static-render recipe |

The generator's `template/` *is* the Svelte sample, and `@eona-x/create-app` is released with the
toolkit at the same version, so a new site always pins the toolkit it was generated with. CI
(`.github/workflows/bridges.yml`) builds all three projects on every push — the generated one
against this commit's packages.

## Documentation

**<https://eona-x.github.io/eona-ui-toolkit/>**, deployed by `.github/workflows/pages.yml` on every
`main` push that touches what a page shows:

| Page | What | Rendered by |
| --- | --- | --- |
| [`/`](https://eona-x.github.io/eona-ui-toolkit/) | overview of the three stacks | Yew, server-side (`examples/render-docs.rs`) |
| [`/yew/getting-started/`](https://eona-x.github.io/eona-ui-toolkit/yew/getting-started/), [`/react/…`](https://eona-x.github.io/eona-ui-toolkit/react/getting-started/), [`/svelte/…`](https://eona-x.github.io/eona-ui-toolkit/svelte/getting-started/) | install, a first component, a static site (Trunk or Yew SSR; Vite; SvelteKit), theming, the page shell — and for Svelte, mounting a Yew component over WebAssembly. Source: `site/guides/*.md` | Yew, server-side |
| [`/yew/`](https://eona-x.github.io/eona-ui-toolkit/yew/) | the gallery: every component's Yew, React and Svelte call side by side, plus the crate's design sections | Yew, server-side |
| [`/react/`](https://eona-x.github.io/eona-ui-toolkit/react/) | every `@eona-x/ui-toolkit-react` call, live | React (`site/react`) |
| [`/svelte/`](https://eona-x.github.io/eona-ui-toolkit/svelte/) | every `@eona-x/ui-toolkit-svelte` call, live | Svelte (`site/svelte`) |

Every page carries the same navigation — the toolkit's own `SiteHeader`, with a getting started /
components switch per stack. The guides' setup instructions were each built and run as written.

The calls come from one place: `cargo xtask bridgegen` writes the gallery's snippet table and the
two live pages' component lists (`site/*/src/gallery.*`) from `abi/examples.json`, so the three
pages cannot show different calls, and building the live pages typechecks every snippet against the
generated packages. Locally: build `packages/react` and `packages/svelte` (see below), then
`npm ci && npm run dev` in `site/react` or `site/svelte`.

## The npm packages

| Package | From | Built by |
| --- | --- | --- |
| [`@eona-x/ui-toolkit-css`](packages/css) | `assets/` | `cargo xtask bridgegen` (`xtask/src/css.rs`) |
| [`@eona-x/ui-toolkit-react`](packages/react) | `src/` | `cargo xtask bridgegen`, then `tsup` |
| [`@eona-x/ui-toolkit-svelte`](packages/svelte) | `src/` | `cargo xtask bridgegen`, then `svelte-package` |

All three carry the crate's version, and both component packages depend on the stylesheet at
exactly that version. **The stylesheet is scoped**: every rule sits under `.eona-ui`, so a
consumer imports it once and renders the components inside an element with that class, and the
host page is left alone — the crate's `components.css` opens with a page reset and pushes `body`
down 156px, which is right for an EONA-X page and wrong inside anybody else's. That page shell is
published separately as the opt-in `page.css`. `tests/bridge-parity`'s `npm run test:css`
renders every component in Chromium under the crate's sheets and under the published ones and
compares every computed style of every element.

```js
import '@eona-x/ui-toolkit-css';
import { Button } from '@eona-x/ui-toolkit-react'; // or -svelte
// <div class="eona-ui"> <Button label="Go" /> </div>
```

**Releasing.** Bump `version` in `Cargo.toml` and `PACKAGE_VERSION` in `xtask/src/emit.rs` (a
test holds them equal), run `cargo xtask bridgegen`, merge, then push a `vX.Y.Z` tag.
`.github/workflows/publish.yml` refuses a tag that disagrees with either, re-runs the gate and the
bridge suites, and publishes css, react, svelte in that order with npm provenance, skipping any
version already on the registry (so a failed run is re-run with `workflow_dispatch`). It
authenticates with npm trusted publishing and **stages** the release (`npm stage publish`): nothing is
installable until a maintainer approves each version with 2FA, on npmjs.com under Staged Packages or
with `npm stage approve <stage-id>` — css first, since the other two depend on it.

## Theming

`TOKENS_CSS` carries the charter palette plus a semantic layer named after the job rather
than the ink — `--eona-surface-page`, `--eona-text-primary`, `--eona-border-subtle`,
`--eona-accent`. Those are the themeable ones; the raw palette never changes.

Dark mode is opt-in via an explicit attribute and is deliberately **not** wired to
`prefers-color-scheme`, so a consumer that never sets it keeps the appearance it has today:

```html
<html data-theme="dark">
```

Its values are the charter's own `websiteDarkNeutrals` (slide 5). `assets/tokens.css` began as a
byte-identical mirror of `brand/tokens.css` in
[`eona-design-system`](https://gitlab.eona-x.org/eona-x/poc-rnd/design-system), on the
`charte-graphique-design-system` lineage — but that repository is **archived**, and the two have
since diverged: the alert levels moved to the Data Quality set, `--eona-critical` was added, and
a chart scale that does not exist there was introduced. **This file is the source of truth for the
charter now**; the archive is provenance, not an upstream to sync against.

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
component for a React app, a Svelte component or `@eona-x/ui-toolkit-svelte/wasm` for a Svelte one.

## Development

```sh
cargo test --features ssr                               # SSR parity + demo-page contracts
cargo check --target wasm32-unknown-unknown --features csr
cargo clippy --all-targets --features ssr
cargo test -p xtask                                     # the generator's own suite
cargo xtask check                                       # packages/react, packages/svelte and abi/ match src/
```

`cargo xtask check` is the gate that keeps the three stacks from drifting. CI runs it twice:
`.github/workflows/pages.yml` before every gallery deploy, and `.github/workflows/bridges.yml` on
every push and pull request, together with `svelte-check`, the React/Svelte parity suite and the
WASM mount test. Run it before committing anything under `src/`. `xtask/README.md` documents the stages and the seven
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
