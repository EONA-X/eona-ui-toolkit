//! The Rust half of `@eona-x/ui-toolkit-svelte/wasm/charts`.
//!
//! One export, the contract of `@eona-x/ui-toolkit-svelte/wasm`:
//!
//! ```text
//! mount(target: Element, component: &str, props: JsValue) -> Result<Handle, JsValue>
//! ```
//!
//! `component` is `BarChart`, `StackedBarChart`, `LineChart`, `CubeScoreDonut` or
//! `CubeMetricTile`. Props are `{ jsonld, fetchError?, lang?, select?, help? }`; a
//! key a component does not use is ignored, so a host can pass one object to all
//! five. An unknown component or a mistyped prop is an `Err`, never a default.
//!
//! The JSON-LD is parsed in Rust (`eona_ui_toolkit::cube`); the host only fetches
//! the text. A failed fetch goes in as `fetchError` and is rendered by the chart.
//!
//! Build: `wasm-pack build --target web --release wasm/charts`.

use serde::Deserialize;
use wasm_bindgen::prelude::*;
use yew::{AppHandle, BaseComponent, Renderer};

use eona_ui_toolkit::charts::{
    BarChart, BarChartProps, CubeMetricTile, CubeMetricTileProps, CubeScoreDonut,
    CubeScoreDonutProps, LineChart, LineChartProps, StackedBarChart, StackedBarChartProps,
};

/// Mount `component` into `target` with `props`.
///
/// `target` must be empty and stay owned by the caller: the host created it and
/// will remove it; Yew only manages what it renders inside.
#[wasm_bindgen]
pub fn mount(target: web_sys::Element, component: &str, props: JsValue) -> Result<Handle, JsValue> {
    match Kind::parse(component)? {
        Kind::Bar => start::<BarChart>(target, props, bar_props),
        Kind::Stacked => start::<StackedBarChart>(target, props, stacked_props),
        Kind::Line => start::<LineChart>(target, props, line_props),
        Kind::Donut => start::<CubeScoreDonut>(target, props, donut_props),
        Kind::Metric => start::<CubeMetricTile>(target, props, metric_props),
    }
}

/// A mounted chart, handed back to JavaScript.
#[wasm_bindgen]
pub struct Handle {
    inner: Option<Box<dyn Mounted>>,
}

#[wasm_bindgen]
impl Handle {
    /// Re-render with new props. Yew diffs, so the app is not remounted.
    pub fn update(&mut self, props: JsValue) -> Result<(), JsValue> {
        match &mut self.inner {
            Some(m) => m.update(props),
            None => Err(JsValue::from_str("update() after destroy()")),
        }
    }

    /// Unmount. Idempotent: a host may tear down twice on a fast navigation.
    pub fn destroy(&mut self) {
        if let Some(m) = self.inner.take() {
            m.destroy();
        }
    }
}

/// Erases `AppHandle<C>`'s component type so one `Handle` can hold any of them.
trait Mounted {
    fn update(&mut self, props: JsValue) -> Result<(), JsValue>;
    fn destroy(self: Box<Self>);
}

struct App<C: BaseComponent> {
    app: AppHandle<C>,
    read: fn(JsValue) -> Result<C::Properties, JsValue>,
}

impl<C: BaseComponent> Mounted for App<C> {
    fn update(&mut self, props: JsValue) -> Result<(), JsValue> {
        let props = (self.read)(props)?;
        self.app.update(props);
        Ok(())
    }

    fn destroy(self: Box<Self>) {
        self.app.destroy();
    }
}

fn start<C: BaseComponent>(
    target: web_sys::Element,
    props: JsValue,
    read: fn(JsValue) -> Result<C::Properties, JsValue>,
) -> Result<Handle, JsValue> {
    let app = Renderer::<C>::with_root_and_props(target, read(props)?).render();
    Ok(Handle { inner: Some(Box::new(App { app, read })) })
}

/// The registry's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Bar,
    Stacked,
    Line,
    Donut,
    Metric,
}

impl Kind {
    fn parse(name: &str) -> Result<Kind, String> {
        match name {
            "BarChart" => Ok(Kind::Bar),
            "StackedBarChart" => Ok(Kind::Stacked),
            "LineChart" => Ok(Kind::Line),
            "CubeScoreDonut" => Ok(Kind::Donut),
            "CubeMetricTile" => Ok(Kind::Metric),
            other => Err(format!(
                "this module mounts BarChart, StackedBarChart, LineChart, CubeScoreDonut and CubeMetricTile, not {other:?}"
            )),
        }
    }
}

/// The JavaScript props, one shape for all five components.
#[derive(Deserialize, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ChartIn {
    /// Absent when the host's fetch failed and `fetchError` says why.
    #[serde(default)]
    jsonld: String,
    #[serde(default)]
    fetch_error: Option<String>,
    #[serde(default)]
    lang: Option<String>,
    #[serde(default)]
    select: Option<String>,
    #[serde(default)]
    help: Option<String>,
}

fn read(v: JsValue) -> Result<ChartIn, JsValue> {
    serde_wasm_bindgen::from_value(v).map_err(|e| JsValue::from_str(&e.to_string()))
}

fn bar_of(i: ChartIn) -> BarChartProps {
    yew::props!(BarChartProps {
        jsonld: i.jsonld,
        fetch_error: i.fetch_error,
        lang: i.lang,
    })
}

fn stacked_of(i: ChartIn) -> StackedBarChartProps {
    yew::props!(StackedBarChartProps {
        jsonld: i.jsonld,
        fetch_error: i.fetch_error,
        lang: i.lang,
    })
}

fn line_of(i: ChartIn) -> LineChartProps {
    yew::props!(LineChartProps {
        jsonld: i.jsonld,
        fetch_error: i.fetch_error,
        lang: i.lang,
    })
}

fn donut_of(i: ChartIn) -> CubeScoreDonutProps {
    yew::props!(CubeScoreDonutProps {
        jsonld: i.jsonld,
        fetch_error: i.fetch_error,
        lang: i.lang,
        select: i.select,
    })
}

fn metric_of(i: ChartIn) -> CubeMetricTileProps {
    yew::props!(CubeMetricTileProps {
        jsonld: i.jsonld,
        fetch_error: i.fetch_error,
        lang: i.lang,
        help: i.help,
    })
}

fn bar_props(v: JsValue) -> Result<BarChartProps, JsValue> {
    read(v).map(bar_of)
}
fn stacked_props(v: JsValue) -> Result<StackedBarChartProps, JsValue> {
    read(v).map(stacked_of)
}
fn line_props(v: JsValue) -> Result<LineChartProps, JsValue> {
    read(v).map(line_of)
}
fn donut_props(v: JsValue) -> Result<CubeScoreDonutProps, JsValue> {
    read(v).map(donut_of)
}
fn metric_props(v: JsValue) -> Result<CubeMetricTileProps, JsValue> {
    read(v).map(metric_of)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(json: &str) -> ChartIn {
        serde_json::from_str(json).unwrap()
    }

    const FULL: &str = r#"{"jsonld":"{}","fetchError":"404","lang":"fr","select":"2025","help":"per month","extra":1}"#;

    #[test]
    fn registry_names() {
        assert_eq!(Kind::parse("BarChart"), Ok(Kind::Bar));
        assert_eq!(Kind::parse("StackedBarChart"), Ok(Kind::Stacked));
        assert_eq!(Kind::parse("LineChart"), Ok(Kind::Line));
        assert_eq!(Kind::parse("CubeScoreDonut"), Ok(Kind::Donut));
        assert_eq!(Kind::parse("CubeMetricTile"), Ok(Kind::Metric));
    }

    #[test]
    fn unknown_component_is_an_error() {
        let e = Kind::parse("PieChart").unwrap_err();
        assert!(e.contains("\"PieChart\""), "{e}");
        assert!(Kind::parse("barchart").is_err());
        assert!(Kind::parse("").is_err());
    }

    #[test]
    fn bar_takes_jsonld_error_lang() {
        let p = bar_of(input(FULL));
        assert!(p == yew::props!(BarChartProps {
                jsonld: "{}",
                fetch_error: Some("404"),
                lang: Some("fr"),
            }));
    }

    #[test]
    fn stacked_takes_jsonld_error_lang() {
        let p = stacked_of(input(FULL));
        assert!(p == yew::props!(StackedBarChartProps {
                jsonld: "{}",
                fetch_error: Some("404"),
                lang: Some("fr"),
            }));
    }

    #[test]
    fn line_takes_jsonld_error_lang() {
        let p = line_of(input(FULL));
        assert!(p == yew::props!(LineChartProps {
                jsonld: "{}",
                fetch_error: Some("404"),
                lang: Some("fr"),
            }));
    }

    #[test]
    fn donut_adds_select() {
        let p = donut_of(input(FULL));
        assert!(p == yew::props!(CubeScoreDonutProps {
                jsonld: "{}",
                fetch_error: Some("404"),
                lang: Some("fr"),
                select: Some("2025"),
            }));
    }

    #[test]
    fn metric_adds_help() {
        let p = metric_of(input(FULL));
        assert!(p == yew::props!(CubeMetricTileProps {
                jsonld: "{}",
                fetch_error: Some("404"),
                lang: Some("fr"),
                help: Some("per month"),
            }));
    }

    #[test]
    fn optional_props_default_to_none() {
        let i = input(r#"{"jsonld":"x"}"#);
        assert_eq!(i, ChartIn { jsonld: "x".into(), ..Default::default() });
        assert!(donut_of(i) == yew::props!(CubeScoreDonutProps { jsonld: "x" }));
    }

    #[test]
    fn fetch_failure_needs_no_jsonld() {
        let i = input(r#"{"fetchError":"HTTP 404"}"#);
        assert_eq!(i.jsonld, "");
        assert_eq!(i.fetch_error.as_deref(), Some("HTTP 404"));
    }

    #[test]
    fn mistyped_props_are_rejected() {
        assert!(serde_json::from_str::<ChartIn>(r#"{"jsonld":3}"#).is_err());
        assert!(serde_json::from_str::<ChartIn>(r#"{"jsonld":"x","lang":[1]}"#).is_err());
        assert!(serde_json::from_str::<ChartIn>(r#""text""#).is_err());
    }
}
