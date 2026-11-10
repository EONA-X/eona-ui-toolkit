use yew::prelude::*;

use super::frame::{self, series_class};
use crate::chart::{bar_layout, coord, BarGeometry, VIEW};

/// `.chart--bar` — one bar per axis key, grouped side by side when the dataset has
/// series. Takes the text of a chart-profile JSON-LD dataset; see the module note.
#[derive(Properties, PartialEq)]
pub struct BarChartProps {
    /// The JSON-LD document.
    pub jsonld: AttrValue,
    /// A fetch failure from the host, shown instead of the chart.
    #[prop_or_default]
    pub fetch_error: Option<AttrValue>,
    /// Language tag for the label and comment, e.g. `"fr"`.
    #[prop_or_default]
    pub lang: Option<AttrValue>,
}

/// The rectangles of a bar or stacked layout.
pub(super) fn bars(g: &BarGeometry) -> Html {
    html! {
        <g class="chart__bars">
            { for g.bars.iter().map(|b| html! {
                <rect class={classes!("chart__bar", series_class(b.series))}
                      x={coord(b.x)} y={coord(b.y)} width={coord(b.w)} height={coord(b.h)}>
                    <title>{ format!("{}: {}", b.label, b.value_label) }</title>
                </rect>
            }) }
        </g>
    }
}

#[function_component(BarChart)]
pub fn bar_chart(props: &BarChartProps) -> Html {
    let id = frame::use_chart_id();
    let loaded = use_memo((props.jsonld.clone(), props.fetch_error.clone()), |(j, e)| {
        frame::load(j, e.as_ref())
    });
    let lang = props.lang.as_deref();
    frame::guarded(&loaded, lang, |cube| {
        let g = bar_layout(cube, VIEW);
        let svg = html! { <>{ frame::axes(&g.y_ticks, &g.x_labels, g.zero_y) }{ bars(&g) }</> };
        Ok(frame::figure("bar", &id, cube, lang, "Bar chart", svg))
    })
}
