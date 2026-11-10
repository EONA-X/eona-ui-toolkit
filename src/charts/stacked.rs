use yew::prelude::*;

use super::bar::bars;
use super::frame;
use crate::chart::{stacked_layout, VIEW};

/// `.chart--stacked` — one stack per axis key, series bottom to top. A negative
/// value is an error: a stack of mixed signs has no honest reading.
#[derive(Properties, PartialEq)]
pub struct StackedBarChartProps {
    /// The JSON-LD document.
    pub jsonld: AttrValue,
    /// A fetch failure from the host, shown instead of the chart.
    #[prop_or_default]
    pub fetch_error: Option<AttrValue>,
    /// Language tag for the label and comment, e.g. `"fr"`.
    #[prop_or_default]
    pub lang: Option<AttrValue>,
}

#[function_component(StackedBarChart)]
pub fn stacked_bar_chart(props: &StackedBarChartProps) -> Html {
    let id = frame::use_chart_id();
    let loaded = use_memo((props.jsonld.clone(), props.fetch_error.clone()), |(j, e)| {
        frame::load(j, e.as_ref())
    });
    let lang = props.lang.as_deref();
    frame::guarded(&loaded, lang, |cube| {
        let g = stacked_layout(cube, VIEW)?;
        let svg = html! { <>{ frame::axes(&g.y_ticks, &g.x_labels, g.zero_y) }{ bars(&g) }</> };
        Ok(frame::figure("stacked", &id, cube, lang, "Stacked bar chart", svg))
    })
}
