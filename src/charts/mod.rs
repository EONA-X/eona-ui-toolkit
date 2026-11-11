//! Data-driven charts: each component takes the text of one JSON-LD dataset in the
//! chart profile (`https://eona-x.eu/shape/chart-profile#`), parses it with
//! [`crate::cube::from_jsonld`] and draws it. Behind the `charts` feature.
//!
//! The components are renderer-agnostic like the rest of the crate. They have no
//! React counterpart: the parse runs in Rust, so a host loads them as WebAssembly
//! (`@eona-x/ui-toolkit-svelte/wasm`). `cargo xtask` classifies everything under
//! this directory as `wasm-only` and requires an `abi/overrides.toml` entry for it.
//!
//! Props shared by all five:
//! - `jsonld`: the document text.
//! - `fetch_error`: set by a host whose fetch failed; shown in place of the chart.
//! - `lang`: picks the language-tagged `rdfs:label` and `rdfs:comment`.
//!
//! Every failure is rendered as `.chart--error` with `role="alert"`, and a dataset
//! without observations as `.chart--empty`. Series colours come from the classes
//! `.chart__series--1` to `--8`, never from inline fills.

mod bar;
mod cube_donut;
mod cube_metric;
mod frame;
mod line;
mod stacked;

pub use bar::{BarChart, BarChartProps};
pub use cube_donut::{CubeScoreDonut, CubeScoreDonutProps};
pub use cube_metric::{CubeMetricTile, CubeMetricTileProps};
pub use line::{LineChart, LineChartProps};
pub use stacked::{StackedBarChart, StackedBarChartProps};
