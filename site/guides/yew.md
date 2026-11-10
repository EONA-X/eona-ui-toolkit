> **Recommended.** Yew is the toolkit's source: every component is written here in Rust, and the
> Svelte and React packages are generated from it. Two sample projects to start from:
> [`examples/yew-ssg`](https://github.com/EONA-X/eona-ui-toolkit/tree/main/examples/yew-ssg) — a
> static site rendered at build time, no WebAssembly — and
> [`examples/yew-trunk`](https://github.com/EONA-X/eona-ui-toolkit/tree/main/examples/yew-trunk), a
> WebAssembly app.

## Install

The crate is not on crates.io: depend on the release tag. Pick the renderer feature for how the
app is built — `csr` for a WebAssembly app (Trunk), `ssr` for HTML rendered ahead of time.

```toml
[dependencies]
eona-ui-toolkit = { git = "https://github.com/EONA-X/eona-ui-toolkit", tag = "v{{version}}", features = ["csr"] }
yew = { version = "0.23", features = ["csr"] }
```

Every component is at the crate root; the stylesheets are string constants, so there is no CSS file
to vendor or keep in step with the crate.

| Constant | What |
| --- | --- |
| `TOKENS_CSS` | the charter palette and the semantic layer, as `:root` custom properties |
| `COMPONENTS_CSS` | component styles — and the EONA-X page shell (see [The page shell](#the-page-shell)) |
| `ONTOLOGY_CSS` | `OntoBadge` and `OntoAnnotation` |
| `LOGO_SVG` | the mark `SiteHeader` loads from `logo/logo-transparent.svg` |

## Your first component

```rust
use eona_ui_toolkit::{Button, MetricTile, Pill, PillTone, COMPONENTS_CSS, TOKENS_CSS};
use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    html! {
        <>
            // The crate's stylesheets, embedded: nothing to serve separately.
            <style>{ TOKENS_CSS }{ COMPONENTS_CSS }</style>
            <MetricTile label="Datasets" value="12" help="Unchanged since the last run" />
            <Pill tone={PillTone::Ok}>{ "Mobility" }</Pill>
            <Button label="Explore the catalogue" href="https://eona-x.eu/" />
        </>
    }
}
```

Every component is presentational: no state, no callbacks, the same markup under `ssr` and `csr`.
Interaction belongs to your app. The [Components](../) page shows every one with its call.

## A static site with Trunk

Trunk builds a WebAssembly single-page app into plain files any static host serves.

`index.html`, next to `Cargo.toml`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>My EONA-X app</title>
    <link data-trunk rel="rust" />
  </head>
  <body></body>
</html>
```

`src/main.rs`: the `App` above, plus

```rust
fn main() {
    yew::Renderer::<App>::new().render();
}
```

```sh
cargo install trunk
rustup target add wasm32-unknown-unknown
trunk serve              # http://127.0.0.1:8080
trunk build --release    # -> dist/
```

> If `trunk build --release` fails with `error parsing wasm`, the `wasm-opt` on your `PATH` is
> older than your Rust toolchain (version 116 cannot read what Rust 1.97 emits). Update binaryen,
> or remove the stale binary so Trunk downloads a matching one.

## A static site rendered ahead of time

No WebAssembly at all: render the pages to HTML at build time with Yew's server renderer — the way
this documentation's Yew pages are built (`examples/render-docs.rs`).

```toml
[dependencies]
eona-ui-toolkit = { git = "https://github.com/EONA-X/eona-ui-toolkit", tag = "v{{version}}", features = ["ssr"] }
yew = { version = "0.23", features = ["ssr"] }
tokio = { version = "1", features = ["macros", "rt"] }
```

```rust
use eona_ui_toolkit::{Button, MetricTile, Pill, PillTone, COMPONENTS_CSS, LOGO_SVG, TOKENS_CSS};
use yew::prelude::*;

#[function_component(Home)]
fn home() -> Html {
    html! {
        <main>
            <MetricTile label="Datasets" value="12" help="Unchanged since the last run" />
            <Pill tone={PillTone::Ok}>{ "Mobility" }</Pill>
            <Button label="Explore the catalogue" href="https://eona-x.eu/" />
        </main>
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::io::Result<()> {
    // `hydratable(false)`: plain HTML, without the markers a client would hydrate from.
    let body = yew::ServerRenderer::<Home>::new().hydratable(false).render().await;
    let page = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <title>My EONA-X site</title>\
         <link rel=\"stylesheet\" href=\"tokens.css\"><link rel=\"stylesheet\" href=\"components.css\">\
         </head><body>{body}</body></html>"
    );
    std::fs::create_dir_all("dist/logo")?;
    std::fs::write("dist/index.html", page)?;
    std::fs::write("dist/tokens.css", TOKENS_CSS)?;
    std::fs::write("dist/components.css", COMPONENTS_CSS)?;
    std::fs::write("dist/logo/logo-transparent.svg", LOGO_SVG)?;
    Ok(())
}
```

`cargo run` writes `dist/`; serve it from anywhere.

## Data-driven charts

The `charts` feature adds five components that draw a JSON-LD dataset. It pulls in `serde_json`; a
build without it stays `yew`-only.

```toml
eona-ui-toolkit = { git = "https://github.com/EONA-X/eona-ui-toolkit", tag = "v{{version}}", features = ["csr", "charts"] }
```

| Component | Draws |
| --- | --- |
| `BarChart` | one bar per category |
| `StackedBarChart` | categories, one segment per series |
| `LineChart` | dates, one line per series |
| `CubeScoreDonut` | one observation as a fraction of 1 |
| `CubeMetricTile` | the latest value and the change from the date before |

They take the text of one dataset in the chart profile, `https://eona-x.eu/shape/chart-profile#`
(terms and SHACL shapes: `eonax-chart-profile` in
[eona-vocabularies-reference](https://github.com/Eona-X/eona-vocabularies-reference)). A dataset is one
`qb:DataSet` with a label and 0 to 10 000 `qb:Observation`s, each with a `chart:category` or a
`chart:date`, a `chart:value` and optionally a `chart:series` (at most 8). The `@context` may be the
built-in `https://eona-x.eu/shape/chart-profile/context.jsonld`, which is never fetched, an inline
object, or absent with full IRIs; any other URL is an error.

| Prop | On | |
| --- | --- | --- |
| `jsonld: AttrValue` | all | the document text |
| `fetch_error: Option<AttrValue>` | all | set by a host whose fetch failed; shown in place of the chart |
| `lang: Option<AttrValue>` | all | language tag for the label and the comment |
| `select: Option<AttrValue>` | `CubeScoreDonut` | the category or date to draw; default the latest |
| `help: Option<AttrValue>` | `CubeMetricTile` | small print under the value |

```rust
<BarChart jsonld={include_str!("airports.jsonld")} />
```

An invalid document renders `.chart--error` (`role="alert"`) with the parser's message, an empty one
`.chart--empty`. Colours come from the classes `.chart__series--1` to `--8`, which read the tokens
`--eona-chart-series-1` to `-8`. React has no version of these: the parse runs in Rust, so the
browser loads them as WebAssembly. [The components page](../) runs all five on sample data.

## Theming

`TOKENS_CSS` has a semantic layer named after the job rather than the ink —
`--eona-surface-page`, `--eona-text-primary`, `--eona-border-subtle`, `--eona-accent` — and a dark
variant of it, switched on the document:

```html
<html data-theme="dark">
```

Use those tokens for your own surfaces and they follow the theme. **The components do not yet:**
`COMPONENTS_CSS` still reads the raw palette, so they render the same in both modes.

## The page shell

`COMPONENTS_CSS` was written for whole EONA-X pages. Besides the component rules it resets `*`,
`body`, `img`, `a`, headings, `p` and `code`, and gives `body` a 156px top padding so the fixed
`SiteHeader` does not cover the page. That is right for an EONA-X page and wrong inside someone
else's: to embed the components in an existing layout, use the scoped `@eona-x/ui-toolkit-css`
package instead (every rule under `.eona-ui`, the shell split out into an opt-in `page.css`) — see
the [React guide](../../react/getting-started/#the-page-shell).

`SiteHeader` loads its logo from `logo/logo-transparent.svg` relative to the page: write
`LOGO_SVG` there, as both examples above do.
